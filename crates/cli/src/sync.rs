//! `lk sync` subcommand handlers — Google Drive only.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use lockit_sync::auth::{login as google_login, token};
use lockit_sync::backends::google_drive::GoogleDriveSyncConfig;
use lockit_sync::config::{GoogleDriveConfig, GoogleTokenStore, load_tokens, save_tokens};
use lockit_sync::conflict::ResolveStrategy;
use lockit_sync::engine::vault_key::VaultKey;
use lockit_sync::engine::{SmartSyncEngine, SyncError};
use lockit_sync::factory::SyncBackendFactory;
use lockit_sync::{SyncOutcome, SyncState};
use secrecy::ExposeSecret;

use crate::args::{SyncAction, SyncStrategy};
use crate::vault::{config_dir, vault_path};

/// Path to Google OAuth tokens.
fn token_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("gdrive-tokens.toml"))
}

/// Path to sync state.
fn sync_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("sync-state.toml"))
}

/// Path to sync key.
fn sync_key_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("sync-key.txt"))
}

/// Path to sync backend config (folder_id cache, etc).
fn sync_config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("gdrive-sync-config.json"))
}

fn load_sync_state() -> Result<Option<SyncState>> {
    let path = sync_state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let state =
        toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(state))
}

fn save_sync_state(state: &SyncState) -> Result<()> {
    let path = sync_state_path()?;
    let raw = toml::to_string(state)?;
    std::fs::write(&path, raw).with_context(|| format!("Failed to write {}", path.display()))
}

/// Load Google OAuth client config from `~/.lockit/config.toml`.
fn load_google_config() -> Result<GoogleDriveConfig> {
    let path = config_dir()?.join("config.toml");
    if !path.exists() {
        bail!(
            "No config found at {}. Create it with:\n\n\
             [google]\n\
             client_id = \"YOUR_CLIENT_ID.apps.googleusercontent.com\"\n\
             client_secret = \"GOCSPX-...\"",
            path.display()
        );
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    #[derive(serde::Deserialize)]
    struct Config {
        google: Option<GoogleDriveConfig>,
    }

    let config: Config =
        toml::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))?;
    config
        .google
        .ok_or_else(|| anyhow::anyhow!("No [google] section found in {}", path.display()))
}

/// Ensure Google tokens exist (login if needed).
/// SAFETY: Retries token refresh on transient errors before falling back to login.
fn ensure_logged_in() -> Result<GoogleTokenStore> {
    let path = token_path()?;
    if let Some(tokens) = load_tokens(&path)? {
        if token::is_token_valid(&tokens) {
            return Ok(tokens);
        }
        // Token expired, try to refresh with retry on transient errors
        let config = load_google_config()?;
        if !tokens.refresh_token.expose_secret().is_empty() {
            // Retry up to 3 times for transient network errors
            for attempt in 1..=3 {
                match token::refresh_tokens(
                    &config.client_id,
                    &config.client_secret,
                    tokens.refresh_token.expose_secret(),
                ) {
                    Ok(new_tokens) => {
                        save_tokens(&path, &new_tokens)?;
                        return Ok(new_tokens);
                    }
                    Err(e) => {
                        if attempt < 3 {
                            // Log transient error and retry
                            eprintln!("Token refresh attempt {attempt} failed: {e}. Retrying...");
                            std::thread::sleep(std::time::Duration::from_secs(2));
                        } else {
                            // All retries failed, fall back to login
                            eprintln!("Token refresh failed after {attempt} attempts: {e}. Re-authenticating...");
                        }
                    }
                }
            }
        }
    }

    // Need to login
    let config = load_google_config()?;
    let tokens = google_login(&config.client_id, &config.client_secret)?;
    save_tokens(&path, &tokens)?;
    eprintln!("\nLogged in successfully! Tokens saved.");
    Ok(tokens)
}

/// Resolve conflict strategy from CLI args.
fn resolve_strategy_from_cli(strategy: SyncStrategy) -> ResolveStrategy {
    match strategy {
        SyncStrategy::KeepLocal => ResolveStrategy::KeepLocal,
        SyncStrategy::KeepRemote => ResolveStrategy::KeepRemote,
        SyncStrategy::LastWriteWins => ResolveStrategy::LastWriteWins,
    }
}

/// Load or create vault key.
fn load_vault_key() -> Option<VaultKey> {
    let path = sync_key_path().ok()?;
    if path.exists() {
        let content = std::fs::read_to_string(&path).ok()?;
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return VaultKey::from_base64(trimmed).ok();
        }
    }
    None
}

/// Dispatch a sync subcommand.
pub fn handle_sync(action: &SyncAction) -> Result<()> {
    match action {
        SyncAction::Login => handle_login(),
        SyncAction::Logout => handle_logout(),
        SyncAction::Key => handle_key(),
        SyncAction::SetKey { key } => handle_set_key(key),
        SyncAction::Push => handle_push(),
        SyncAction::Pull => handle_pull(),
        SyncAction::Status => handle_status(),
        SyncAction::Config => handle_config(),
        SyncAction::Sync { strategy } => handle_sync_bidirectional(strategy),
        SyncAction::Poll => handle_poll(),
    }
}

/// Build the sync engine with auth check.
fn build_engine(vault_path: PathBuf) -> Result<SmartSyncEngine> {
    let tokens = ensure_logged_in()?;
    let state = load_sync_state()?;
    let sync_config = load_sync_config()?;
    let backend = SyncBackendFactory::from_token_store(tokens, sync_config)?;
    let mut engine = SmartSyncEngine::new(backend, state, vault_path);

    // Load sync key if available
    if let Some(key) = load_vault_key() {
        engine.set_vault_key(key);
    }

    Ok(engine)
}

/// Load Google Drive sync config (folder_id cache, migration flag).
fn load_sync_config() -> Result<GoogleDriveSyncConfig> {
    let path = sync_config_path()?;
    if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        if let Ok(cfg) = serde_json::from_str(&raw) {
            return Ok(cfg);
        }
    }
    let gconfig = load_google_config()?;
    Ok(GoogleDriveSyncConfig {
        client_id: gconfig.client_id,
        client_secret: gconfig.client_secret,
        folder_id: None,
        migrated_from_appdata: false,
    })
}

fn persist_state(engine: &SmartSyncEngine) -> Result<()> {
    if let Some(state) = engine.state() {
        save_sync_state(state)?;
    }
    Ok(())
}

fn outcome_message(outcome: SyncOutcome) -> String {
    match outcome {
        SyncOutcome::AlreadyUpToDate => "Already up to date.".to_string(),
        SyncOutcome::Pushed => "Pushed to Google Drive.".to_string(),
        SyncOutcome::Pulled => "Pulled from Google Drive.".to_string(),
        SyncOutcome::NeedsBaseline => {
            "Both local and remote vaults exist without a sync baseline.\n\
             Use `lk sync push --strategy keep-local` or `lk sync pull --strategy keep-remote` to choose a baseline."
                .to_string()
        }
        SyncOutcome::Error => "Sync completed with errors.".to_string(),
    }
}

fn handle_sync_bidirectional(strategy: &SyncStrategy) -> Result<()> {
    let vault_path = vault_path()?;
    let strategy = resolve_strategy_from_cli(*strategy);
    let mut engine = build_engine(vault_path.clone())?;

    let outcome = run_async(async { engine.sync_with_strategy(strategy).await })?
        .map_err(sync_error_to_anyhow)?;

    persist_state(&engine)?;
    println!("{}", outcome_message(outcome));
    Ok(())
}

fn handle_poll() -> Result<()> {
    let vault_path = vault_path()?;
    let engine = build_engine(vault_path.clone())?;

    let changed_checksum = run_async(async { engine.poll().await })?;

    match changed_checksum {
        Some(checksum) => {
            println!("{checksum}");
            Ok(())
        }
        None => {
            // SAFETY: Print message before exit so user understands the non-zero exit code.
            eprintln!("No changes detected since last sync.");
            std::process::exit(1);
        }
    }
}

fn handle_push() -> Result<()> {
    let vault_path = vault_path()?;

    if !vault_path.exists() {
        bail!("No vault found. Run 'lk init' to create one.");
    }

    let mut engine = build_engine(vault_path.clone())?;

    run_async(async { engine.push().await })?
        .map_err(sync_error_to_anyhow)?;

    persist_state(&engine)?;

    let data_len = std::fs::read(&vault_path)?.len();
    println!("Pushed vault ({} bytes) to Google Drive.", data_len);
    Ok(())
}

fn handle_pull() -> Result<()> {
    let vault_path = vault_path()?;
    let mut engine = build_engine(vault_path.clone())?;

    let backup_path = backup_existing_vault(&vault_path)?;

    let result = run_async(async { engine.pull().await })?;

    if let Err(e) = result {
        restore_backup(&vault_path, backup_path.as_ref())?;
        return Err(sync_error_to_anyhow(e));
    }

    remove_backup(backup_path.as_ref())?;

    persist_state(&engine)?;

    let data_len = std::fs::read(&vault_path)?.len();
    println!("Pulled vault ({} bytes) from Google Drive.", data_len);
    Ok(())
}

fn handle_status() -> Result<()> {
    let tokens = ensure_logged_in()?;
    let state = load_sync_state()?;
    let sync_config = load_sync_config()?;
    let backend = SyncBackendFactory::from_token_store(tokens, sync_config)?;

    let vault_path = vault_path()?;
    let local_checksum = if vault_path.exists() {
        let data = std::fs::read(&vault_path)?;
        Some(lockit_sync::sha256_hex(&data))
    } else {
        None
    };

    let meta = run_async(async {
        match backend.metadata("vault.enc").await {
            Ok(meta) => Ok(Some(meta)),
            Err(lockit_sync::error::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to fetch remote metadata: {e}")),
        }
    })??;

    println!("Backend:       Google Drive");
    println!("Authenticated: yes");

    if let Some(ref m) = meta {
        println!("Version:       {}", m.version);
        println!("Size:          {} bytes", m.size);
        println!("Last modified: {} (unix secs)", m.last_modified);
        println!("Checksum:      {}", m.checksum);
    } else {
        println!("Remote vault:  not found");
    }

    if let Some(ref s) = state {
        println!("\nLast sync state:");
        println!("  Local checksum:  {}", s.local_checksum);
        println!("  Remote checksum: {}", s.remote_checksum);
        println!("  Remote size:     {} bytes", s.remote_size);

        if let Some(local_cs) = local_checksum {
            let local_same = s.local_checksum == local_cs;
            let remote_same = meta
                .as_ref()
                .is_none_or(|m| s.remote_checksum == m.checksum);
            println!("\nCurrent status:");
            println!(
                "  Local matches last sync:  {}",
                if local_same { "yes" } else { "no" }
            );
            if let Some(_ref_m) = meta {
                println!(
                    "  Remote matches last sync: {}",
                    if remote_same {
                        "yes"
                    } else {
                        "no (remote changed)"
                    }
                );
            }
        }
    } else {
        println!("\nSync state:  never synced");
    }

    // Show sync key status
    if let Some(_key) = load_vault_key() {
        println!("\nSync Key: **** (use `lk sync key` to show)");
    } else {
        println!("\nSync Key: not configured (use `lk sync key` to generate)");
    }

    Ok(())
}

fn handle_config() -> Result<()> {
    let path = config_dir()?.join("config.toml");
    println!("Config path: {}", path.display());
    if path.exists() {
        let raw = std::fs::read_to_string(&path)?;
        println!("{raw}");
    } else {
        println!("(file does not exist — create a [google] section with client_id)");
    }
    Ok(())
}

fn handle_login() -> Result<()> {
    let config = load_google_config()?;
    let tokens = google_login(&config.client_id, &config.client_secret)?;
    save_tokens(&token_path()?, &tokens)?;
    eprintln!("\nLogged in to Google Drive successfully!");
    Ok(())
}

fn handle_logout() -> Result<()> {
    let path = token_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
        println!("Logged out. Google tokens removed.");
    } else {
        println!("No stored Google tokens found.");
    }
    Ok(())
}

fn handle_key() -> Result<()> {
    let path = sync_key_path()?;
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let key = content.trim();
        println!("Current Sync Key:");
        println!("{key}");
        println!("\nShare this key with other devices to enable sync.");
    } else {
        let key = VaultKey::generate();
        let encoded = key.to_base64();
        std::fs::write(&path, &encoded)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&path, perms)?;
        }
        println!("Generated new Sync Key:");
        println!("{encoded}");
        println!("\nSave this key! You'll need it on all devices.");
    }
    Ok(())
}

fn handle_set_key(encoded: &str) -> Result<()> {
    let key = VaultKey::from_base64(encoded)?;
    let path = sync_key_path()?;
    std::fs::write(&path, key.to_base64())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    println!("Sync Key saved successfully.");
    Ok(())
}

fn sync_error_to_anyhow(err: SyncError) -> anyhow::Error {
    match err {
        SyncError::Conflict(c) => anyhow::anyhow!(
            "{c}\nResolve it with `lk sync sync --strategy keep-local` or `lk sync sync --strategy keep-remote`."
        ),
        SyncError::Backend(msg) => anyhow::anyhow!("Backend error: {msg}"),
        SyncError::Io(msg) => anyhow::anyhow!("I/O error: {msg}"),
        SyncError::ChecksumMismatch { expected, actual } => {
            anyhow::anyhow!("Checksum mismatch: expected {expected}, got {actual}")
        }
    }
}

fn backup_existing_vault(vault_path: &std::path::Path) -> Result<Option<PathBuf>> {
    if !vault_path.exists() {
        return Ok(None);
    }

    let backup_path = vault_path.with_extension("lockit.pull_backup");
    std::fs::copy(vault_path, &backup_path)
        .with_context(|| format!("Failed to backup vault to {}", backup_path.display()))?;
    Ok(Some(backup_path))
}

fn restore_backup(vault_path: &std::path::Path, backup_path: Option<&PathBuf>) -> Result<()> {
    let Some(backup_path) = backup_path else {
        return Ok(());
    };
    if !backup_path.exists() {
        return Ok(());
    }
    std::fs::copy(backup_path, vault_path).with_context(|| {
        format!(
            "Pull failed and backup restore failed from {}",
            backup_path.display()
        )
    })?;
    remove_backup(Some(backup_path))
}

fn remove_backup(backup_path: Option<&PathBuf>) -> Result<()> {
    let Some(backup_path) = backup_path else {
        return Ok(());
    };
    if backup_path.exists() {
        std::fs::remove_file(backup_path)
            .with_context(|| format!("Failed to remove backup {}", backup_path.display()))?;
    }
    Ok(())
}

/// Run an async function synchronously using a tokio runtime.
/// SAFETY: Propagates runtime creation failure instead of panicking.
fn run_async<F, T>(f: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;
    Ok(rt.block_on(f))
}

#[cfg(test)]
mod tests {
    use lockit_sync::state::SyncState;

    #[test]
    fn sync_state_local_changed_works() {
        let s = SyncState::new("local-1".to_string(), "remote-1".to_string(), 42);
        assert!(s.local_changed("local-2"));
        assert!(!s.local_changed("local-1"));
    }

    #[test]
    fn sync_state_remote_changed_works() {
        let s = SyncState::new("local-1".to_string(), "remote-1".to_string(), 42);
        assert!(s.remote_changed("remote-2", 50));
        assert!(s.remote_changed("remote-1", 99));
        assert!(!s.remote_changed("remote-1", 42));
    }
}
