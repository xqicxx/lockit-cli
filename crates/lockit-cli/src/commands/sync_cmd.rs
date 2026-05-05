use anyhow::Context;
mod payload;
mod state;

use lockit_core::sync::google_drive::{GoogleDriveBackend, GoogleDriveConfig};
use lockit_core::sync::{
    compute_sync_status, plan_smart_sync, sha256_checksum, SmartSyncPlan, SyncBackend,
    SyncCheckpoint, SyncInputs,
};
use lockit_core::vault::VaultPaths;
use payload::{materialize_download, prepare_upload};
use state::{
    config_path, empty_sync_config, load_checkpoint, load_sync_config, save_checkpoint, save_config,
};
use zeroize::Zeroize;

fn load_backend(paths: &VaultPaths) -> GoogleDriveBackend {
    let mut backend = GoogleDriveBackend::new();
    let cfg_path = config_path(paths);
    if cfg_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cfg_path) {
            if let Ok(config) = serde_json::from_str::<GoogleDriveConfig>(&data) {
                backend.configure(config);
            }
        }
    }
    backend
}

pub fn status(paths: &VaultPaths) -> anyhow::Result<()> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        println!("Status: Not configured");
        return Ok(());
    }

    if !paths.vault_path.exists() {
        crate::output::error("Vault not initialized. Run 'lockit init' first.");
        return Ok(());
    }

    let vault_bytes = std::fs::read(&paths.vault_path).context("Failed to read vault file")?;
    let local_checksum = sha256_checksum(&vault_bytes);

    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?;

    let input = SyncInputs {
        local_checksum,
        cloud_manifest,
        checkpoint: load_checkpoint(paths),
        sync_key_configured: true,
        backend_configured: true,
    };

    let status = compute_sync_status(input);
    println!("Status: {status:?}");
    Ok(())
}

pub fn sync(paths: &VaultPaths, pw: Option<String>) -> anyhow::Result<()> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
    }

    let sync_config = load_sync_config(paths);
    let prepared = prepare_upload(paths, pw.clone(), sync_config.as_ref(), "lockit-cli")?;
    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?;

    let plan = plan_smart_sync(SyncInputs {
        local_checksum: prepared.local_checksum.clone(),
        cloud_manifest: cloud_manifest.clone(),
        checkpoint: load_checkpoint(paths),
        sync_key_configured: true,
        backend_configured: true,
    });

    match plan {
        SmartSyncPlan::AlreadyUpToDate => {
            crate::output::success("Already up to date.");
        }
        SmartSyncPlan::Push => {
            backend
                .upload_vault(&prepared.upload_bytes, &prepared.manifest)
                .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))?;
            save_checkpoint(
                paths,
                SyncCheckpoint {
                    local_checksum: prepared.local_checksum,
                    cloud_checksum: prepared.manifest.vault_checksum,
                },
            )?;
            crate::output::success("Vault pushed to cloud.");
        }
        SmartSyncPlan::Pull => {
            let manifest = cloud_manifest.context("No manifest found in cloud")?;
            pull_manifest(paths, pw, sync_config.as_ref(), &backend, manifest)?;
            crate::output::success("Vault pulled from cloud.");
        }
        SmartSyncPlan::Conflict => {
            anyhow::bail!(
                "Sync conflict: local and cloud both changed. Use 'lockit sync push' to overwrite cloud or 'lockit sync pull' to overwrite local."
            );
        }
        SmartSyncPlan::NotConfigured => {
            anyhow::bail!("Sync key not configured. Run 'lockit sync key-gen' or 'lockit sync key-set' first.");
        }
        SmartSyncPlan::BackendError => {
            anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
        }
    }

    Ok(())
}

pub fn push(paths: &VaultPaths, pw: Option<String>) -> anyhow::Result<()> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
    }

    let sync_config = load_sync_config(paths);
    let prepared = prepare_upload(paths, pw, sync_config.as_ref(), "lockit-cli")?;

    backend
        .upload_vault(&prepared.upload_bytes, &prepared.manifest)
        .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))?;
    save_checkpoint(
        paths,
        SyncCheckpoint {
            local_checksum: prepared.local_checksum,
            cloud_checksum: prepared.manifest.vault_checksum,
        },
    )?;

    crate::output::success("Vault pushed to cloud.");
    Ok(())
}

pub fn pull(paths: &VaultPaths, pw: Option<String>) -> anyhow::Result<()> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
    }

    let sync_config = load_sync_config(paths);
    let sync_key: Option<[u8; 32]> = sync_config
        .as_ref()
        .and_then(|c| c.sync_key.as_ref())
        .map(|k| {
            lockit_core::sync::SyncCrypto::decode_key(k)
                .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))
        })
        .transpose()?;

    // Fetch manifest first — cheap metadata, no large download yet
    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?
        .context("No manifest found in cloud")?;
    let cloud_checksum = &cloud_manifest.vault_checksum;

    // Early-return if already up to date (non-sync-key only — sync key random nonce prevents matching)
    if paths.vault_path.exists() && sync_key.is_none() {
        let local_bytes =
            std::fs::read(&paths.vault_path).context("Failed to read local vault file")?;
        let local_checksum = sha256_checksum(&local_bytes);
        if local_checksum == *cloud_checksum {
            crate::output::success("Already up to date.");
            return Ok(());
        }
    }

    // Validate password before network download
    if let Some(ref p) = pw {
        lockit_core::vault::unlock_vault(paths, &p).context("Failed to unlock vault")?;
    }

    pull_manifest(paths, pw, sync_config.as_ref(), &backend, cloud_manifest)?;

    crate::output::success("Vault pulled from cloud.");
    Ok(())
}

fn pull_manifest(
    paths: &VaultPaths,
    pw: Option<String>,
    sync_config: Option<&GoogleDriveConfig>,
    backend: &GoogleDriveBackend,
    cloud_manifest: lockit_core::sync::SyncManifest,
) -> anyhow::Result<()> {
    let cloud_bytes = backend
        .download_vault()
        .map_err(|e| anyhow::anyhow!("Download failed: {e}"))?;
    let pulled = materialize_download(pw, sync_config, &cloud_manifest, cloud_bytes)?;
    std::fs::write(&paths.vault_path, &pulled.local_bytes).context("Failed to write vault file")?;
    save_checkpoint(
        paths,
        SyncCheckpoint {
            local_checksum: pulled.local_checksum,
            cloud_checksum: pulled.cloud_checksum,
        },
    )?;
    Ok(())
}

pub fn config(paths: &VaultPaths) -> anyhow::Result<()> {
    println!("Google Drive OAuth Configuration");
    println!("-------------------------------");
    println!("You need to create a Google Cloud project and enable the Drive API.");
    println!("Then create an OAuth 2.0 Client ID (Desktop application).");
    println!();

    let client_id = inquire::Text::new("Client ID:")
        .prompt()
        .context("Failed to read client_id")?;

    let client_secret = inquire::Password::new("Client secret:")
        .without_confirmation()
        .prompt()
        .context("Failed to read client_secret")?;

    let refresh_token = inquire::Password::new("Refresh token:")
        .without_confirmation()
        .prompt()
        .context("Failed to read refresh_token")?;

    let access_token = inquire::Password::new("Access token:")
        .without_confirmation()
        .prompt()
        .context("Failed to read access_token")?;

    let config = GoogleDriveConfig {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
        refresh_token: refresh_token.trim().to_string(),
        access_token: access_token.trim().to_string(),
        token_expiry: 0,
        sync_key: load_sync_config(paths).and_then(|c| c.sync_key),
    };

    // Save config to file next to vault
    save_config(paths, &config)?;

    let cfg_path = config_path(paths);
    crate::output::success(&format!(
        "Sync configuration saved to {}",
        cfg_path.display()
    ));

    Ok(())
}

pub fn key_gen(paths: &VaultPaths) -> anyhow::Result<()> {
    let mut key = lockit_core::sync::SyncCrypto::generate_key();
    let encoded = lockit_core::sync::SyncCrypto::encode_key(&key);
    key.zeroize();

    let mut config = load_sync_config(paths).unwrap_or_else(empty_sync_config);
    config.sync_key = Some(encoded);
    save_config(paths, &config)?;

    crate::output::success("Sync key generated and saved.");
    println!("Use 'lockit sync key-show' to reveal the key for cross-platform setup.");

    Ok(())
}

pub fn key_show(paths: &VaultPaths) -> anyhow::Result<()> {
    let config =
        load_sync_config(paths).ok_or_else(|| anyhow::anyhow!("No sync configuration found."))?;
    let key = config.sync_key.ok_or_else(|| {
        anyhow::anyhow!("No sync key configured. Run 'lockit sync key-gen' first.")
    })?;
    println!("{key}");
    Ok(())
}

pub fn key_set(paths: &VaultPaths) -> anyhow::Result<()> {
    let mut key =
        rpassword::prompt_password("Sync key (Base64): ").context("Failed to read sync key")?;

    lockit_core::sync::SyncCrypto::decode_key(key.trim())
        .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;

    let mut config = load_sync_config(paths).unwrap_or_else(empty_sync_config);
    config.sync_key = Some(key.trim().to_string());
    let result = save_config(paths, &config);
    key.zeroize();
    result?;

    crate::output::success("Sync key configured.");
    println!("Push and pull will now use this key for cross-platform sync.");

    Ok(())
}
