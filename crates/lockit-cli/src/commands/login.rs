use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::oauth;

use crate::commands::sync_cmd::state::config_path;
use crate::output;

pub fn run(paths: &lockit_core::vault::VaultPaths) -> anyhow::Result<()> {
    eprintln!("Starting Google OAuth login...");
    eprintln!("A browser window will open for authorization.\n");

    let tokens = oauth::start_oauth_flow()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Preserve existing sync key if already configured
    let existing_key = std::fs::read_to_string(config_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str::<GoogleDriveConfig>(&s).ok())
        .and_then(|c| c.sync_key);

    let cfg = GoogleDriveConfig {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_expiry: now + tokens.expires_in,
        client_id: oauth::google_client_id(),
        client_secret: oauth::google_client_secret(),
        sync_key: existing_key,
    };

    let cfg_file = config_path(paths);
    if let Some(parent) = cfg_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&cfg)?;
    std::fs::write(&cfg_file, json)?;

    output::success(&format!(
        "Logged in. Config saved to {}",
        cfg_file.display()
    ));
    Ok(())
}
