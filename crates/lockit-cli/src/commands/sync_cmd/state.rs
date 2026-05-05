use anyhow::Context;
use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::SyncCheckpoint;
use lockit_core::vault::VaultPaths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn config_path(paths: &VaultPaths) -> PathBuf {
    let mut p = paths.vault_path.clone();
    p.set_file_name("sync_config.json");
    p
}

fn state_path(paths: &VaultPaths) -> PathBuf {
    let mut p = paths.vault_path.clone();
    p.set_file_name("sync_state.json");
    p
}

pub fn load_sync_config(paths: &VaultPaths) -> Option<GoogleDriveConfig> {
    let data = std::fs::read_to_string(config_path(paths)).ok()?;
    serde_json::from_str::<GoogleDriveConfig>(&data).ok()
}

pub fn empty_sync_config() -> GoogleDriveConfig {
    GoogleDriveConfig {
        access_token: String::new(),
        refresh_token: String::new(),
        token_expiry: 0,
        client_id: lockit_core::sync::oauth::google_client_id(),
        client_secret: lockit_core::sync::oauth::google_client_secret(),
        sync_key: None,
    }
}

pub fn save_config(paths: &VaultPaths, config: &GoogleDriveConfig) -> anyhow::Result<()> {
    let cfg_path = config_path(paths);
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&cfg_path, json)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncStateFile {
    last_local_checksum: String,
    last_cloud_checksum: String,
    last_sync_unix_seconds: u64,
}

pub fn load_checkpoint(paths: &VaultPaths) -> Option<SyncCheckpoint> {
    let data = std::fs::read_to_string(state_path(paths)).ok()?;
    let state = serde_json::from_str::<SyncStateFile>(&data).ok()?;
    Some(SyncCheckpoint {
        local_checksum: state.last_local_checksum,
        cloud_checksum: state.last_cloud_checksum,
    })
}

pub fn save_checkpoint(paths: &VaultPaths, checkpoint: SyncCheckpoint) -> anyhow::Result<()> {
    let path = state_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let state = SyncStateFile {
        last_local_checksum: checkpoint.local_checksum,
        last_cloud_checksum: checkpoint.cloud_checksum,
        last_sync_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let json = serde_json::to_string_pretty(&state).context("Failed to serialize sync state")?;
    std::fs::write(path, json).context("Failed to write sync state")?;
    Ok(())
}
