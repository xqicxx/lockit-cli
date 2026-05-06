use anyhow::Context;
use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::vault::VaultPaths;

use super::state::{config_path, load_sync_config, save_config};

pub fn config(paths: &VaultPaths) -> anyhow::Result<()> {
    println!("Google Drive OAuth Configuration");
    println!("-------------------------------");
    println!("You need to create a Google Cloud project and enable the Drive API.");
    println!("Then create an OAuth 2.0 Client ID (Desktop application).");
    println!();

    let client_id = inquire::Text::new("Client ID:")
        .prompt()
        .context("Failed to read client_id")?;

    let client_secret = inquire::Password::new("Client secret:")
        .without_confirmation()
        .prompt()
        .context("Failed to read client_secret")?;

    let refresh_token = inquire::Password::new("Refresh token:")
        .without_confirmation()
        .prompt()
        .context("Failed to read refresh_token")?;

    let access_token = inquire::Password::new("Access token:")
        .without_confirmation()
        .prompt()
        .context("Failed to read access_token")?;

    let config = GoogleDriveConfig {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
        refresh_token: refresh_token.trim().to_string(),
        access_token: access_token.trim().to_string(),
        token_expiry: 0,
        sync_key: load_sync_config(paths).and_then(|c| c.sync_key),
    };

    save_config(paths, &config)?;

    let cfg_path = config_path(paths);
    crate::output::success(&format!(
        "Sync configuration saved to {}",
        cfg_path.display()
    ));

    Ok(())
}
