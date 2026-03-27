//! In-memory mock backend for use in tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::backend::{SyncBackend, SyncMetadata};
use crate::error::{Error, Result};
use crate::util::sha256_hex;

/// Thread-safe in-memory backend.
///
/// All data lives in a `HashMap<String, Vec<u8>>` behind an `Arc<Mutex<…>>`,
/// so clones of the same `MockBackend` share the same store.  This makes it
/// easy to pass one clone to the unit-under-test while keeping another clone
/// for assertions.
///
/// ```rust
/// # use lockit_sync::backends::mock::MockBackend;
/// # use lockit_sync::backend::SyncBackend;
/// # #[tokio::main] async fn main() {
/// let backend = MockBackend::new();
/// backend.upload("vault.lockit", b"encrypted").await.unwrap();
/// let data = backend.download("vault.lockit").await.unwrap();
/// assert_eq!(data, b"encrypted");
/// # }
/// ```
#[derive(Clone)]
pub struct MockBackend {
    store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockBackend {
    /// Create a new, empty mock backend.
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return the number of objects currently stored.
    pub fn len(&self) -> usize {
        self.store.lock().unwrap().len()
    }

    /// Return `true` when the store contains no objects.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncBackend for MockBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        self.store
            .lock()
            .unwrap()
            .insert(key.to_string(), data.to_vec());
        tracing::debug!(key, bytes = data.len(), "mock upload");
        Ok(())
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        self.store
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| Error::NotFound {
                key: key.to_string(),
            })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let store = self.store.lock().unwrap();
        let mut keys: Vec<String> = store
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.store
            .lock()
            .unwrap()
            .remove(key)
            .ok_or_else(|| Error::NotFound {
                key: key.to_string(),
            })?;
        Ok(())
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let store = self.store.lock().unwrap();
        let data = store.get(key).ok_or_else(|| Error::NotFound {
            key: key.to_string(),
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(SyncMetadata {
            version: 1,
            last_modified: now,
            checksum: sha256_hex(data),
            size: data.len() as u64,
        })
    }

    fn backend_name(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upload_then_download_roundtrip() {
        let b = MockBackend::new();
        b.upload("vault.lockit", b"hello world").await.unwrap();
        let got = b.download("vault.lockit").await.unwrap();
        assert_eq!(got, b"hello world");
    }

    #[tokio::test]
    async fn download_missing_key_returns_not_found() {
        let b = MockBackend::new();
        let err = b.download("missing").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn upload_overwrites_existing() {
        let b = MockBackend::new();
        b.upload("k", b"v1").await.unwrap();
        b.upload("k", b"v2").await.unwrap();
        assert_eq!(b.download("k").await.unwrap(), b"v2");
    }

    #[tokio::test]
    async fn list_filters_by_prefix() {
        let b = MockBackend::new();
        b.upload("lockit/vault.lockit", b"a").await.unwrap();
        b.upload("lockit/meta.json", b"b").await.unwrap();
        b.upload("other/file", b"c").await.unwrap();

        let keys = b.list("lockit/").await.unwrap();
        assert_eq!(keys, ["lockit/meta.json", "lockit/vault.lockit"]);
    }

    #[tokio::test]
    async fn list_empty_prefix_returns_all() {
        let b = MockBackend::new();
        b.upload("a", b"1").await.unwrap();
        b.upload("b", b"2").await.unwrap();
        let keys = b.list("").await.unwrap();
        assert_eq!(keys, ["a", "b"]);
    }

    #[tokio::test]
    async fn delete_removes_object() {
        let b = MockBackend::new();
        b.upload("k", b"v").await.unwrap();
        assert_eq!(b.len(), 1);
        b.delete("k").await.unwrap();
        assert!(b.is_empty());
    }

    #[tokio::test]
    async fn delete_missing_key_returns_not_found() {
        let b = MockBackend::new();
        let err = b.delete("nope").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn metadata_checksum_matches_content() {
        let b = MockBackend::new();
        let data = b"encrypted vault bytes";
        b.upload("vault.lockit", data).await.unwrap();
        let meta = b.metadata("vault.lockit").await.unwrap();
        assert_eq!(meta.checksum, sha256_hex(data));
        assert_eq!(meta.size, data.len() as u64);
    }

    #[tokio::test]
    async fn metadata_missing_key_returns_not_found() {
        let b = MockBackend::new();
        let err = b.metadata("gone").await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[tokio::test]
    async fn clones_share_store() {
        let b1 = MockBackend::new();
        let b2 = b1.clone();
        b1.upload("shared", b"data").await.unwrap();
        let got = b2.download("shared").await.unwrap();
        assert_eq!(got, b"data");
    }

    #[test]
    fn backend_name_is_mock() {
        assert_eq!(MockBackend::new().backend_name(), "mock");
    }
}
