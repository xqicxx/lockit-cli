//! INI-format credentials file (~/.lockit/credentials).
//!
//! This is a **plaintext** file for third-party tool compatibility (AWS CLI style).
//! The vault file is the encrypted store; this file is a convenience export.
//! File permissions are always set to 0600.

use std::io::Write;

use anyhow::{Result, bail};
use lockit_core::UnlockedVault;
use secrecy::ExposeSecret;

use crate::vault::config_dir;

/// Write all vault credentials to `~/.lockit/credentials` in INI format.
/// Called automatically after every `lk add` / `lk delete`.
pub fn write_credentials(vault: &UnlockedVault) -> Result<()> {
    let path = config_dir()?.join("credentials");

    let mut profiles = vault.profiles()?;
    profiles.sort();

    let mut content = String::new();
    content.push_str("# lockit credentials — auto-generated, do not edit manually\n");
    content.push_str("# Format: AWS CLI compatible INI  |  permissions: 0600\n\n");

    for profile in &profiles {
        let mut keys = vault.keys(profile)?;
        keys.sort();
        if keys.is_empty() {
            continue;
        }
        content.push_str(&format!("[{}]\n", profile));
        for key in &keys {
            if let Some(value) = vault.get(profile, key)? {
                let val = String::from_utf8_lossy(value.expose_secret());
                content.push_str(&format!("{} = {}\n", key, val));
            }
        }
        content.push('\n');
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, content.as_bytes())?;
    }

    Ok(())
}

/// Parse dotenv content (`&str`) into `(key, value)` pairs.
///
/// Supported syntax:
/// - Comments: lines starting with `#`
/// - Empty lines: skipped
/// - `KEY=value` — bare value
/// - `KEY="value"` — double-quoted (quotes stripped)
/// - `KEY='value'` — single-quoted (quotes stripped)
/// - `export KEY=value` — optional `export` prefix
pub fn parse_dotenv_str(content: &str) -> Result<Vec<(String, String)>> {
    let mut entries = Vec::new();

    for (line_num, raw) in content.lines().enumerate() {
        let line = raw.trim();

        // Skip blank lines and comments.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip optional `export` prefix.
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();

        let eq_pos = line.find('=').ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid .env syntax at line {}: '{}' (expected KEY=value)",
                line_num + 1,
                raw
            )
        })?;

        let key = line[..eq_pos].trim().to_string();
        if key.is_empty() {
            bail!("Empty key at line {}", line_num + 1);
        }

        let raw_val = line[eq_pos + 1..].trim();
        let value = strip_quotes(raw_val);

        entries.push((key, value));
    }

    Ok(entries)
}

/// Parse a .env file into `(key, value)` pairs.
pub fn parse_dotenv(path: &str) -> Result<Vec<(String, String)>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read '{}': {}", path, e))?;
    parse_dotenv_str(&content)
}

/// Strip single or double quotes from a dotenv value (e.g. `"foo"` → `foo`).
fn strip_quotes(s: &str) -> String {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
