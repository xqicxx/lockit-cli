//! Pure-logic conflict detection for vault sync.
//! Mirrors the Android `ConflictDetector` object.

use crate::backend::SyncMetadata;
use crate::state::SyncState;

/// Result of conflict detection.
#[derive(Debug, Clone)]
pub struct SyncConflict {
    pub local_checksum: String,
    pub remote_checksum: String,
    pub remote_updated: u64,
    pub backend_name: String,
}

impl std::fmt::Display for SyncConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sync conflict: local ({}) and remote ({}) both modified since last sync on {}",
            self.local_checksum, self.remote_checksum, self.backend_name,
        )
    }
}

/// Outcome of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    AlreadyUpToDate,
    Pushed,
    Pulled,
    NeedsBaseline,
    Error,
}

/// Strategy for resolving sync conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolveStrategy {
    /// Keep local version, overwrite remote.
    KeepLocal,
    /// Keep remote version, overwrite local.
    KeepRemote,
    /// Newer timestamp wins (default for auto-sync).
    #[default]
    LastWriteWins,
}

/// Pure-logic conflict detection.
///
/// No I/O, no dependencies — testable with unit tests.
pub struct ConflictDetector;

impl ConflictDetector {
    /// Check for push conflict: remote was modified since our last sync.
    ///
    /// Returns `Some(SyncConflict)` when both local and remote have changed
    /// since the last recorded sync state.
    pub fn check_push_conflict(
        local_checksum: &str,
        remote: &SyncMetadata,
        state: Option<&SyncState>,
    ) -> Option<SyncConflict> {
        let Some(state) = state else {
            // First push with existing remote — upload is safe.
            return None;
        };

        let local_changed = state.local_changed(local_checksum);
        let remote_changed = state.remote_changed(&remote.checksum, remote.size);

        if local_changed && remote_changed {
            Some(SyncConflict {
                local_checksum: local_checksum.to_string(),
                remote_checksum: remote.checksum.clone(),
                remote_updated: remote.last_modified,
                backend_name: String::new(),
            })
        } else if remote_changed {
            // Remote changed but local didn't — special case: user is
            // trying to push an unchanged local vault onto a changed remote.
            // This means the local vault is stale.
            Some(SyncConflict {
                local_checksum: state.local_checksum.clone(),
                remote_checksum: remote.checksum.clone(),
                remote_updated: remote.last_modified,
                backend_name: String::new(),
            })
        } else {
            // Neither changed since last sync — no conflict.
            None
        }
    }

    /// Check for pull conflict: local was modified since our last sync.
    ///
    /// Returns `Some(SyncConflict)` when the local vault has changed
    /// since the last recorded sync state.
    pub fn check_pull_conflict(
        local_checksum: &str,
        state: Option<&SyncState>,
    ) -> Option<SyncConflict> {
        let Some(state) = state else {
            // First pull — no local state to conflict with.
            return None;
        };

        if state.local_changed(local_checksum) {
            Some(SyncConflict {
                local_checksum: local_checksum.to_string(),
                remote_checksum: state.remote_checksum.clone(),
                remote_updated: 0,
                backend_name: String::new(),
            })
        } else {
            None
        }
    }

    /// Determine conflict resolution using LastWriteWins heuristic.
    ///
    /// Without a local modification timestamp, compare remote timestamp
    /// against now: if remote was modified recently (< 60s), pull wins;
    /// otherwise push (prefer the user's active workspace).
    pub fn resolve_last_write_wins(remote_timestamp: u64, now: u64) -> ResolveDecision {
        let recent = remote_timestamp > now.saturating_sub(60);
        if recent {
            ResolveDecision::PullWins
        } else {
            ResolveDecision::PushWins
        }
    }
}

/// Decision from LastWriteWins resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveDecision {
    PullWins,
    PushWins,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_meta(checksum: &str, size: u64, ts: u64) -> SyncMetadata {
        SyncMetadata {
            version: 1,
            last_modified: ts,
            checksum: checksum.to_string(),
            size,
        }
    }

    fn state(local: &str, remote: &str, size: u64) -> SyncState {
        SyncState::new(local.to_string(), remote.to_string(), size)
    }

    #[test]
    fn push_no_state_means_no_conflict() {
        let meta = remote_meta("remote-1", 42, 1000);
        assert!(ConflictDetector::check_push_conflict("local-1", &meta, None).is_none());
    }

    #[test]
    fn push_local_changed_remote_unchanged_no_conflict() {
        let meta = remote_meta("remote-1", 42, 1000);
        let s = state("local-1", "remote-1", 42);
        assert!(ConflictDetector::check_push_conflict("local-2", &meta, Some(&s)).is_none());
    }

    #[test]
    fn push_both_changed_is_conflict() {
        let meta = remote_meta("remote-2", 50, 1000);
        let s = state("local-1", "remote-1", 42);
        let conflict = ConflictDetector::check_push_conflict("local-2", &meta, Some(&s)).unwrap();
        assert_eq!(conflict.local_checksum, "local-2");
        assert_eq!(conflict.remote_checksum, "remote-2");
    }

    #[test]
    fn push_remote_changed_local_unchanged_is_conflict() {
        let meta = remote_meta("remote-2", 50, 1000);
        let s = state("local-1", "remote-1", 42);
        let conflict = ConflictDetector::check_push_conflict("local-1", &meta, Some(&s)).unwrap();
        // local didn't change but remote did — user is trying to push a stale vault
        assert_eq!(conflict.local_checksum, "local-1");
    }

    #[test]
    fn pull_no_state_means_no_conflict() {
        assert!(ConflictDetector::check_pull_conflict("local-1", None).is_none());
    }

    #[test]
    fn pull_local_unchanged_no_conflict() {
        let s = state("local-1", "remote-1", 42);
        assert!(ConflictDetector::check_pull_conflict("local-1", Some(&s)).is_none());
    }

    #[test]
    fn pull_local_changed_is_conflict() {
        let s = state("local-1", "remote-1", 42);
        let conflict = ConflictDetector::check_pull_conflict("local-2", Some(&s)).unwrap();
        assert_eq!(conflict.local_checksum, "local-2");
    }

    #[test]
    fn last_write_wins_recent_remote_pulls() {
        let now = 2000;
        let decision = ConflictDetector::resolve_last_write_wins(1980, now);
        assert_eq!(decision, ResolveDecision::PullWins);
    }

    #[test]
    fn last_write_wins_stale_remote_pushes() {
        let now = 2000;
        let decision = ConflictDetector::resolve_last_write_wins(1000, now);
        assert_eq!(decision, ResolveDecision::PushWins);
    }
}
