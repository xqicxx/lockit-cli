//! Local filesystem sync backend.
//!
//! Stores vault files under a configured base directory.  Useful for testing,
//! network-mounted volumes, or any scenario where the "remote" storage is a
//! local path.
//!
//! # Example
//!
//! ```rust,no_run
//! # use lockit_sync::backends::local::LocalBackend;
//! # use lockit_sync::backend::SyncBackend;
//! # #[tokio::main] async fn main() -> lockit_sync::error::Result<()> {
//! let backend = LocalBackend::new("/tmp/lockit-sync");
//! backend.upload("vault.lockit", b"encrypted").await?;
//! let data = backend.download("vault.lockit").await?;
//! assert_eq!(data, b"encrypted");
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;

use crate::backend::{SyncBackend, SyncMetadata};
use crate::error::{Error, Result};
use crate::util::sha256_hex;

/// Backend that reads and writes files on the local filesystem.
///
/// Thread-safe — all operations acquire no shared state; they rely on
/// filesystem atomicity for correctness.
pub struct LocalBackend {
    base_dir: PathBuf,
}

impl LocalBackend {
    /// Create a new local backend rooted at `base_dir`.
    ///
    /// The directory is created lazily on the first `upload` call if it does
    /// not already exist.
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Resolve `key` to its full path under `base_dir`.
    ///
    /// Returns `Err` if `key` is not a plain filename (i.e. contains path
    /// separators or `..` components) so callers cannot escape `base_dir`
    /// with a path-traversal payload such as `../../etc/passwd`.
    fn file_path(&self, key: &str) -> Result<PathBuf> {
        // Reject any key that looks like a path: no separators, no dot-dot.
        if key.contains(['/', '\\']) || key.split('/').any(|c| c == "..") || key.starts_with('.') {
            return Err(Error::InvalidKey {
                key: key.to_string(),
                reason: "key must be a plain filename with no path components".to_string(),
            });
        }
        Ok(self.base_dir.join(key))
    }
}

#[async_trait]
impl SyncBackend for LocalBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir).map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: format!("create dir: {e}"),
        })?;
        let path = self.file_path(key)?;
        std::fs::write(&path, data).map_err(|e| Error::Upload {
            key: key.to_string(),
            reason: e.to_string(),
        })?;
        tracing::debug!(key, bytes = data.len(), "local upload");
        Ok(())
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.file_path(key)?;
        if !path.exists() {
            return Err(Error::NotFound {
                key: key.to_string(),
            });
        }
        std::fs::read(&path).map_err(|e| Error::Download {
            key: key.to_string(),
            reason: e.to_string(),
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        if !self.base_dir.exists() {
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(&self.base_dir).map_err(|e| Error::List {
            prefix: prefix.to_string(),
            reason: e.to_string(),
        })?;

        let mut keys: Vec<String> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name().into_string().ok()?;
                if name.starts_with(prefix) {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.file_path(key)?;
        if !path.exists() {
            return Err(Error::NotFound {
                key: key.to_string(),
            });
        }
        std::fs::remove_file(&path).map_err(|e| Error::Delete {
            key: key.to_string(),
            reason: e.to_string(),
        })
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let path = self.file_path(key)?;
        if !path.exists() {
            return Err(Error::NotFound {
                key: key.to_string(),
            });
        }
        let data = std::fs::read(&path).map_err(|e| Error::Metadata {
            key: key.to_string(),
            reason: e.to_string(),
        })?;
        let meta = std::fs::metadata(&path).map_err(|e| Error::Metadata {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        let last_modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(SyncMetadata {
            version: 1,
            last_modified,
            checksum: sha256_hex(&data),
            size: data.len() as u64,
        })
    }

    fn backend_name(&self) -> &str {
        "local"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_backend() -> (TempDir, LocalBackend) {
        let dir = TempDir::new().unwrap();
        let backend = LocalBackend::new(dir.path());
        (dir, backend)
    }

    #[tokio::test]
    async fn upload_then_download_roundtrip() {
        let (_dir, b) = temp_backend();
        b.upload("vault.lockit", b"hello world").await.unwrap();
        let got = b.download("vault.lockit").await.unwrap();
        assert_eq!(got, b"hello world");
    }

    #[tokio::test]
    async fn download_missing_key_returns_not_found() {
        let (_dir, b) = temp_backend();
        let err = b.download("missing").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn upload_overwrites_existing() {
        let (_dir, b) = temp_backend();
        b.upload("k", b"v1").await.unwrap();
        b.upload("k", b"v2").await.unwrap();
        assert_eq!(b.download("k").await.unwrap(), b"v2");
    }

    #[tokio::test]
    async fn list_returns_sorted_keys() {
        let (_dir, b) = temp_backend();
        b.upload("b.lockit", b"1").await.unwrap();
        b.upload("a.lockit", b"2").await.unwrap();
        let keys = b.list("").await.unwrap();
        assert_eq!(keys, ["a.lockit", "b.lockit"]);
    }

    #[tokio::test]
    async fn list_filters_by_prefix() {
        let (_dir, b) = temp_backend();
        b.upload("vault.lockit", b"a").await.unwrap();
        b.upload("other.dat", b"b").await.unwrap();
        let keys = b.list("vault").await.unwrap();
        assert_eq!(keys, ["vault.lockit"]);
    }

    #[tokio::test]
    async fn list_empty_dir_returns_empty() {
        let (_dir, b) = temp_backend();
        let keys = b.list("").await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn delete_removes_file() {
        let (_dir, b) = temp_backend();
        b.upload("k", b"v").await.unwrap();
        b.delete("k").await.unwrap();
        let err = b.download("k").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn delete_missing_key_returns_not_found() {
        let (_dir, b) = temp_backend();
        let err = b.delete("nope").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn metadata_checksum_matches_content() {
        let (_dir, b) = temp_backend();
        let data = b"encrypted vault bytes";
        b.upload("vault.lockit", data).await.unwrap();
        let meta = b.metadata("vault.lockit").await.unwrap();
        assert_eq!(meta.checksum, sha256_hex(data));
        assert_eq!(meta.size, data.len() as u64);
    }

    #[tokio::test]
    async fn metadata_missing_key_returns_not_found() {
        let (_dir, b) = temp_backend();
        let err = b.metadata("gone").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[test]
    fn backend_name_is_local() {
        let (_dir, b) = temp_backend();
        assert_eq!(b.backend_name(), "local");
    }

    // ── Path-traversal rejection ──────────────────────────────────────────

    #[tokio::test]
    async fn upload_rejects_dotdot_traversal() {
        let (_dir, b) = temp_backend();
        let err = b.upload("../../etc/passwd", b"x").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }

    #[tokio::test]
    async fn upload_rejects_absolute_path() {
        let (_dir, b) = temp_backend();
        let err = b.upload("/etc/passwd", b"x").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }

    #[tokio::test]
    async fn upload_rejects_subdirectory() {
        let (_dir, b) = temp_backend();
        let err = b.upload("sub/dir/file", b"x").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }

    #[tokio::test]
    async fn download_rejects_traversal() {
        let (_dir, b) = temp_backend();
        let err = b.download("../secret").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }

    #[tokio::test]
    async fn delete_rejects_traversal() {
        let (_dir, b) = temp_backend();
        let err = b.delete("../target").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }

    #[tokio::test]
    async fn metadata_rejects_traversal() {
        let (_dir, b) = temp_backend();
        let err = b.metadata("../Cargo.toml").await.unwrap_err();
        assert!(matches!(err, Error::InvalidKey { .. }), "got: {err}");
    }
}
