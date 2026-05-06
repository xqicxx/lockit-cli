use anyhow::Context;
use lockit_core::sync::{compute_sync_status, sha256_checksum, SyncBackend, SyncInputs};
use lockit_core::vault::VaultPaths;

use super::state::load_checkpoint;
use super::load_backend;

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
