//! Sync state persistence — tracks checksums to detect drift between
//! local and remote vaults.
//!
//! The Android side stores three checksums (`local`, `cloud`, and a legacy
//! unified `sync` checksum).  This struct mirrors that design so the same
//! conflict-detection logic works on both platforms.

use serde::{Deserialize, Serialize};

/// Persistent sync state saved after each successful sync operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    /// SHA-256 of the local vault file at last successful sync.
    pub local_checksum: String,
    /// SHA-256 of the remote vault file at last successful sync.
    pub remote_checksum: String,
    /// Content size of the remote vault at last successful sync.
    pub remote_size: u64,
}

impl SyncState {
    pub fn new(local_checksum: String, remote_checksum: String, remote_size: u64) -> Self {
        Self {
            local_checksum,
            remote_checksum,
            remote_size,
        }
    }

    /// Whether the local vault has changed since last sync.
    pub fn local_changed(&self, current_local_checksum: &str) -> bool {
        self.local_checksum != current_local_checksum
    }

    /// Whether the remote vault has changed since last sync.
    pub fn remote_changed(&self, current_remote_checksum: &str, current_remote_size: u64) -> bool {
        self.remote_checksum != current_remote_checksum || self.remote_size != current_remote_size
    }
}
