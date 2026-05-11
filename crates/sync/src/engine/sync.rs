//! `SmartSyncEngine` — high-level sync orchestration with conflict detection.
//!
//! Mirrors the Android `VaultSyncEngine`:
//! - Pushes/pulls encrypted vault + manifest to Google Drive
//! - Uses `appDataFolder` storage (same as Android)
//! - Sync Key encrypts vault before upload

use crate::backend::{SyncBackend, SyncMetadata};
use crate::engine::conflict::{
    ConflictDetector, ResolveDecision, ResolveStrategy, SyncConflict, SyncOutcome,
};
use crate::engine::vault_key::{MANIFEST_FILE, VAULT_FILE, VaultKey};
use crate::error::Error as BackendError;
use crate::manifest::SyncManifest;
use crate::state::SyncState;
use crate::util::sha256_hex;

/// High-level sync engine that composes a [`SyncBackend`] with local state
/// and conflict detection.
pub struct SmartSyncEngine {
    backend: Box<dyn SyncBackend>,
    state: Option<SyncState>,
    vault_path: std::path::PathBuf,
    vault_key: Option<VaultKey>,
}

impl SmartSyncEngine {
    /// Create a new engine without encryption (passthrough mode).
    pub fn new(
        backend: Box<dyn SyncBackend>,
        state: Option<SyncState>,
        vault_path: std::path::PathBuf,
    ) -> Self {
        Self {
            backend,
            state,
            vault_path,
            vault_key: None,
        }
    }

    /// Create a new engine with sync key encryption (compatible with Android).
    pub fn new_with_key(
        backend: Box<dyn SyncBackend>,
        state: Option<SyncState>,
        vault_path: std::path::PathBuf,
        vault_key: VaultKey,
    ) -> Self {
        Self {
            backend,
            state,
            vault_path,
            vault_key: Some(vault_key),
        }
    }

    /// Set or update the vault encryption key.
    pub fn set_vault_key(&mut self, key: VaultKey) {
        self.vault_key = Some(key);
    }

    /// Return the current sync state.
    pub fn state(&self) -> Option<&SyncState> {
        self.state.as_ref()
    }

    /// Get the remote vault checksum without downloading the vault.
    pub async fn cloud_checksum(&self) -> Option<String> {
        self.fetch_remote_manifest()
            .await
            .ok()?
            .map(|m| m.vault_checksum)
    }

    /// Check if the remote vault has changed since last sync.
    pub async fn poll(&self) -> Option<String> {
        let state = self.state.as_ref()?;
        let manifest = self.fetch_remote_manifest().await.ok()??;
        if state.remote_changed(&manifest.vault_checksum, manifest.encrypted_size as u64) {
            Some(manifest.vault_checksum)
        } else {
            None
        }
    }

    /// Fetch remote manifest JSON.
    async fn fetch_remote_manifest(&self) -> Result<Option<SyncManifest>, SyncError> {
        match self.backend.download(MANIFEST_FILE).await {
            Ok(data) => {
                let manifest = serde_json::from_slice::<SyncManifest>(&data)
                    .map_err(|e| SyncError::Backend(format!("Invalid manifest: {e}")))?;
                Ok(Some(manifest))
            }
            Err(BackendError::NotFound { .. }) => Ok(None),
            Err(e) => Err(SyncError::Backend(e.to_string())),
        }
    }

    /// Convert manifest to SyncMetadata for conflict detection.
    fn manifest_to_meta(manifest: &SyncManifest) -> SyncMetadata {
        SyncMetadata {
            version: manifest.version as u64,
            last_modified: chrono::DateTime::parse_from_rfc3339(&manifest.updated_at)
                .ok()
                .map_or(0, |dt| dt.timestamp() as u64),
            checksum: manifest.vault_checksum.clone(),
            size: manifest.encrypted_size as u64,
        }
    }

    /// Read local vault file and compute checksum.
    fn read_local_vault(&self) -> Result<(Vec<u8>, String), SyncError> {
        let data = std::fs::read(&self.vault_path)
            .map_err(|e| SyncError::Io(format!("Failed to read vault: {e}")))?;
        let checksum = sha256_hex(&data);
        Ok((data, checksum))
    }

    /// Save sync state internally.
    fn record_sync(
        &mut self,
        local_checksum: String,
        remote_checksum: String,
        remote_size: u64,
    ) -> SyncState {
        let new_state = SyncState::new(local_checksum, remote_checksum, remote_size);
        self.state = Some(new_state.clone());
        new_state
    }

    /// Encrypt vault data using sync key (if configured).
    fn encrypt_vault(&self, plaintext: &[u8]) -> Result<Vec<u8>, SyncError> {
        if let Some(ref key) = self.vault_key {
            key.encrypt(plaintext)
                .map_err(|e| SyncError::Backend(format!("Encryption failed: {e}")))
        } else {
            Ok(plaintext.to_vec())
        }
    }

    /// Decrypt vault data using sync key (if configured).
    fn decrypt_vault(&self, encrypted: &[u8]) -> Result<Vec<u8>, SyncError> {
        if let Some(ref key) = self.vault_key {
            key.decrypt(encrypted)
                .map_err(|e| SyncError::Backend(format!("Decryption failed: {e}")))
        } else {
            Ok(encrypted.to_vec())
        }
    }

    /// Build a manifest for upload.
    fn build_manifest(&self, encrypted: &[u8], device_id: &str) -> SyncManifest {
        let checksum = format!("sha256:{}", sha256_hex(encrypted));
        SyncManifest::new(checksum, device_id.to_string(), encrypted.len() as i64)
    }

    /// Upload vault + manifest atomically.
    async fn encrypt_and_upload(
        &mut self,
        plaintext: Vec<u8>,
        local_checksum: String,
    ) -> Result<SyncOutcome, SyncError> {
        let encrypted = self.encrypt_vault(&plaintext)?;
        let manifest = self.build_manifest(&encrypted, "cli");
        let manifest_json = serde_json::to_string(&manifest)
            .map_err(|e| SyncError::Backend(format!("Manifest serialization: {e}")))?;

        // Upload vault.enc
        self.backend
            .upload(VAULT_FILE, &encrypted)
            .await
            .map_err(|e| SyncError::Backend(e.to_string()))?;

        // Upload manifest.json
        self.backend
            .upload(MANIFEST_FILE, manifest_json.as_bytes())
            .await
            .map_err(|e| SyncError::Backend(e.to_string()))?;

        // Upload a timestamped backup for Android restore visibility (best-effort)
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let backup_key = format!("vault_{timestamp}.enc");
        let _ = self.backend.upload(&backup_key, &encrypted).await;

        self.record_sync(
            local_checksum,
            manifest.vault_checksum.clone(),
            manifest.encrypted_size as u64,
        );
        Ok(SyncOutcome::Pushed)
    }

    /// Download and decrypt vault from remote.
    async fn download_and_decrypt(
        &mut self,
        manifest: &SyncManifest,
    ) -> Result<SyncOutcome, SyncError> {
        let encrypted = self
            .backend
            .download(VAULT_FILE)
            .await
            .map_err(|e| SyncError::Backend(e.to_string()))?;

        // Verify checksum against manifest
        let downloaded_checksum = format!("sha256:{}", sha256_hex(&encrypted));
        if downloaded_checksum != manifest.vault_checksum {
            return Err(SyncError::ChecksumMismatch {
                expected: manifest.vault_checksum.clone(),
                actual: downloaded_checksum,
            });
        }

        let plaintext = self.decrypt_vault(&encrypted)?;

        let backup_path = self.vault_path.with_extension("lockit.pull_backup");
        replace_file_with_backup(&self.vault_path, &backup_path, |path| {
            std::fs::write(path, &plaintext)
        })
        .map_err(|e| SyncError::Io(format!("Failed to replace vault: {e}")))?;

        let local_checksum = sha256_hex(&plaintext);
        self.record_sync(
            local_checksum,
            manifest.vault_checksum.clone(),
            manifest.encrypted_size as u64,
        );
        Ok(SyncOutcome::Pulled)
    }

    // ── Push ──────────────────────────────────────────────────────────

    /// Push local vault to remote.  Fails with `SyncConflict` if both
    /// local and remote changed since last sync.
    pub async fn push(&mut self) -> Result<SyncOutcome, SyncError> {
        let (data, local_checksum) = self.read_local_vault()?;
        let remote = self.fetch_remote_manifest().await?;

        if let Some(ref manifest) = remote {
            let remote_meta = Self::manifest_to_meta(manifest);
            if let Some(conflict) = ConflictDetector::check_push_conflict(
                &local_checksum,
                &remote_meta,
                self.state.as_ref(),
            ) {
                return Err(SyncError::Conflict(conflict));
            }
        }

        self.encrypt_and_upload(data, local_checksum).await
    }

    /// Push with conflict resolution strategy.
    pub async fn push_with_strategy(
        &mut self,
        strategy: ResolveStrategy,
    ) -> Result<SyncOutcome, SyncError> {
        let (data, local_checksum) = self.read_local_vault()?;
        let remote = self.fetch_remote_manifest().await?;

        match remote {
            None => self.encrypt_and_upload(data, local_checksum).await,
            Some(ref manifest) => {
                let remote_meta = Self::manifest_to_meta(manifest);
                match ConflictDetector::check_push_conflict(
                    &local_checksum,
                    &remote_meta,
                    self.state.as_ref(),
                ) {
                    None => self.encrypt_and_upload(data, local_checksum).await,
                    Some(_) => match strategy {
                        ResolveStrategy::KeepLocal => {
                            self.encrypt_and_upload(data, local_checksum).await
                        }
                        ResolveStrategy::KeepRemote => Ok(SyncOutcome::AlreadyUpToDate),
                        ResolveStrategy::LastWriteWins => {
                            let decision = ConflictDetector::resolve_last_write_wins(
                                remote_meta.last_modified,
                                now_secs(),
                            );
                            match decision {
                                ResolveDecision::PushWins => {
                                    self.encrypt_and_upload(data, local_checksum).await
                                }
                                ResolveDecision::PullWins => {
                                    self.download_and_decrypt(manifest).await
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    // ── Pull ──────────────────────────────────────────────────────────

    /// Pull remote vault to local.  Fails with `SyncConflict` if local
    /// changed since last sync.
    pub async fn pull(&mut self) -> Result<SyncOutcome, SyncError> {
        let remote = self.fetch_remote_manifest().await?;
        let Some(manifest) = remote else {
            return Err(SyncError::Backend("No remote vault exists".into()));
        };

        let local_checksum = if self.vault_path.exists() {
            let data = std::fs::read(&self.vault_path)
                .map_err(|e| SyncError::Io(format!("Failed to read vault: {e}")))?;
            sha256_hex(&data)
        } else {
            String::new()
        };

        if let Some(conflict) =
            ConflictDetector::check_pull_conflict(&local_checksum, self.state.as_ref())
        {
            return Err(SyncError::Conflict(conflict));
        }

        self.download_and_decrypt(&manifest).await
    }

    /// Pull with conflict resolution strategy.
    pub async fn pull_with_strategy(
        &mut self,
        strategy: ResolveStrategy,
    ) -> Result<SyncOutcome, SyncError> {
        let remote = self.fetch_remote_manifest().await?;
        let Some(manifest) = remote else {
            return Err(SyncError::Backend("No remote vault exists".into()));
        };

        let local_checksum = if self.vault_path.exists() {
            let data = std::fs::read(&self.vault_path)
                .map_err(|e| SyncError::Io(format!("Failed to read vault: {e}")))?;
            sha256_hex(&data)
        } else {
            String::new()
        };

        match ConflictDetector::check_pull_conflict(&local_checksum, self.state.as_ref()) {
            None => self.download_and_decrypt(&manifest).await,
            Some(_) => match strategy {
                ResolveStrategy::KeepRemote => self.download_and_decrypt(&manifest).await,
                ResolveStrategy::KeepLocal => Ok(SyncOutcome::AlreadyUpToDate),
                ResolveStrategy::LastWriteWins => {
                    let remote_ts = chrono::DateTime::parse_from_rfc3339(&manifest.updated_at)
                        .ok()
                        .map_or(0, |dt| dt.timestamp() as u64);
                    let decision = ConflictDetector::resolve_last_write_wins(remote_ts, now_secs());
                    match decision {
                        ResolveDecision::PullWins => self.download_and_decrypt(&manifest).await,
                        ResolveDecision::PushWins => Ok(SyncOutcome::AlreadyUpToDate),
                    }
                }
            },
        }
    }

    // ── Smart Sync ────────────────────────────────────────────────────

    /// Bidirectional sync: automatically pushes or pulls as needed.
    pub async fn sync(&mut self) -> Result<SyncOutcome, SyncError> {
        self.sync_with_strategy(ResolveStrategy::LastWriteWins)
            .await
    }

    /// Bidirectional sync with configurable conflict resolution.
    pub async fn sync_with_strategy(
        &mut self,
        strategy: ResolveStrategy,
    ) -> Result<SyncOutcome, SyncError> {
        let (local_data, local_checksum) = match self.read_local_vault() {
            Ok(v) => v,
            Err(_) => {
                let remote = self.fetch_remote_manifest().await?;
                let Some(manifest) = remote else {
                    return Err(SyncError::Backend("No local or remote vault exists".into()));
                };
                return self.download_and_decrypt(&manifest).await;
            }
        };

        let remote = self.fetch_remote_manifest().await?;
        let Some(manifest) = remote else {
            return self.encrypt_and_upload(local_data, local_checksum).await;
        };

        let Some(ref state) = self.state else {
            return Ok(SyncOutcome::NeedsBaseline);
        };

        let local_changed = state.local_changed(&local_checksum);
        let remote_changed =
            state.remote_changed(&manifest.vault_checksum, manifest.encrypted_size as u64);

        match (local_changed, remote_changed) {
            (false, false) => Ok(SyncOutcome::AlreadyUpToDate),
            (true, false) => self.push_with_strategy(strategy).await,
            (false, true) => self.pull_with_strategy(strategy).await,
            (true, true) => match strategy {
                ResolveStrategy::KeepLocal => {
                    self.encrypt_and_upload(local_data, local_checksum).await
                }
                ResolveStrategy::KeepRemote => self.download_and_decrypt(&manifest).await,
                ResolveStrategy::LastWriteWins => {
                    let remote_ts = chrono::DateTime::parse_from_rfc3339(&manifest.updated_at)
                        .ok()
                        .map_or(0, |dt| dt.timestamp() as u64);
                    let decision = ConflictDetector::resolve_last_write_wins(remote_ts, now_secs());
                    match decision {
                        ResolveDecision::PushWins => {
                            self.encrypt_and_upload(local_data, local_checksum).await
                        }
                        ResolveDecision::PullWins => self.download_and_decrypt(&manifest).await,
                    }
                }
            },
        }
    }

    /// Return the backend name for display.
    pub fn backend_name(&self) -> &str {
        self.backend.backend_name()
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn replace_file_with_backup<F>(
    path: &std::path::Path,
    backup_path: &std::path::Path,
    write: F,
) -> std::io::Result<()>
where
    F: FnOnce(&std::path::Path) -> std::io::Result<()>,
{
    let had_existing = path.exists();
    if had_existing {
        std::fs::copy(path, backup_path)?;
    }

    if let Err(write_err) = write(path) {
        if had_existing {
            let restore_result = std::fs::copy(backup_path, path);
            let _ = std::fs::remove_file(backup_path);
            restore_result?;
        }
        return Err(write_err);
    }

    if had_existing {
        std::fs::remove_file(backup_path)?;
    }
    Ok(())
}

/// Sync error type covering all failure modes.
#[derive(Debug)]
pub enum SyncError {
    Conflict(SyncConflict),
    Backend(String),
    Io(String),
    ChecksumMismatch { expected: String, actual: String },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Conflict(c) => write!(f, "{c}"),
            SyncError::Backend(msg) => write!(f, "Backend error: {msg}"),
            SyncError::Io(msg) => write!(f, "I/O error: {msg}"),
            SyncError::ChecksumMismatch { expected, actual } => {
                write!(f, "Checksum mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for SyncError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mock::MockBackend;
    use crate::engine::vault_key::VaultKey;

    struct TestCtx {
        engine: SmartSyncEngine,
        _dir: tempfile::TempDir,
    }

    fn new_engine(state: Option<SyncState>) -> TestCtx {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.lockit");
        std::fs::write(&vault_path, b"local vault").unwrap();
        let engine = SmartSyncEngine::new(Box::new(MockBackend::new()), state, vault_path);
        TestCtx { engine, _dir: dir }
    }

    fn new_encrypted_engine(state: Option<SyncState>) -> TestCtx {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.lockit");
        std::fs::write(&vault_path, b"local vault").unwrap();
        let key = VaultKey::generate();
        let engine =
            SmartSyncEngine::new_with_key(Box::new(MockBackend::new()), state, vault_path, key);
        TestCtx { engine, _dir: dir }
    }

    #[tokio::test]
    async fn push_when_no_remote_uploads_successfully() {
        let mut ctx = new_engine(None);
        let result = ctx.engine.push().await;
        assert!(
            matches!(result, Ok(SyncOutcome::Pushed)),
            "got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn encrypted_push_decryptable() {
        let mut ctx = new_encrypted_engine(None);
        let result = ctx.engine.push().await;
        assert!(
            matches!(result, Ok(SyncOutcome::Pushed)),
            "got: {:?}",
            result
        );

        // Clear state and overwrite vault to simulate fresh pull
        ctx.engine.state = None;
        std::fs::write(&ctx.engine.vault_path, b"").unwrap();

        let result = ctx.engine.pull().await;
        assert!(
            matches!(result, Ok(SyncOutcome::Pulled)),
            "got: {:?}",
            result
        );

        let data = std::fs::read(&ctx.engine.vault_path).unwrap();
        assert_eq!(data, b"local vault");
    }

    #[tokio::test]
    async fn sync_already_up_to_date() {
        let local_cs = sha256_hex(b"local vault");
        let remote_cs = format!("sha256:{}", sha256_hex(b"local vault"));
        let mut ctx = new_engine(Some(SyncState::new(
            local_cs.clone(),
            remote_cs.clone(),
            11,
        )));
        // Set up manifest
        let manifest = crate::manifest::SyncManifest::new(remote_cs.clone(), "cli".into(), 11);
        let json = serde_json::to_string(&manifest).unwrap();
        ctx.engine
            .backend
            .upload(MANIFEST_FILE, json.as_bytes())
            .await
            .unwrap();
        ctx.engine
            .backend
            .upload(VAULT_FILE, b"local vault")
            .await
            .unwrap();

        let result = ctx.engine.sync().await;
        assert!(
            matches!(result, Ok(SyncOutcome::AlreadyUpToDate)),
            "got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn pull_succeeds_with_valid_state() {
        let mut ctx = new_engine(None);
        let manifest = crate::manifest::SyncManifest::new(
            format!("sha256:{}", sha256_hex(b"remote data")),
            "cli".into(),
            11,
        );
        let json = serde_json::to_string(&manifest).unwrap();
        ctx.engine
            .backend
            .upload(MANIFEST_FILE, json.as_bytes())
            .await
            .unwrap();
        ctx.engine
            .backend
            .upload(VAULT_FILE, b"remote data")
            .await
            .unwrap();

        let result = ctx.engine.pull().await;
        assert!(
            matches!(result, Ok(SyncOutcome::Pulled)),
            "got: {:?}",
            result
        );

        let data = std::fs::read(&ctx.engine.vault_path).unwrap();
        assert_eq!(data, b"remote data");
    }

    #[test]
    fn replace_file_with_backup_restores_original_when_write_fails() {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.lockit");
        let backup_path = dir.path().join("vault.lockit.pull_backup");
        std::fs::write(&vault_path, b"original vault").unwrap();

        let result = replace_file_with_backup(&vault_path, &backup_path, |path| {
            std::fs::write(path, b"partial remote")?;
            Err(std::io::Error::other("simulated write failure"))
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&vault_path).unwrap(), b"original vault");
        assert!(!backup_path.exists());
    }
}
