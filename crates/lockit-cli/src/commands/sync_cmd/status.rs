use lockit_core::sync::{compute_sync_status, sha256_checksum, SyncBackend, SyncInputs};
use lockit_core::vault::VaultPaths;

use super::load_backend;
use super::state::load_checkpoint;

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

    let local_bytes = std::fs::read(&paths.vault_path)
        .map_err(|e| anyhow::anyhow!("Failed to read vault: {e}"))?;
    let local_checksum = sha256_checksum(&local_bytes);

    let cloud_manifest = backend
        .get_manifest()
        .map_err(|e| anyhow::anyhow!("Failed to fetch cloud manifest: {e}"))?;

    let status = compute_sync_status(SyncInputs {
        local_checksum,
        cloud_manifest,
        checkpoint: load_checkpoint(paths),
        sync_key_configured: true,
        backend_configured: true,
    });
    println!("Status: {status:?}");
    Ok(())
}
