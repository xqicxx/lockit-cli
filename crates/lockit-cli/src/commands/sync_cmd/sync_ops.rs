use anyhow::Context;
use lockit_core::sync::{
    plan_smart_sync, sha256_checksum, SmartSyncPlan, SyncBackend, SyncInputs,
};
use lockit_core::vault::VaultPaths;

use super::load_backend;
use super::payload::{materialize_download, prepare_upload};
use super::state::{load_checkpoint, load_sync_config, save_checkpoint};

fn require_backend(
    paths: &VaultPaths,
) -> anyhow::Result<lockit_core::sync::google_drive::GoogleDriveBackend> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
    }
    Ok(backend)
}

/// Smart sync: push if only local changed, pull if only cloud changed, bail on conflict.
pub fn sync(paths: &VaultPaths) -> anyhow::Result<()> {
    let backend = require_backend(paths)?;
    let sync_config = load_sync_config(paths);
    let (upload_bytes, local_checksum, manifest) = prepare_upload(paths, sync_config.as_ref())?;
    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?;

    let plan = plan_smart_sync(SyncInputs {
        local_checksum: local_checksum.clone(),
        cloud_manifest: cloud_manifest.clone(),
        checkpoint: load_checkpoint(paths),
        sync_key_configured: true,
        backend_configured: true,
    });

    match plan {
        SmartSyncPlan::AlreadyUpToDate => crate::output::success("Already up to date."),
        SmartSyncPlan::Push => {
            backend
                .upload_vault(&upload_bytes, &manifest)
                .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))?;
            save_checkpoint(paths, &local_checksum, &manifest.vault_checksum)?;

            // Upload a timestamped backup for Android restore visibility (best-effort)
            let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            if let Err(e) = backend.upload_backup(&upload_bytes, &timestamp) {
                eprintln!("Warning: backup file upload failed: {e}");
            }

            persist_config_if_needed(paths, &backend);
            crate::output::success("Vault pushed to cloud.");
        }
        SmartSyncPlan::Pull => {
            let cloud = cloud_manifest.context("No manifest found in cloud")?;
            do_pull(paths, sync_config.as_ref(), &backend, cloud)?;
            crate::output::success("Vault pulled from cloud.");
        }
        SmartSyncPlan::Conflict => anyhow::bail!(
            "Sync conflict: local and cloud both changed. Use 'lockit sync push' or 'lockit sync pull'."
        ),
        SmartSyncPlan::NotConfigured => {
            anyhow::bail!("Google sync is not configured. Run 'lockit login' first.")
        }
        SmartSyncPlan::BackendError => {
            anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.")
        }
    }
    Ok(())
}

/// Force push local vault to cloud.
pub fn push(paths: &VaultPaths) -> anyhow::Result<()> {
    let backend = require_backend(paths)?;
    let sync_config = load_sync_config(paths);
    let (upload_bytes, local_checksum, manifest) = prepare_upload(paths, sync_config.as_ref())?;

    backend
        .upload_vault(&upload_bytes, &manifest)
        .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))?;
    save_checkpoint(paths, &local_checksum, &manifest.vault_checksum)?;

    // Upload a timestamped backup for Android restore visibility (best-effort)
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    if let Err(e) = backend.upload_backup(&upload_bytes, &timestamp) {
        eprintln!("Warning: backup file upload failed: {e}");
    }

    persist_config_if_needed(paths, &backend);
    crate::output::success("Vault pushed to cloud.");
    Ok(())
}

/// Force pull cloud vault to local.
pub fn pull(paths: &VaultPaths) -> anyhow::Result<()> {
    let backend = require_backend(paths)?;
    let sync_config = load_sync_config(paths);

    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?
        .context("No manifest found in cloud")?;

    // Fast path: already up to date (no sync key → can compare checksums)
    if sync_config.as_ref().and_then(|c| c.sync_key.as_ref()).is_none()
        && paths.vault_path.exists()
    {
        let local_bytes =
            std::fs::read(&paths.vault_path).context("Failed to read local vault file")?;
        if sha256_checksum(&local_bytes) == cloud_manifest.vault_checksum {
            crate::output::success("Already up to date.");
            return Ok(());
        }
    }

    do_pull(paths, sync_config.as_ref(), &backend, cloud_manifest)?;
    crate::output::success("Vault pulled from cloud.");
    Ok(())
}

/// Download cloud vault → decrypt → import into local vault → save checkpoint.
fn do_pull(
    paths: &VaultPaths,
    sync_config: Option<&lockit_core::sync::google_drive::GoogleDriveConfig>,
    backend: &lockit_core::sync::google_drive::GoogleDriveBackend,
    cloud_manifest: lockit_core::sync::SyncManifest,
) -> anyhow::Result<()> {
    let cloud_bytes = backend
        .download_vault()
        .map_err(|e| anyhow::anyhow!("Download failed: {e}"))?;
    let (local_checksum, cloud_checksum) =
        materialize_download(paths, sync_config, &cloud_manifest, cloud_bytes)?;
    save_checkpoint(paths, &local_checksum, &cloud_checksum)?;
    persist_config_if_needed(paths, backend);
    Ok(())
}

fn persist_config_if_needed(
    paths: &VaultPaths,
    backend: &lockit_core::sync::google_drive::GoogleDriveBackend,
) {
    if let Some(cfg) = backend.get_config() {
        if cfg.folder_id.is_some() {
            let cfg_path = super::state::config_path(paths);
            if let Ok(json) = serde_json::to_string_pretty(&cfg) {
                let _ = std::fs::write(&cfg_path, json);
            }
        }
    }
}
