//! SyncManifest — metadata for vault.enc in Google Drive cloud storage.
//! Mirrors the Android `SyncManifest` data class.

use serde::{Deserialize, Serialize};

/// Manifest uploaded alongside `vault.enc` to Google Drive.
///
/// Format matches the Android `SyncManifest`:
/// ```json
/// {
///   "version": 2,
///   "vault_checksum": "sha256:...",
///   "updated_at": "2026-05-08T12:00:00Z",
///   "updated_by": "device-id",
///   "encrypted_size": 12345,
///   "schema_version": 2
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncManifest {
    /// Manifest format version (current: 2).
    #[serde(default = "default_manifest_version")]
    pub version: i32,

    /// SHA-256 of the encrypted vault blob, prefixed with `sha256:`.
    pub vault_checksum: String,

    /// ISO-8601 timestamp of last upload.
    pub updated_at: String,

    /// Device identifier of the uploader.
    pub updated_by: String,

    /// Size of the encrypted vault blob in bytes.
    pub encrypted_size: i64,

    /// Schema version of the vault payload.
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
}

fn default_manifest_version() -> i32 {
    2
}
fn default_schema_version() -> i32 {
    2
}

impl SyncManifest {
    pub fn new(vault_checksum: String, updated_by: String, encrypted_size: i64) -> Self {
        Self {
            version: 2,
            vault_checksum,
            updated_at: chrono::Utc::now().to_rfc3339(),
            updated_by,
            encrypted_size,
            schema_version: 2,
        }
    }
}
