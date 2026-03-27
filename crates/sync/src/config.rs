//! Backend configuration types, suitable for deserialization from `config.toml`.

use secrecy::Secret;
use serde::{Deserialize, Serialize};

/// Top-level backend selector.
///
/// Configure in `~/.lockit/config.toml`:
///
/// ```toml
/// [sync]
/// backend = "s3"
/// bucket  = "my-lockit-vault"
/// region  = "us-east-1"
/// ```
///
/// `Serialize` is omitted because `WebDavConfig` contains a `Secret<String>`
/// password field that cannot be serialized; the config is only *read*.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "backend", rename_all = "lowercase")]
pub enum BackendConfig {
    Local(LocalConfig),
    S3(S3Config),
    WebDav(WebDavConfig),
    Git(GitConfig),
}

/// Sync to a local directory (useful for testing or network-mounted volumes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    /// Absolute path to the sync directory.
    pub path: String,
}

/// Configuration for any S3-compatible object-storage backend.
///
/// Works with AWS S3, Aliyun OSS, Tencent COS, Huawei OBS, MinIO, Qiniu Kodo,
/// and any other service that implements the S3 REST API.
///
/// # Example — Aliyun OSS
/// ```toml
/// [sync]
/// backend            = "s3"
/// bucket             = "my-lockit-vault"
/// region             = "cn-hangzhou"
/// endpoint           = "https://oss-cn-hangzhou.aliyuncs.com"
/// path_style         = false
/// access_key_id      = "..."   # or leave empty to read from env
/// secret_access_key  = "..."
/// ```
///
/// # Example — MinIO (local dev)
/// ```toml
/// [sync]
/// backend            = "s3"
/// bucket             = "lockit"
/// region             = "us-east-1"
/// endpoint           = "http://localhost:9000"
/// path_style         = true
/// access_key_id      = "minioadmin"
/// secret_access_key  = "minioadmin"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// Target bucket name.
    pub bucket: String,

    /// Optional key prefix applied to every object (e.g. `"lockit/"`).
    #[serde(default)]
    pub prefix: String,

    /// Custom endpoint URL.  `None` → use the default AWS S3 endpoint.
    ///
    /// Set this for any non-AWS provider.
    pub endpoint: Option<String>,

    /// AWS region or equivalent (e.g. `"cn-hangzhou"` for Aliyun).
    pub region: String,

    /// Use path-style URLs (`endpoint/bucket/key` instead of `bucket.endpoint/key`).
    ///
    /// Required for MinIO and some Chinese cloud providers.
    #[serde(default)]
    pub path_style: bool,

    /// Access key ID.  Falls back to `AWS_ACCESS_KEY_ID` env var when empty.
    #[serde(default)]
    pub access_key_id: String,

    /// Secret access key.  Falls back to `AWS_SECRET_ACCESS_KEY` env var when empty.
    #[serde(default)]
    pub secret_access_key: String,
}

/// WebDAV backend configuration (e.g. Nextcloud, 坚果云).
///
/// `Serialize` is intentionally omitted: the password is a `Secret<String>`
/// that does not implement `SerializableSecret`, and the config is only ever
/// *read* from `config.toml`, never written back.
#[derive(Debug, Clone, Deserialize)]
pub struct WebDavConfig {
    /// Full WebDAV base URL.
    pub url: String,
    pub username: String,
    /// WebDAV password; zeroized on drop.
    pub password: Secret<String>,
}

/// Git repository backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Remote repository URL (HTTPS or SSH).
    pub repo_url: String,
    /// Branch to commit vault files to.
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}
