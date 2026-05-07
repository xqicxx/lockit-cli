//! Google Drive backend using the REST API.
//!
//! Compatible with the Android `GoogleDriveBackend`:
//! - Uses `appDataFolder` space (hidden from user)
//! - Stores `vault.enc` and `manifest.json`
//! - Same file names so both platforms interoperate

use async_trait::async_trait;
use secrecy::ExposeSecret;

use crate::backend::{SyncBackend, SyncMetadata};
use crate::config::GoogleTokenStore;
use crate::error::{Error, Result};
use crate::http::{async_client, send_async_with_retry};
use crate::util::{sha256_hex, url_encode};

const MIME_OCTET: &str = "application/octet-stream";
const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";

/// Google Drive sync backend.
///
/// Reads/writes to Google Drive's `appDataFolder` — the same hidden
/// storage space used by the Android lockit app.
pub struct GoogleDriveBackend {
    client: reqwest::Client,
    token: GoogleTokenStore,
}

impl GoogleDriveBackend {
    pub fn new(token: GoogleTokenStore) -> Result<Self> {
        Ok(Self {
            client: async_client()?,
            token,
        })
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token.access_token.expose_secret())
    }

    /// Find a file in appDataFolder by name.
    async fn find_file(&self, name: &str) -> Result<Option<GoogleFile>> {
        let query = format!("name='{}' and trashed=false", name);
        let url = format!(
            "{}/files?q={}&spaces=appDataFolder&fields=files(id,name,size)&orderBy=modifiedTime desc",
            DRIVE_API,
            url_encode(&query)
        );

        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Download {
                key: name.into(),
                reason: e.to_string(),
            })?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::Download {
            key: name.into(),
            reason: format!("Failed to read response: {e}"),
        })?;

        if !status.is_success() {
            return Err(Error::Download {
                key: name.into(),
                reason: format!("HTTP {status}: {body}"),
            });
        }

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| Error::Download {
            key: name.into(),
            reason: format!("JSON parse: {e}"),
        })?;

        let files = json["files"].as_array().cloned().unwrap_or_default();
        let first = files.into_iter().next();
        first
            .map(|f| GoogleFile {
                id: f["id"].as_str().unwrap_or("").to_string(),
                name: f["name"].as_str().unwrap_or("").to_string(),
            })
            .map_or(Ok(None), |f| Ok(Some(f)))
    }

    /// Download raw bytes of a Drive file by ID.
    async fn download_file_by_id(&self, file_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/files/{file_id}?alt=media", DRIVE_API);
        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Download {
                key: file_id.into(),
                reason: e.to_string(),
            })?;

        let status = resp.status();
        let body = resp.bytes().await.map_err(|e| Error::Download {
            key: file_id.into(),
            reason: e.to_string(),
        })?;

        if !status.is_success() {
            return Err(Error::Download {
                key: file_id.into(),
                reason: format!("HTTP {status}"),
            });
        }

        Ok(body.to_vec())
    }

    /// Upload or update a file in appDataFolder.
    async fn upsert_file(&self, name: &str, mime: &str, data: &[u8]) -> Result<()> {
        if let Some(file) = self.find_file(name).await? {
            // Update existing file
            let url = format!("{}/files/{}?uploadType=media", DRIVE_API, file.id);
            let request = self
                .client
                .patch(&url)
                .header("Authorization", self.bearer())
                .header("Content-Type", mime)
                .body(data.to_vec());
            let resp = send_async_with_retry(request)
                .await
                .map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: e.to_string(),
                })?;

            if !resp.status().is_success() {
                return Err(Error::Upload {
                    key: name.into(),
                    reason: format!("HTTP {}", resp.status()),
                });
            }
        } else {
            // Create new file using multipart upload
            let url = format!("{}/files?uploadType=multipart", DRIVE_API);

            // Build multipart body manually
            let boundary = "===========lockit_boundary===========";
            let mut body = Vec::new();

            // Part 1: metadata
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
            body.extend_from_slice(
                format!(r#"{{"name":"{name}","parents":["appDataFolder"],"mimeType":"{mime}"}}"#)
                    .as_bytes(),
            );
            body.extend_from_slice(b"\r\n");

            // Part 2: file data
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

            let request = self
                .client
                .post(&url)
                .header("Authorization", self.bearer())
                .header(
                    "Content-Type",
                    format!("multipart/related; boundary={boundary}"),
                )
                .body(body);
            let resp = send_async_with_retry(request)
                .await
                .map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: e.to_string(),
                })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_body = resp.text().await.map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: format!("Failed to read response: {e}"),
                })?;
                return Err(Error::Upload {
                    key: name.into(),
                    reason: format!("HTTP {status}: {err_body}"),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct GoogleFile {
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[async_trait]
impl SyncBackend for GoogleDriveBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        self.upsert_file(key, MIME_OCTET, data).await
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let file = self.find_file(key).await?.ok_or_else(|| Error::NotFound {
            key: key.to_string(),
        })?;
        self.download_file_by_id(&file.id).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let query = format!("name contains '{}' and trashed=false", prefix);
        let url = format!(
            "{}/files?q={}&spaces=appDataFolder&fields=files(name)&orderBy=name",
            DRIVE_API,
            url_encode(&query)
        );

        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::List {
                prefix: prefix.into(),
                reason: e.to_string(),
            })?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::List {
            prefix: prefix.into(),
            reason: e.to_string(),
        })?;

        if !status.is_success() {
            return Err(Error::List {
                prefix: prefix.into(),
                reason: format!("HTTP {status}"),
            });
        }

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| Error::List {
            prefix: prefix.into(),
            reason: e.to_string(),
        })?;

        let files = json["files"].as_array().cloned().unwrap_or_default();
        let mut keys: Vec<String> = files
            .into_iter()
            .filter_map(|f| f["name"].as_str().map(String::from))
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let file = self.find_file(key).await?.ok_or_else(|| Error::NotFound {
            key: key.to_string(),
        })?;
        let url = format!("{}/files/{}", DRIVE_API, file.id);
        let request = self
            .client
            .delete(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Delete {
                key: key.into(),
                reason: e.to_string(),
            })?;

        if !resp.status().is_success() {
            return Err(Error::Delete {
                key: key.into(),
                reason: format!("HTTP {}", resp.status()),
            });
        }
        Ok(())
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let file = self.find_file(key).await?.ok_or_else(|| Error::NotFound {
            key: key.to_string(),
        })?;

        // Get detailed metadata including modifiedTime
        let url = format!(
            "{}/files/{}?fields=name,size,modifiedTime",
            DRIVE_API, file.id
        );
        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Metadata {
                key: key.into(),
                reason: e.to_string(),
            })?;

        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::Metadata {
            key: key.into(),
            reason: e.to_string(),
        })?;

        if !status.is_success() {
            return Err(Error::Metadata {
                key: key.into(),
                reason: format!("HTTP {status}"),
            });
        }

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| Error::Metadata {
            key: key.into(),
            reason: e.to_string(),
        })?;

        // Download the file to compute checksum (metadata endpoint doesn't return it)
        let data = self.download_file_by_id(&file.id).await?;
        let checksum = sha256_hex(&data);

        let last_modified = json["modifiedTime"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or(0);

        Ok(SyncMetadata {
            version: 2,
            last_modified,
            checksum,
            size: json["size"].as_u64().unwrap_or(0),
        })
    }

    fn backend_name(&self) -> &str {
        "google-drive"
    }
}
