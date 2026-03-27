//! lockit CLI entry point.

mod args;
mod biometric;
mod credentials;
mod daemon;
mod shell;
mod sync;
mod vault;

use std::io::{self, BufRead, Read};

use anyhow::{Result, bail};
use args::{Args, Commands, DaemonAction, ImportFormat};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use lockit_core::{Error as CoreError, Secret, UnlockedVault};
use secrecy::{ExposeSecret, SecretString};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use zeroize::Zeroizing;

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(&args.log_level)?;

    match args.command {
        Commands::Init => handle_init(),
        Commands::Add {
            profile,
            key,
            value,
            from_env,
            from_dotenv,
            force,
            expires,
        } => handle_add(
            &profile,
            key.as_deref(),
            value.as_deref(),
            from_env,
            from_dotenv.as_deref(),
            force,
            expires.as_deref(),
        ),
        Commands::Get {
            profile,
            key,
            json,
            export,
        } => handle_get(&profile, key.as_deref(), json, export),
        Commands::List => handle_list(),
        Commands::Delete { profile, key } => handle_delete(&profile, key.as_deref()),
        Commands::Run {
            profile,
            prefix,
            no_inherit,
            cmd,
        } => handle_run(&profile, prefix.as_deref(), no_inherit, &cmd),
        Commands::Recover => handle_recover(),
        Commands::Daemon { action } => match action {
            DaemonAction::Start => daemon::start_daemon(),
            DaemonAction::Stop => daemon::stop_daemon(),
            DaemonAction::Status => daemon::status_daemon(),
            DaemonAction::Run => daemon::run_daemon_foreground(),
        },
        Commands::Sync { action } => sync::handle_sync(&action),
        Commands::Export { profile, json } => handle_export(profile.as_deref(), json),
        Commands::Import {
            profile,
            file,
            force,
            format,
        } => handle_import(&profile, &file, force, format),
        Commands::Unlock {
            stdin,
            biometric,
            save_biometric,
            clear_biometric,
        } => handle_unlock(stdin, biometric, save_biometric, clear_biometric),
        Commands::Expire { warn_days } => handle_expire(warn_days),
        Commands::Vault => handle_vault_info(),
        Commands::GenerateCompletion { shell } => {
            let mut cmd = Args::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut io::stdout());
            Ok(())
        }
    }
}

// ─── Tracing ─────────────────────────────────────────────────────────────────

fn init_tracing(level: &str) -> Result<()> {
    let level = level.parse::<Level>()?;
    FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();
    Ok(())
}

// ─── Password prompts ────────────────────────────────────────────────────────

fn prompt_password(prompt: &str) -> Result<SecretString> {
    rpassword::prompt_password(prompt)
        .map(SecretString::new)
        .map_err(Into::into)
}

fn prompt_new_password() -> Result<SecretString> {
    let password = prompt_password("Enter new vault password: ")?;
    let confirm = prompt_password("Confirm password: ")?;
    if password.expose_secret() != confirm.expose_secret() {
        bail!("Passwords do not match");
    }
    Ok(password)
}

fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 12 {
        bail!("Password too short — minimum 12 characters required.");
    }
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_symbol = password.chars().any(|c| !c.is_alphanumeric());
    let class_count = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|&&x| x)
        .count();
    if class_count < 2 {
        bail!(
            "Password too weak — use at least 2 of: lowercase, uppercase, digits, symbols.\n\
             A 12-character password from a single character class has far less entropy than \
             Argon2id's memory-hard parameters are designed to protect."
        );
    }
    Ok(())
}

// ─── Vault helpers ───────────────────────────────────────────────────────────

/// Map lockit-core errors to user-friendly anyhow errors.
fn map_core_error(e: CoreError) -> anyhow::Error {
    match e {
        CoreError::IncorrectPassword => anyhow::anyhow!("Incorrect password"),
        _ => anyhow::anyhow!("Failed to unlock vault: {}", e),
    }
}

fn open_vault() -> Result<UnlockedVault> {
    if !vault::vault_exists()? {
        bail!("No vault found. Run 'lk init' to create one.");
    }
    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;
    let password = prompt_password("Vault password: ")?;

    UnlockedVault::open(&vault_path, password.expose_secret(), &device_key).map_err(map_core_error)
}

/// Save vault and write the plaintext credentials file for tool compatibility.
fn save_and_sync(vault: &UnlockedVault) -> Result<()> {
    vault.save()?;
    credentials::write_credentials(vault)
}

// ─── Profile validation ──────────────────────────────────────────────────────

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Profile name cannot be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "Invalid profile name '{}'. Only [a-z0-9_-] characters are allowed.",
            name
        );
    }
    Ok(())
}

// ─── Command handlers ────────────────────────────────────────────────────────

fn handle_init() -> Result<()> {
    if vault::vault_exists()? {
        bail!("Vault already exists. Delete ~/.lockit to reinitialize.");
    }

    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;
    let password = prompt_new_password()?;
    validate_password_strength(password.expose_secret())?;

    let (mut vault, mnemonic) =
        UnlockedVault::init_with_recovery(password.expose_secret(), &device_key)?;

    // Display mnemonic with formatting
    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║          RECOVERY PHRASE — SAVE THIS NOW         ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("  These 24 words can restore your vault if you forget");
    println!("  your password. Write them down and store them safely.");
    println!("  They will NOT be shown again.");
    println!();

    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    for (i, chunk) in words.chunks(6).enumerate() {
        let line: Vec<String> = chunk
            .iter()
            .enumerate()
            .map(|(j, w)| format!("{:2}. {:<12}", i * 6 + j + 1, w))
            .collect();
        println!("  {}", line.join("  "));
    }
    println!();

    print!("Type 'yes' to confirm you have saved the recovery phrase: ");
    use std::io::Write;
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("yes") {
        bail!("Initialization aborted. Run 'lk init' again to create your vault.");
    }

    vault.save_to(&vault_path)?;
    println!();
    println!("Vault initialized: {}", vault_path.display());
    Ok(())
}

fn handle_add(
    profile: &str,
    key: Option<&str>,
    value: Option<&str>,
    from_env: bool,
    from_dotenv: Option<&str>,
    force: bool,
    expires: Option<&str>,
) -> Result<()> {
    validate_profile_name(profile)?;

    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;

    // Open or create vault
    let mut vault = if vault::vault_exists()? {
        let password = prompt_password("Vault password: ")?;
        UnlockedVault::open(&vault_path, password.expose_secret(), &device_key)
            .map_err(map_core_error)?
    } else {
        let password = prompt_new_password()?;
        validate_password_strength(password.expose_secret())?;
        let (mut v, mnemonic) =
            UnlockedVault::init_with_recovery(password.expose_secret(), &device_key)?;
        v.save_to(&vault_path)?;
        eprintln!();
        eprintln!("╔══════════════════════════════════════════════════╗");
        eprintln!("║          RECOVERY PHRASE — SAVE THIS NOW         ║");
        eprintln!("╚══════════════════════════════════════════════════╝");
        eprintln!();
        eprintln!("  {}", mnemonic);
        eprintln!();
        eprintln!("  Write it down and store it safely.");
        eprintln!("  It will NOT be shown again.");
        eprintln!();
        v
    };

    // --from-env: import all environment variables
    if from_env {
        let mut count = 0usize;
        for (k, v) in std::env::vars() {
            vault.set(profile, &k.to_lowercase(), &Secret::new(v.into_bytes()))?;
            count += 1;
        }
        check_vault_size(vault.credential_count(), force)?;
        save_and_sync(&vault)?;
        println!(
            "Imported {} environment variables into profile '{}'.",
            count, profile
        );
        return Ok(());
    }

    // --from-dotenv <file>
    if let Some(dotenv_path) = from_dotenv {
        let entries = credentials::parse_dotenv(dotenv_path)?;
        let count = entries.len();
        for (k, v) in entries {
            vault.set(profile, &k, &Secret::new(v.into_bytes()))?;
        }
        check_vault_size(vault.credential_count(), force)?;
        save_and_sync(&vault)?;
        println!(
            "Imported {} keys from '{}' into profile '{}'.",
            count, dotenv_path, profile
        );
        return Ok(());
    }

    // Normal --key / --value mode
    let key = key.ok_or_else(|| anyhow::anyhow!("--key is required"))?;

    let secret_value = match value {
        Some(v) => v.to_string(),
        None => {
            let v = prompt_password(&format!("Value for '{}': ", key))?;
            v.expose_secret().clone()
        }
    };

    // Overwrite confirmation
    if vault.contains(profile, key)? {
        eprint!(
            "Key '{}' already exists in profile '{}'. Overwrite? [y/N] ",
            key, profile
        );
        use std::io::Write;
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let expires_ts = expires.map(parse_expires_date).transpose()?;
    vault.set_with_expiry(
        profile,
        key,
        &Secret::new(secret_value.into_bytes()),
        expires_ts,
    )?;
    check_vault_size(vault.credential_count(), force)?;
    save_and_sync(&vault)?;

    if let Some(ts) = expires_ts {
        println!(
            "Added '{}' to profile '{}' (expires {}).",
            key,
            profile,
            format_unix_ts(ts)
        );
    } else {
        println!("Added '{}' to profile '{}'.", key, profile);
    }
    Ok(())
}

fn handle_get(profile: &str, key: Option<&str>, json: bool, export: bool) -> Result<()> {
    let vault = open_vault()?;

    match key {
        Some(k) => match vault.get(profile, k)? {
            Some(value) => {
                let v = String::from_utf8_lossy(value.expose_secret());
                if export {
                    println!("export {}={}", k.to_uppercase(), shell::shell_quote(&v));
                } else if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "profile": profile,
                            "key": k,
                            "value": v.as_ref()
                        })
                    );
                } else {
                    println!("{}", v);
                }
            }
            None => bail!("Key '{}' not found in profile '{}'.", k, profile),
        },
        None => {
            let mut keys = vault.keys(profile)?;
            keys.sort();

            if json {
                let mut map = serde_json::Map::new();
                for k in &keys {
                    if let Some(v) = vault.get(profile, k)? {
                        map.insert(
                            k.clone(),
                            serde_json::Value::String(
                                String::from_utf8_lossy(v.expose_secret()).to_string(),
                            ),
                        );
                    }
                }
                println!("{}", serde_json::Value::Object(map));
            } else if export {
                for k in &keys {
                    if let Some(v) = vault.get(profile, k)? {
                        let s = String::from_utf8_lossy(v.expose_secret());
                        println!("export {}={}", k.to_uppercase(), shell::shell_quote(&s));
                    }
                }
            } else if keys.is_empty() {
                println!("Profile '{}' is empty or does not exist.", profile);
            } else {
                for k in &keys {
                    println!("{}", k);
                }
            }
        }
    }

    Ok(())
}

fn handle_list() -> Result<()> {
    let vault = open_vault()?;
    let mut profiles = vault.profiles()?;

    if profiles.is_empty() {
        println!("No profiles found.");
    } else {
        profiles.sort();
        println!("Profiles ({}):", profiles.len());
        for profile in &profiles {
            let key_count = vault.keys(profile)?.len();
            println!(
                "  {}  ({} key{})",
                profile,
                key_count,
                if key_count == 1 { "" } else { "s" }
            );
        }
    }

    Ok(())
}

fn handle_delete(profile: &str, key: Option<&str>) -> Result<()> {
    let mut vault = open_vault()?;

    match key {
        Some(k) => {
            if vault.delete(profile, k)? {
                save_and_sync(&vault)?;
                println!("Deleted key '{}' from profile '{}'.", k, profile);
            } else {
                bail!("Key '{}' not found in profile '{}'.", k, profile);
            }
        }
        None => {
            // Delete entire profile — requires explicit confirmation.
            let keys = vault.keys(profile)?;
            if keys.is_empty() {
                bail!("Profile '{}' not found.", profile);
            }
            eprint!(
                "This will permanently delete profile '{}' ({} key{}).\nType the profile name to confirm: ",
                profile,
                keys.len(),
                if keys.len() == 1 { "" } else { "s" }
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim() != profile {
                bail!("Aborted — profile name did not match.");
            }
            for k in &keys {
                vault.delete(profile, k)?;
            }
            save_and_sync(&vault)?;
            println!("Deleted profile '{}'.", profile);
        }
    }

    Ok(())
}

fn handle_run(
    profiles: &[String],
    prefix: Option<&str>,
    no_inherit: bool,
    cmd: &[String],
) -> Result<()> {
    if cmd.is_empty() {
        bail!("No command specified after '--'.");
    }

    // If no --profile flags were provided, fall back to .lockitrc discovery.
    let resolved_profiles: Vec<String> = if profiles.is_empty() {
        match find_lockitrc_profile()? {
            Some(p) => {
                eprintln!("lockit: using profile '{}' from .lockitrc", p);
                vec![p]
            }
            None => bail!(
                "No --profile specified and no .lockitrc found in the current directory tree.\n\
                 Use: lk run --profile <name> -- <command>"
            ),
        }
    } else {
        profiles.to_vec()
    };

    let vault = open_vault()?;

    // Collect env vars from all specified profiles
    let mut env_vars: Vec<(String, String)> = Vec::new();
    for profile in &resolved_profiles {
        let mut keys = vault.keys(profile)?;
        keys.sort();
        for key in keys {
            if let Some(value) = vault.get(profile, &key)? {
                let env_key = match prefix {
                    Some(p) => format!("{}_{}", p.to_uppercase(), key.to_uppercase()),
                    None => key.to_uppercase(),
                };
                let env_val = String::from_utf8_lossy(value.expose_secret()).to_string();
                env_vars.push((env_key, env_val));
            }
        }
    }

    // Build and run the child process
    let mut command = std::process::Command::new(&cmd[0]);
    command.args(&cmd[1..]);

    if no_inherit {
        command.env_clear();
    }
    for (k, v) in &env_vars {
        command.env(k, v);
    }

    let status = command
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run '{}': {}", cmd[0], e))?;

    // Passthrough exit code.
    // TODO #87: On Unix, status.code() returns None when the child is killed by a signal;
    // unwrap_or(1) loses that information. Fix: use ExitStatusExt::signal() to return
    // 128+signal instead, and avoid std::process::exit() to allow Drop to run cleanly.
    std::process::exit(status.code().unwrap_or(1));
}

fn handle_recover() -> Result<()> {
    if !vault::vault_exists()? {
        bail!("No vault found. Run 'lk init' first.");
    }

    println!("Enter your 24-word recovery phrase (space-separated):");
    print!("> ");
    use std::io::Write;
    io::stdout().flush()?;

    let mut mnemonic = String::new();
    io::stdin().lock().read_line(&mut mnemonic)?;

    let new_password = prompt_new_password()?;
    validate_password_strength(new_password.expose_secret())?;

    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;

    UnlockedVault::recover_with_mnemonic(
        &vault_path,
        mnemonic.trim(),
        new_password.expose_secret(),
        &device_key,
    )
    .map_err(|e| anyhow::anyhow!("Recovery failed: {}", e))?;

    println!("Vault password updated successfully.");
    Ok(())
}

fn handle_export(profile: Option<&str>, json: bool) -> Result<()> {
    let vault = open_vault()?;

    let profiles = match profile {
        Some(p) => vec![p.to_string()],
        None => {
            let mut ps = vault.profiles()?;
            ps.sort();
            ps
        }
    };

    if json {
        let mut root = serde_json::Map::new();
        for p in &profiles {
            let mut map = serde_json::Map::new();
            let mut keys = vault.keys(p)?;
            keys.sort();
            for k in &keys {
                if let Some(v) = vault.get(p, k)? {
                    map.insert(
                        k.clone(),
                        serde_json::Value::String(
                            String::from_utf8_lossy(v.expose_secret()).to_string(),
                        ),
                    );
                }
            }
            root.insert(p.clone(), serde_json::Value::Object(map));
        }
        println!("{}", serde_json::Value::Object(root));
    } else {
        for p in &profiles {
            let mut keys = vault.keys(p)?;
            keys.sort();
            if !keys.is_empty() {
                println!("# profile: {}", p);
                for k in &keys {
                    if let Some(v) = vault.get(p, k)? {
                        let val = String::from_utf8_lossy(v.expose_secret());
                        println!("{}={}", k.to_uppercase(), shell::shell_quote(&val));
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

fn handle_import(profile: &str, file: &str, force: bool, format: ImportFormat) -> Result<()> {
    validate_profile_name(profile)?;

    let raw = read_input(file)?;

    // Each variant returns a list of (profile, key, value) triples.
    // For single-profile formats the profile from the CLI arg is used.
    let triples: Vec<(String, String, Secret<String>)> = match format {
        ImportFormat::Dotenv => credentials::parse_dotenv_str(&raw)?
            .into_iter()
            .map(|(k, v)| (profile.to_string(), k, Secret::new(v)))
            .collect(),
        ImportFormat::Json => parse_import_json(&raw, profile)?,
        ImportFormat::Csv => parse_import_csv(&raw, profile)?,
        ImportFormat::OnePasswordCsv => parse_import_1password_csv(&raw, profile)?,
    };

    if triples.is_empty() {
        bail!("No entries found in input.");
    }

    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;

    let mut v = if vault::vault_exists()? {
        let password = prompt_password("Vault password: ")?;
        UnlockedVault::open(&vault_path, password.expose_secret(), &device_key)
            .map_err(map_core_error)?
    } else {
        bail!("No vault found. Run 'lk init' to create one.");
    };

    let count = triples.len();
    for (p, k, val) in triples {
        v.set(
            &p,
            &k,
            &Secret::new(val.expose_secret().as_bytes().to_vec()),
        )?;
    }
    check_vault_size(v.credential_count(), force)?;
    save_and_sync(&v)?;

    println!("Imported {} key(s).", count);
    Ok(())
}

/// Read a file path or stdin ("-") into a `Zeroizing<String>` so the
/// raw import data (potentially containing credential values) is wiped
/// from memory when it goes out of scope.
fn read_input(path: &str) -> Result<Zeroizing<String>> {
    if path == "-" {
        let mut s = String::new();
        io::stdin().lock().read_to_string(&mut s)?;
        Ok(Zeroizing::new(s))
    } else {
        Ok(Zeroizing::new(std::fs::read_to_string(path)?))
    }
}

/// Parse JSON: either flat `{"KEY":"val"}` or multi-profile `{"profile":{"KEY":"val"}}`.
///
/// For the multi-profile form all profiles in the JSON are imported and the
/// CLI `profile` argument is ignored (a notice is printed).
fn parse_import_json(src: &str, profile: &str) -> Result<Vec<(String, String, Secret<String>)>> {
    let val: serde_json::Value =
        serde_json::from_str(src).map_err(|e| anyhow::anyhow!("Invalid JSON: {}", e))?;
    let obj = val
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("JSON root must be an object"))?;

    // Detect multi-profile: all values are objects.
    let multi = obj.values().all(|v| v.is_object());

    if multi {
        eprintln!(
            "note: JSON contains multiple profiles — importing all (profile argument '{}' ignored).",
            profile
        );
        let mut out = Vec::new();
        for (p, keys_val) in obj {
            let keys = keys_val
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Expected object for profile '{}'", p))?;
            for (k, v) in keys {
                let s = v
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Value for {}/{} must be a string", p, k))?;
                out.push((p.clone(), k.clone(), Secret::new(s.to_string())));
            }
        }
        Ok(out)
    } else {
        // Flat {"KEY":"val"} → single profile
        let mut out = Vec::new();
        for (k, v) in obj {
            let s = v
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Value for key '{}' must be a string", k))?;
            out.push((profile.to_string(), k.clone(), Secret::new(s.to_string())));
        }
        Ok(out)
    }
}

/// Parse a two-column `key,value` CSV using the `csv` crate (RFC-4180 compliant,
/// handles quoted fields with embedded commas and newlines).
fn parse_import_csv(src: &str, profile: &str) -> Result<Vec<(String, String, Secret<String>)>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .comment(Some(b'#'))
        .flexible(true)
        .from_reader(src.as_bytes());
    let mut out = Vec::new();
    let mut first = true;
    for result in rdr.records() {
        let record = result.map_err(|e| anyhow::anyhow!("CSV parse error: {}", e))?;
        let k = record.get(0).unwrap_or("").trim().to_string();
        let v = record.get(1).unwrap_or("").trim().to_string();
        // Skip header row (key,value or similar)
        if first && k.eq_ignore_ascii_case("key") {
            first = false;
            continue;
        }
        first = false;
        if k.is_empty() {
            continue;
        }
        out.push((profile.to_string(), k, Secret::new(v)));
    }
    Ok(out)
}

/// Parse a 1Password item-export CSV (RFC-4180 via `csv` crate).
///
/// Expected columns (case-insensitive): Title, Username, Password, URL, Notes.
/// Each row produces up to four keys under `{profile}/{slug(title)}`:
/// `username`, `password`, `url`, `notes` (empty values are skipped).
fn parse_import_1password_csv(
    src: &str,
    profile: &str,
) -> Result<Vec<(String, String, Secret<String>)>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(src.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| anyhow::anyhow!("Failed to read 1Password CSV headers: {}", e))?
        .clone();

    let col_idx = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim().trim_matches('"').eq_ignore_ascii_case(name))
    };
    let title_i = col_idx("title").ok_or_else(|| anyhow::anyhow!("Missing 'Title' column"))?;
    let user_i = col_idx("username");
    let pass_i = col_idx("password");
    let url_i = col_idx("url");
    let notes_i = col_idx("notes");

    let mut out = Vec::new();
    for (i, result) in rdr.records().enumerate() {
        let record = result
            .map_err(|e| anyhow::anyhow!("1Password CSV parse error at row {}: {}", i + 2, e))?;
        let title = record.get(title_i).unwrap_or("").trim().to_string();
        if title.is_empty() {
            continue;
        }
        let slug: String = title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string();
        let sub_profile = format!("{}/{}", profile, slug);

        let mut added = 0usize;
        let mut add = |key: &str, idx: Option<usize>| {
            let val = record.get(idx?).unwrap_or("").trim().to_string();
            if !val.is_empty() {
                out.push((sub_profile.clone(), key.to_string(), Secret::new(val)));
                added += 1;
            }
            Some(())
        };
        add("username", user_i);
        add("password", pass_i);
        add("url", url_i);
        add("notes", notes_i);

        if added == 0 {
            eprintln!("warning: row {}: no usable fields for '{}'", i + 2, title);
        }
    }
    Ok(out)
}

// ─── Unlock / biometric ───────────────────────────────────────────────────────

/// Implements `lk unlock [--stdin] [--biometric] [--save-biometric] [--clear-biometric]`.
///
/// # Modes
///
/// | Flags              | Behaviour                                              |
/// |--------------------|--------------------------------------------------------|
/// | *(none)*           | Interactive password prompt                            |
/// | `--stdin`          | Read password from stdin (one line, CI-friendly)       |
/// | `--biometric`      | Retrieve password from Keychain via Touch ID (macOS)   |
/// | `--save-biometric` | Prompt, validate, then save password to Keychain       |
/// | `--clear-biometric`| Remove the saved Keychain item                         |
///
/// After the vault password is validated, the command also sends an
/// `UnlockVault` / `UnlockWithBiometric` IPC request to the daemon if one is
/// running, so the daemon session stays active for subsequent commands.
fn handle_unlock(
    from_stdin: bool,
    biometric: bool,
    save_biometric: bool,
    clear_biometric: bool,
) -> Result<()> {
    // ── --clear-biometric: just remove the Keychain item ─────────────────────
    if clear_biometric {
        biometric::delete_saved_password()?;
        println!("Biometric credential removed from Keychain.");
        return Ok(());
    }

    let device_key = vault::load_or_create_device_key()?;
    let vault_path = vault::vault_path()?;
    if !vault_path.exists() {
        bail!("No vault found. Run 'lk init' to create one.");
    }

    // ── --biometric: retrieve password from Keychain via Touch ID ────────────
    if biometric {
        let (password, used_biometric) = match biometric::load_password() {
            Ok(pwd) => (pwd, true),
            Err(biometric_err) => {
                // Biometric failed or not configured — offer password fallback.
                eprintln!("Biometric unlock: {}", biometric_err);
                eprintln!("Falling back to master password...");
                (prompt_password("Vault password: ")?, false)
            }
        };

        UnlockedVault::open(&vault_path, password.expose_secret(), &device_key)
            .map_err(map_core_error)?;

        if used_biometric {
            send_unlock_to_daemon(password.expose_secret(), &device_key, Some("touchid"));
            println!("Vault unlocked via Touch ID.");
        } else {
            send_unlock_to_daemon(password.expose_secret(), &device_key, None);
            println!("Vault unlocked via password (biometric fallback).");
        }
        return Ok(());
    }

    // ── Obtain password (stdin or interactive prompt) ─────────────────────────
    let password = if from_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        SecretString::new(line.trim_end_matches(['\n', '\r']).to_string())
    } else {
        prompt_password("Vault password: ")?
    };

    // Validate by opening the vault.
    UnlockedVault::open(&vault_path, password.expose_secret(), &device_key)
        .map_err(map_core_error)?;

    // ── --save-biometric: persist password in Keychain after validation ───────
    if save_biometric {
        biometric::save_password(password.expose_secret())?;
        println!(
            "Vault password saved to Keychain with biometric protection.\n\
             Use `lk unlock --biometric` for future Touch ID / Face ID unlocks."
        );
        return Ok(());
    }

    // ── Normal unlock: also forward to daemon if running ─────────────────────
    send_unlock_to_daemon(password.expose_secret(), &device_key, None);
    println!("Vault password verified.");
    Ok(())
}

/// Forward an unlock event to the daemon (best-effort; silently no-ops if the
/// daemon is not running).
///
/// Pass `biometric_source = Some("touchid")` for biometric unlocks so the
/// daemon can record an audit log entry; use `None` for password unlocks.
fn send_unlock_to_daemon(password: &str, device_key: &[u8; 32], biometric_source: Option<&str>) {
    use lockit_ipc::{IpcClient, Request};
    let Ok(path) = lockit_ipc::socket_path() else {
        return;
    };
    if !path.exists() {
        return;
    }
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    let request = match biometric_source {
        Some(source) => Request::UnlockWithBiometric {
            password: lockit_ipc::Password::new(password),
            device_key: device_key.to_vec(),
            biometric_source: source.to_string(),
        },
        None => Request::UnlockVault {
            password: lockit_ipc::Password::new(password),
            device_key: device_key.to_vec(),
        },
    };
    let result = rt.block_on(async {
        let client = IpcClient::new(path);
        client.send_request(&request).await
    });
    match result {
        Ok(_) => {
            if biometric_source.is_some() {
                println!("Daemon session unlocked via biometric.");
            } else {
                println!("Daemon session unlocked.");
            }
        }
        Err(_) => println!("(Daemon not running — start with `lk daemon start`.)"),
    }
}

// ─── Vault size limits (#88) ─────────────────────────────────────────────────

/// Enforce tiered credential count limits.
///
/// | Count       | Action                              |
/// |-------------|-------------------------------------|
/// | < 500       | silent                              |
/// | 500–999     | hint on stderr                      |
/// | 1 000–1 999 | warning on stderr                   |
/// | 2 000–4 999 | strong warning on stderr            |
/// | 5 000–9 999 | error unless `--force`              |
/// | 10 000+     | hard error (no bypass)              |
fn check_vault_size(count: usize, force: bool) -> Result<()> {
    match count {
        0..=499 => {}
        500..=999 => eprintln!(
            "💡 Vault has {} credentials. Consider splitting into multiple profiles.",
            count
        ),
        1000..=1999 => eprintln!(
            "⚠️  Vault has {} credentials — saves may slow down. Consider cleaning up.",
            count
        ),
        2000..=4999 => eprintln!(
            "🔴 Vault has {} credentials — please clean up or split profiles soon.",
            count
        ),
        5000..=9999 => {
            if force {
                eprintln!(
                    "🔴 Vault has {} credentials (--force used to bypass soft limit).",
                    count
                );
            } else {
                bail!(
                    "Vault has {} credentials — over the soft limit of 5 000.\n\
                     Clean up old entries or use `--force` to save anyway.",
                    count
                );
            }
        }
        _ => bail!(
            "Vault has {} credentials — over the hard limit of 10 000. \
             Delete entries before adding more.",
            count
        ),
    }
    Ok(())
}

// ─── Credential expiry (#68) ──────────────────────────────────────────────────

/// Parse an ISO-8601 date/datetime string into a Unix timestamp.
///
/// Accepted formats:
/// - `YYYY-MM-DD` (interpreted as midnight UTC)
/// - `YYYY-MM-DDTHH:MM:SSZ` (UTC)
///
/// Year must be in the range 1970–9999.
fn parse_expires_date(s: &str) -> Result<u64> {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

    let dt: DateTime<Utc> = if s.len() == 10 {
        // YYYY-MM-DD — parse as midnight UTC
        let nd = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
            anyhow::anyhow!(
                "Invalid date '{}'. Use YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ",
                s
            )
        })?;
        Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0).unwrap())
    } else {
        // YYYY-MM-DDTHH:MM:SSZ
        let ndt = NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S")
            .map_err(|_| {
                anyhow::anyhow!(
                    "Invalid date '{}'. Use YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ",
                    s
                )
            })?;
        Utc.from_utc_datetime(&ndt)
    };

    let year = dt.format("%Y").to_string().parse::<i32>().unwrap_or(0);
    if !(1970..=9999).contains(&year) {
        bail!("Year must be in the range 1970–9999 (got {})", year);
    }
    let ts = dt.timestamp();
    if ts < 0 {
        bail!("Expiry date must be after 1970-01-01");
    }
    Ok(ts as u64)
}

/// Format a Unix timestamp as a human-readable UTC date string.
fn format_unix_ts(ts: u64) -> String {
    use chrono::{TimeZone, Utc};
    match Utc.timestamp_opt(ts as i64, 0).single() {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        None => format!("<ts={}>", ts),
    }
}

/// `lk expire [--warn-days N]` — list expired and soon-to-expire credentials.
fn handle_expire(warn_days: u64) -> Result<()> {
    let vault = open_vault()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let expired = vault.expired_credentials(now);
    let expiring = vault.expiring_soon(now, warn_days * 86400);

    if expired.is_empty() && expiring.is_empty() {
        println!("No expired or soon-to-expire credentials.");
        return Ok(());
    }

    if !expired.is_empty() {
        println!("EXPIRED ({}):", expired.len());
        for (profile, key, ts) in &expired {
            println!("  {}/{:20}  expired {}", profile, key, format_unix_ts(*ts));
        }
    }

    if !expiring.is_empty() {
        println!(
            "\nExpiring within {} day(s) ({}):",
            warn_days,
            expiring.len()
        );
        for (profile, key, ts) in &expiring {
            println!("  {}/{:20}  expires {}", profile, key, format_unix_ts(*ts));
        }
    }

    if !expired.is_empty() {
        bail!(
            "{} credential(s) are expired — update them with `lk add`",
            expired.len()
        );
    }
    Ok(())
}

// ─── .lockitrc profile discovery (#70) ──────────────────────────────────────

/// Walk from the current directory up to the filesystem root looking for a
/// `.lockitrc` file.  Returns the profile name on the first `profile = <name>`
/// line found, or `None` if no file is found.
///
/// `.lockitrc` format (either form accepted):
/// ```text
/// profile = myapp
/// profile=myapp
/// ```
fn find_lockitrc_profile() -> Result<Option<String>> {
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".lockitrc");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate)?;
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }
                let stripped = line
                    .strip_prefix("profile")
                    .map(|s| s.trim())
                    .and_then(|s| s.strip_prefix('='))
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string());
                if let Some(profile) = stripped
                    && !profile.is_empty()
                {
                    return Ok(Some(profile));
                }
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }
    Ok(None)
}

// ─── Vault info (#73) ────────────────────────────────────────────────────────

/// `lk vault` — display vault file metadata without unlocking.
fn handle_vault_info() -> Result<()> {
    let path = vault::vault_path()?;
    if !path.exists() {
        bail!(
            "No vault found at {}. Run 'lk init' to create one.",
            path.display()
        );
    }

    let meta = std::fs::metadata(&path)?;
    let size = meta.len();

    // Read just enough to decode the outer VaultFileData envelope (magic + version).
    let data = std::fs::read(&path)?;
    // VaultFileData is msgpack; try to decode the first two named fields.
    let version_info: Option<(String, u16)> = (|| -> Option<(String, u16)> {
        #[derive(serde::Deserialize)]
        struct Peek {
            #[serde(default)]
            version: u16,
        }
        let peek: Peek = rmp_serde::from_slice(&data).ok()?;
        Some(("lockit".into(), peek.version))
    })();

    println!("Vault path:    {}", path.display());
    println!("File size:     {} bytes", size);
    match version_info {
        Some((_, v)) => println!("Format version: v{}", v),
        None => println!("Format version: (unreadable)"),
    }
    println!();
    println!("To check for expired credentials: lk expire");
    println!("To export all credentials:        lk export > backup.env");
    Ok(())
}

#[cfg(test)]
mod expiry_tests {
    use super::*;

    #[test]
    fn test_parse_date_only() {
        let ts = parse_expires_date("2027-01-01").unwrap();
        assert_eq!(format_unix_ts(ts), "2027-01-01T00:00:00Z");
    }

    #[test]
    fn test_parse_datetime() {
        let ts = parse_expires_date("2027-06-15T12:30:00Z").unwrap();
        assert_eq!(format_unix_ts(ts), "2027-06-15T12:30:00Z");
    }

    #[test]
    fn test_invalid_date_rejects_feb_30() {
        assert!(parse_expires_date("2024-02-30").is_err());
    }

    #[test]
    fn test_invalid_month_13() {
        assert!(parse_expires_date("2024-13-01").is_err());
    }

    #[test]
    fn test_year_before_epoch_rejected() {
        assert!(parse_expires_date("1969-12-31").is_err());
    }

    #[test]
    fn test_year_too_large_rejected() {
        assert!(parse_expires_date("10000-01-01").is_err());
    }

    #[test]
    fn test_garbage_rejected() {
        assert!(parse_expires_date("not-a-date").is_err());
        assert!(parse_expires_date("2024/01/01").is_err());
    }

    #[test]
    fn test_format_roundtrip() {
        let ts = parse_expires_date("2030-12-31").unwrap();
        assert_eq!(format_unix_ts(ts), "2030-12-31T00:00:00Z");
    }
}
