//! S3-compatible sync backend.
//!
//! Supports AWS S3, Aliyun OSS, Tencent COS, Huawei OBS, MinIO, Qiniu Kodo,
//! and any other S3-compatible service via a custom `endpoint`.

use async_trait::async_trait;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{Builder, Region};
use aws_sdk_s3::primitives::ByteStream;

use crate::backend::{SyncBackend, SyncMetadata};
use crate::config::S3Config;
use crate::error::{Error, Result};
use crate::util::full_key;
#[cfg(test)]
use crate::util::sha256_hex;

/// S3-compatible sync backend.
///
/// Create via [`S3Backend::new`] or [`crate::factory::SyncBackendFactory::from_config`].
#[derive(Debug)]
pub struct S3Backend {
    client: Client,
    bucket: String,
    prefix: String,
}

impl S3Backend {
    /// Construct an `S3Backend` from an [`S3Config`].
    ///
    /// This is a synchronous constructor — no network calls are made until
    /// the first operation is performed.
    pub fn new(config: S3Config) -> Result<Self> {
        let credentials = if config.access_key_id.is_empty() {
            return Err(Error::Config(
                "access_key_id is required (or set AWS_ACCESS_KEY_ID)".into(),
            ));
        } else {
            Credentials::new(
                &config.access_key_id,
                &config.secret_access_key,
                None,
                None,
                "lockit",
            )
        };

        let mut builder = Builder::new()
            .region(Region::new(config.region))
            .credentials_provider(credentials)
            .force_path_style(config.path_style);

        if let Some(endpoint) = config.endpoint {
            builder = builder.endpoint_url(endpoint);
        }

        let client = Client::from_conf(builder.build());

        Ok(S3Backend {
            client,
            bucket: config.bucket,
            prefix: config.prefix,
        })
    }

    fn object_key(&self, key: &str) -> String {
        full_key(&self.prefix, key)
    }
}

#[async_trait]
impl SyncBackend for S3Backend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        self.upload_if_match(key, data, None).await
    }

    /// S3 override: when `expected_etag` is supplied, set `if_match` on the
    /// `PutObject` request so that S3 rejects the write if another client has
    /// modified the object since we last read it (optimistic concurrency).
    async fn upload_if_match(
        &self,
        key: &str,
        data: &[u8],
        expected_etag: Option<&str>,
    ) -> Result<()> {
        let object_key = self.object_key(key);
        tracing::debug!(
            bucket = %self.bucket,
            key = %object_key,
            bytes = data.len(),
            etag = ?expected_etag,
            "s3 upload"
        );

        let mut req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .body(ByteStream::from(data.to_vec()));

        if let Some(etag) = expected_etag {
            req = req.if_match(etag);
        }

        req.send().await.map_err(|e| {
            let msg = e.to_string();
            // S3 returns 412 Precondition Failed when if_match doesn't match.
            if msg.contains("412") || msg.contains("PreconditionFailed") {
                Error::Upload {
                    key: object_key,
                    reason: "conflict: remote object was modified since last sync (ETag mismatch); run `lk sync pull` first".into(),
                }
            } else {
                Error::Upload {
                    key: object_key,
                    reason: msg,
                }
            }
        })?;

        Ok(())
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let object_key = self.object_key(key);
        tracing::debug!(bucket = %self.bucket, key = %object_key, "s3 download");

        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("NoSuchKey") || msg.contains("404") {
                    Error::NotFound {
                        key: object_key.clone(),
                    }
                } else {
                    Error::Download {
                        key: object_key.clone(),
                        reason: msg,
                    }
                }
            })?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| Error::Download {
                key: object_key,
                reason: e.to_string(),
            })?
            .into_bytes()
            .to_vec();

        Ok(bytes)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let full_prefix = full_key(&self.prefix, prefix);
        tracing::debug!(bucket = %self.bucket, prefix = %full_prefix, "s3 list");

        let mut keys: Vec<String> = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(&full_prefix);

            if let Some(token) = continuation_token.take() {
                req = req.continuation_token(token);
            }

            let page = req.send().await.map_err(|e| Error::List {
                prefix: full_prefix.clone(),
                reason: e.to_string(),
            })?;

            for obj in page.contents() {
                if let Some(k) = obj.key() {
                    keys.push(k.to_string());
                }
            }

            if page.is_truncated().unwrap_or(false) {
                continuation_token = page.next_continuation_token().map(String::from);
            } else {
                break;
            }
        }

        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let object_key = self.object_key(key);
        tracing::debug!(bucket = %self.bucket, key = %object_key, "s3 delete");

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|e| Error::Delete {
                key: object_key,
                reason: e.to_string(),
            })?;

        Ok(())
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let object_key = self.object_key(key);

        let head = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&object_key)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("NotFound") {
                    Error::NotFound {
                        key: object_key.clone(),
                    }
                } else {
                    Error::Metadata {
                        key: object_key.clone(),
                        reason: msg,
                    }
                }
            })?;

        let last_modified = head.last_modified().map(|dt| dt.secs() as u64).unwrap_or(0);

        let checksum = head
            .e_tag()
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        let size = head.content_length().unwrap_or(0) as u64;

        Ok(SyncMetadata {
            version: 1,
            last_modified,
            checksum,
            size,
        })
    }

    fn backend_name(&self) -> &str {
        "s3"
    }
}

// ── Unit tests (no network) ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minio_config() -> S3Config {
        S3Config {
            bucket: "lockit-test".into(),
            prefix: String::new(),
            endpoint: Some("http://localhost:9000".into()),
            region: "us-east-1".into(),
            path_style: true,
            access_key_id: "minioadmin".into(),
            secret_access_key: "minioadmin".into(),
        }
    }

    fn aliyun_config() -> S3Config {
        S3Config {
            bucket: "my-lockit-vault".into(),
            prefix: "lockit/".into(),
            endpoint: Some("https://oss-cn-hangzhou.aliyuncs.com".into()),
            region: "cn-hangzhou".into(),
            path_style: false,
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
        }
    }

    #[test]
    fn construct_minio_backend() {
        let backend = S3Backend::new(minio_config()).unwrap();
        assert_eq!(backend.backend_name(), "s3");
    }

    #[test]
    fn construct_aliyun_backend() {
        let backend = S3Backend::new(aliyun_config()).unwrap();
        assert_eq!(backend.backend_name(), "s3");
    }

    #[test]
    fn object_key_no_prefix() {
        let mut cfg = minio_config();
        cfg.prefix = String::new();
        let b = S3Backend::new(cfg).unwrap();
        assert_eq!(b.object_key("vault.lockit"), "vault.lockit");
    }

    #[test]
    fn object_key_with_prefix() {
        let b = S3Backend::new(aliyun_config()).unwrap();
        assert_eq!(b.object_key("vault.lockit"), "lockit/vault.lockit");
    }

    #[test]
    fn object_key_prefix_trailing_slash_not_doubled() {
        let mut cfg = minio_config();
        cfg.prefix = "a/b/".into();
        let b = S3Backend::new(cfg).unwrap();
        assert_eq!(b.object_key("c"), "a/b/c");
    }

    #[test]
    fn missing_access_key_returns_config_error() {
        let mut cfg = minio_config();
        cfg.access_key_id = String::new();
        let result = S3Backend::new(cfg);
        assert!(matches!(result, Err(Error::Config(_))));
    }

    // ── Integration tests (require MinIO on localhost:9000) ──────────────────
    // Run with: cargo test --package lockit-sync -- --ignored

    #[tokio::test]
    #[ignore = "requires MinIO on localhost:9000 (minioadmin/minioadmin)"]
    async fn integration_upload_download_roundtrip() {
        let b = S3Backend::new(minio_config()).unwrap();
        let data = b"encrypted vault content";
        b.upload("vault.lockit", data).await.unwrap();
        let got = b.download("vault.lockit").await.unwrap();
        assert_eq!(got, data);
        b.delete("vault.lockit").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MinIO on localhost:9000 (minioadmin/minioadmin)"]
    async fn integration_list_objects() {
        let b = S3Backend::new(minio_config()).unwrap();
        b.upload("a/vault.lockit", b"v1").await.unwrap();
        b.upload("a/meta.json", b"{}").await.unwrap();
        b.upload("b/other", b"x").await.unwrap();

        let keys = b.list("a/").await.unwrap();
        assert!(keys.contains(&"a/meta.json".to_string()));
        assert!(keys.contains(&"a/vault.lockit".to_string()));
        assert!(!keys.contains(&"b/other".to_string()));

        b.delete("a/vault.lockit").await.unwrap();
        b.delete("a/meta.json").await.unwrap();
        b.delete("b/other").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires MinIO on localhost:9000 (minioadmin/minioadmin)"]
    async fn integration_checksum_verified_after_download() {
        let b = S3Backend::new(minio_config()).unwrap();
        let data = b"sensitive credential data";
        b.upload("vault.lockit", data).await.unwrap();
        let got = b.download("vault.lockit").await.unwrap();
        assert_eq!(sha256_hex(&got), sha256_hex(data));
        b.delete("vault.lockit").await.unwrap();
    }
}
