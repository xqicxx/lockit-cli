use crate::output;
use std::path::PathBuf;

fn config_path(vault_dir: &PathBuf) -> PathBuf {
    vault_dir.join("sync_config.json")
}

pub fn run(paths: &lockit_core::vault::VaultPaths) -> anyhow::Result<()> {
    let vault_dir = paths
        .vault_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let cfg_file = config_path(&vault_dir.to_path_buf());

    if cfg_file.exists() {
        std::fs::remove_file(&cfg_file)?;
        output::success("Logged out. Sync config removed.");
    } else {
        println!("Already logged out.");
    }
    Ok(())
}
