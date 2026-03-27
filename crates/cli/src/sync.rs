//! `lk sync` subcommand handlers.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use lockit_sync::config::BackendConfig;
use lockit_sync::factory::SyncBackendFactory;
use serde::Deserialize;

use crate::args::SyncAction;
use crate::vault::{config_dir, vault_path};

/// Top-level config.toml structure.
#[derive(Debug, Deserialize)]
struct Config {
    sync: Option<BackendConfig>,
}

/// Path to the config file.
fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Load the sync backend from `~/.lockit/config.toml`.
fn load_backend_config() -> Result<BackendConfig> {
    let path = config_path()?;
    if !path.exists() {
        bail!(
            "No sync config found at {}. Create it with a [sync] section.",
            path.display()
        );
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: Config =
        toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))?;
    config
        .sync
        .ok_or_else(|| anyhow::anyhow!("No [sync] section found in {}", path.display()))
}

/// Dispatch a sync subcommand.
pub fn handle_sync(action: &SyncAction) -> Result<()> {
    match action {
        SyncAction::Push => handle_push(),
        SyncAction::Pull => handle_pull(),
        SyncAction::Status => handle_status(),
        SyncAction::Config => handle_config(),
    }
}

fn handle_push() -> Result<()> {
    let cfg = load_backend_config()?;
    let backend = SyncBackendFactory::from_config(cfg)?;
    let vault_path = vault_path()?;

    if !vault_path.exists() {
        bail!("No vault found. Run 'lk init' to create one.");
    }

    let data = std::fs::read(&vault_path)
        .with_context(|| format!("Failed to read vault at {}", vault_path.display()))?;

    // Get local mtime for conflict detection.
    let local_ts = std::fs::metadata(&vault_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        // Fetch remote metadata to check for conflicts and obtain the ETag for
        // optimistic concurrency control (S3 if_match).
        let (remote_ts, remote_etag) = match backend.metadata("vault.lockit").await {
            Ok(meta) => (Some(meta.last_modified), Some(meta.checksum)),
            Err(lockit_sync::error::Error::NotFound { .. }) => (None, None),
            Err(e) => return Err(anyhow::anyhow!("Failed to fetch remote metadata: {e}")),
        };

        // Refuse to overwrite a newer remote (Last-Write-Wins safety guard).
        if matches!((local_ts, remote_ts), (Some(local), Some(remote)) if remote > local) {
            let (local, remote) = (local_ts.unwrap(), remote_ts.unwrap());
            bail!(
                "Conflict: remote vault is newer (remote={remote}, local={local}).\n\
                 Run `lk sync pull` to update your local copy first."
            );
        }

        // Upload, passing the ETag so backends that support conditional writes
        // (S3) can enforce an additional server-side check.
        backend
            .upload_if_match("vault.lockit", &data, remote_etag.as_deref())
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    })?;

    println!(
        "Pushed vault ({} bytes) to {} backend.",
        data.len(),
        backend.backend_name()
    );
    Ok(())
}

fn handle_pull() -> Result<()> {
    let cfg = load_backend_config()?;
    let backend = SyncBackendFactory::from_config(cfg)?;
    let vault_path = vault_path()?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let data = rt.block_on(backend.download("vault.lockit"))?;

    std::fs::write(&vault_path, &data)
        .with_context(|| format!("Failed to write vault to {}", vault_path.display()))?;

    println!(
        "Pulled vault ({} bytes) from {} backend.",
        data.len(),
        backend.backend_name()
    );
    Ok(())
}

fn handle_status() -> Result<()> {
    let cfg = load_backend_config()?;
    let backend = SyncBackendFactory::from_config(cfg)?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let meta = rt.block_on(backend.metadata("vault.lockit"))?;

    println!("Backend:       {}", backend.backend_name());
    println!("Version:       {}", meta.version);
    println!("Size:          {} bytes", meta.size);
    println!("Last modified: {} (unix secs)", meta.last_modified);
    println!("Checksum:      {}", meta.checksum);
    Ok(())
}

fn handle_config() -> Result<()> {
    let path = config_path()?;
    println!("Sync config path: {}", path.display());
    if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        println!("{}", raw);
    } else {
        println!("(file does not exist)");
    }
    Ok(())
}
