use crate::commands::sync_cmd::state::config_path;
use crate::output;
use lockit_core::sync::google_drive::GoogleDriveConfig;

pub fn run(paths: &lockit_core::vault::VaultPaths) -> anyhow::Result<()> {
    let cfg_file = config_path(paths);

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
