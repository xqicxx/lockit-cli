use anyhow::Context;
use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::SyncCheckpoint;
use lockit_core::vault::VaultPaths;
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
    serde_json::from_str(&data).ok()
}

pub fn load_checkpoint(paths: &VaultPaths) -> Option<SyncCheckpoint> {
    #[derive(serde::Deserialize)]
    struct StateFile {
        last_local_checksum: String,
        last_cloud_checksum: String,
    }
    let data = std::fs::read_to_string(state_path(paths)).ok()?;
    let state = serde_json::from_str::<StateFile>(&data).ok()?;
    Some(SyncCheckpoint {
        local_checksum: state.last_local_checksum,
        cloud_checksum: state.last_cloud_checksum,
    })
}

pub fn save_checkpoint(
    paths: &VaultPaths,
    local_checksum: &str,
    cloud_checksum: &str,
) -> anyhow::Result<()> {
    let path = state_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::json!({
        "last_local_checksum": local_checksum,
        "last_cloud_checksum": cloud_checksum,
        "last_sync_unix_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });
    let pretty = serde_json::to_string_pretty(&json).context("Failed to serialize sync state")?;
    std::fs::write(path, pretty).context("Failed to write sync state")?;
    Ok(())
}
