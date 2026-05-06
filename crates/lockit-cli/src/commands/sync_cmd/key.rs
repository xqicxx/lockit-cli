use anyhow::Context;
use lockit_core::vault::VaultPaths;
use zeroize::Zeroize;

use super::state::{empty_sync_config, load_sync_config, save_config};

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
