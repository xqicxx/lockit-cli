use lockit_core::sync::SyncCrypto;
use lockit_core::vault::VaultPaths;

use super::state::{config_path, load_sync_config};

pub fn config(paths: &VaultPaths) -> anyhow::Result<()> {
    let cfg_path = config_path(paths);
    println!("Config path: {}", cfg_path.display());

    if !cfg_path.exists() {
        println!("Status: not logged in");
        println!("Run 'lockit login' to sign in with Google.");
        return Ok(());
    }

    println!("Status: logged in");
    println!("Backend: Google Drive (lockit-sync/)");
    Ok(())
}

pub fn key(paths: &VaultPaths, new_key: Option<String>) -> anyhow::Result<()> {
    let cfg_path = config_path(paths);

    if !cfg_path.exists() {
        anyhow::bail!("Not logged in. Run 'lockit login' first.");
    }

    match new_key {
        Some(k) => set_sync_key(paths, &k),
        None => show_sync_key(paths),
    }
}

fn show_sync_key(paths: &VaultPaths) -> anyhow::Result<()> {
    let cfg = load_sync_config(paths)
        .ok_or_else(|| anyhow::anyhow!("Failed to read sync config"))?;

    match &cfg.sync_key {
        Some(k) => {
            // Validate it's a real key
            match SyncCrypto::decode_key(k) {
                Ok(_) => println!("Sync key: {} (valid)", &k[..8.min(k.len())]),
                Err(_) => println!("Sync key: {} (INVALID)", &k[..8.min(k.len())]),
            }
        }
        None => println!("Sync key: not set (payloads uploaded unencrypted)"),
    }
    Ok(())
}

fn set_sync_key(paths: &VaultPaths, key: &str) -> anyhow::Result<()> {
    // Validate key
    SyncCrypto::decode_key(key)
        .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;

    let cfg_path = config_path(paths);
    let mut cfg = load_sync_config(paths)
        .ok_or_else(|| anyhow::anyhow!("Failed to read sync config"))?;

    cfg.sync_key = Some(key.to_string());
    let json = serde_json::to_string_pretty(&cfg)?;
    std::fs::write(&cfg_path, json)?;
    println!("Sync key updated.");
    Ok(())
}
