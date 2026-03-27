//! `SyncBackend` trait and associated metadata types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Uniform interface implemented by every sync backend.
///
/// All methods are `async` and the trait is object-safe via `async_trait`.
/// To swap backends in tests, use `Box<dyn SyncBackend>` with [`MockBackend`].
///
/// [`MockBackend`]: crate::backends::mock::MockBackend
#[async_trait]
pub trait SyncBackend: Send + Sync {
    /// Upload `data` as object `key`, overwriting any existing content.
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()>;

    /// Upload `data` only if the remote object's ETag matches `expected_etag`
    /// (optimistic concurrency control).  Pass `None` to skip the check and
    /// behave identically to [`upload`][Self::upload].
    ///
    /// Backends that do not support conditional writes (local, mock, WebDAV,
    /// Git) fall back to an unconditional upload via the default implementation.
    /// S3 overrides this to use `PutObject` with `if_match`.
    async fn upload_if_match(
        &self,
        key: &str,
        data: &[u8],
        expected_etag: Option<&str>,
    ) -> Result<()> {
        let _ = expected_etag; // default: ignore etag
        self.upload(key, data).await
    }

    /// Download object `key`, returning its raw bytes.
    async fn download(&self, key: &str) -> Result<Vec<u8>>;

    /// List all object keys that begin with `prefix`.
    ///
    /// Returns keys in lexicographic order.  An empty `prefix` lists all objects.
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Permanently delete object `key`.
    async fn delete(&self, key: &str) -> Result<()>;

    /// Return metadata for object `key` without downloading its content.
    async fn metadata(&self, key: &str) -> Result<SyncMetadata>;

    /// Short, human-readable backend identifier (e.g. `"s3"`, `"local"`, `"mock"`).
    fn backend_name(&self) -> &str;
}

/// Metadata that describes a single synced object.
///
/// Stored alongside every vault file to enable conflict detection and
/// deduplication without re-downloading the full content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncMetadata {
    /// Monotonic version counter; incremented on every successful upload.
    pub version: u64,
    /// Unix timestamp (seconds since epoch) of the last modification.
    pub last_modified: u64,
    /// Hex-encoded SHA-256 digest of the object content.
    pub checksum: String,
    /// Content length in bytes.
    pub size: u64,
}
