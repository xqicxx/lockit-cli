use anyhow::Context;
use lockit_core::sync::google_drive::{GoogleDriveBackend, GoogleDriveConfig};
use lockit_core::sync::{
    compute_sync_status, sha256_checksum, SyncBackend, SyncInputs, SyncManifest,
};
use lockit_core::vault::VaultPaths;
use std::path::PathBuf;

fn config_path(paths: &VaultPaths) -> PathBuf {
    let mut p = paths.vault_path.clone();
    p.set_file_name("sync_config.json");
    p
}

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

fn load_sync_config(paths: &VaultPaths) -> Option<GoogleDriveConfig> {
    let cfg_path = config_path(paths);
    if cfg_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cfg_path) {
            return serde_json::from_str::<GoogleDriveConfig>(&data).ok();
        }
    }
    None
}

fn save_config(paths: &VaultPaths, config: &GoogleDriveConfig) -> anyhow::Result<()> {
    let cfg_path = config_path(paths);
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&cfg_path, json)?;
    Ok(())
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
        last_sync_checksum: None,
        sync_key_configured: true,
        backend_configured: true,
    };

    let status = compute_sync_status(input);
    println!("Status: {status:?}");
    Ok(())
}

pub fn push(paths: &VaultPaths, pw: Option<String>) -> anyhow::Result<()> {
    let backend = load_backend(paths);
    if !backend.is_configured() {
        anyhow::bail!("Sync backend not configured. Run 'lockit sync config' first.");
    }

    // Unlock vault to validate password
    let _pw = crate::utils::read_password(pw, "Master password")?;
    let _session =
        lockit_core::vault::unlock_vault(paths, &_pw).context("Failed to unlock vault")?;

    let vault_bytes = std::fs::read(&paths.vault_path).context("Failed to read vault file")?;

    // Encrypt with sync key if configured (cross-platform compatible)
    let sync_config = load_sync_config(paths);
    let upload_bytes = if let Some(ref key_b64) = sync_config.and_then(|c| c.sync_key) {
        let sync_key = lockit_core::sync::SyncCrypto::decode_key(key_b64)
            .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;
        lockit_core::sync::SyncCrypto::encrypt(&vault_bytes, &sync_key)
            .map_err(|e| anyhow::anyhow!("Sync encryption failed: {e}"))?
    } else {
        vault_bytes
    };

    let checksum = sha256_checksum(&upload_bytes);
    let encrypted_size = upload_bytes.len() as u64;

    let manifest = SyncManifest::new(checksum, "lockit-cli", encrypted_size, 2);

    backend
        .upload_vault(&upload_bytes, &manifest)
        .map_err(|e| anyhow::anyhow!("Upload failed: {e}"))?;

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

    // Early-return if already up to date
    if paths.vault_path.exists() {
        let local_bytes =
            std::fs::read(&paths.vault_path).context("Failed to read local vault file")?;
        let local_checksum = if let Some(ref key) = sync_key {
            let encrypted = lockit_core::sync::SyncCrypto::encrypt(&local_bytes, key)
                .context("Failed to encrypt for checksum comparison")?;
            sha256_checksum(&encrypted)
        } else {
            sha256_checksum(&local_bytes)
        };
        if local_checksum == *cloud_checksum {
            crate::output::success("Already up to date.");
            return Ok(());
        }
    }

    // Validate password before network download
    if let Some(p) = pw {
        lockit_core::vault::unlock_vault(paths, &p).context("Failed to unlock vault")?;
    }

    let cloud_bytes = backend
        .download_vault()
        .map_err(|e| anyhow::anyhow!("Download failed: {e}"))?;

    // Verify checksum
    let downloaded_checksum = sha256_checksum(&cloud_bytes);
    if downloaded_checksum != *cloud_checksum {
        anyhow::bail!(
            "Checksum mismatch: downloaded data does not match manifest. Expected {}, got {}",
            cloud_checksum,
            downloaded_checksum
        );
    }

    // Decrypt with sync key if configured
    let vault_bytes = if let Some(ref key) = sync_key {
        lockit_core::sync::SyncCrypto::decrypt(&cloud_bytes, key)
            .map_err(|e| anyhow::anyhow!("Sync decryption failed: {e}"))?
    } else {
        cloud_bytes
    };

    std::fs::write(&paths.vault_path, &vault_bytes).context("Failed to write vault file")?;

    crate::output::success("Vault pulled from cloud.");
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
        sync_key: None,
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
    let key = lockit_core::sync::SyncCrypto::generate_key();
    let encoded = lockit_core::sync::SyncCrypto::encode_key(&key);

    let mut config = load_sync_config(paths).unwrap_or(GoogleDriveConfig {
        access_token: String::new(),
        refresh_token: String::new(),
        token_expiry: 0,
        client_id: String::new(),
        client_secret: String::new(),
        sync_key: None,
    });
    config.sync_key = Some(encoded.clone());
    save_config(paths, &config)?;

    println!("Sync key generated and saved.");
    println!();
    println!("Key (Base64): {encoded}");
    println!();
    println!("Share this key with other devices to sync the same vault.");
    println!("Keep it secret — anyone with this key can decrypt your synced data.");

    Ok(())
}
