use crate::output;
use lockit_core::sync::google_drive::GoogleDriveConfig;
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

    if !cfg_file.exists() {
        println!("Status: not logged in (no sync config)");
        println!("Run 'lockit login' to sign in with Google.");
        return Ok(());
    }

    let content = std::fs::read_to_string(&cfg_file)?;
    let cfg: GoogleDriveConfig = serde_json::from_str(&content)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_in = cfg.token_expiry - now;

    println!("Vault: {}", paths.vault_path.display());
    println!("Backend: Google Drive (appDataFolder)");

    if expires_in > 0 {
        let mins = expires_in / 60;
        output::success(&format!("Access token valid for ~{mins} minutes"));
    } else {
        output::error("Access token expired. Run 'lockit login' to refresh.");
    }

    if !cfg.refresh_token.is_empty() {
        println!("Refresh token: present");
    }
    Ok(())
}
