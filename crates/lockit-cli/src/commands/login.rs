use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::oauth;
use std::path::{Path, PathBuf};

use crate::output;

fn config_path(vault_dir: &Path) -> PathBuf {
    vault_dir.join("sync_config.json")
}

pub fn run(paths: &lockit_core::vault::VaultPaths) -> anyhow::Result<()> {
    eprintln!("Starting Google OAuth login...");
    eprintln!("A browser window will open for authorization.\n");

    let tokens =
        oauth::start_oauth_flow().map_err(|e| anyhow::anyhow!("OAuth flow failed: {e}"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cfg = GoogleDriveConfig {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_expiry: now + tokens.expires_in,
        client_id: oauth::GOOGLE_CLIENT_ID.to_string(),
        client_secret: String::new(),
    };

    let vault_dir = paths
        .vault_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let cfg_file = config_path(vault_dir);
    std::fs::create_dir_all(vault_dir)?;
    let json = serde_json::to_string_pretty(&cfg)?;
    std::fs::write(&cfg_file, json)?;

    output::success(&format!(
        "Logged in. Config saved to {}",
        cfg_file.display()
    ));
    Ok(())
}
