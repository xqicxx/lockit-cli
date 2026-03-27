//! Vault path and device key management.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use lockit_core::generate_device_key;

/// Get the lockit config directory (~/.lockit).
pub fn config_dir() -> Result<PathBuf> {
    let home = home::home_dir().context("Could not determine home directory")?;
    let dir = home.join(".lockit");

    // Create directory if it doesn't exist
    if !dir.exists() {
        fs::create_dir_all(&dir).context("Failed to create config directory")?;
    }

    Ok(dir)
}

/// Get the vault file path.
pub fn vault_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("vault.lockit"))
}

/// Get the device key file path.
pub fn device_key_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("device.key"))
}

/// Load or generate device key.
pub fn load_or_create_device_key() -> Result<[u8; 32]> {
    let path = device_key_path()?;

    if path.exists() {
        // Load existing key
        let data = fs::read(&path).context("Failed to read device key")?;
        let actual = data.len();
        let key: [u8; 32] = data.try_into().map_err(|_| {
            anyhow::anyhow!(
                "Invalid device key file at {}: expected 32 bytes, found {} byte{}.\n\
                 The file may be truncated or corrupted. If you have a backup, restore it.\n\
                 Otherwise run `lk recover` to regain access using your recovery phrase.",
                path.display(),
                actual,
                if actual == 1 { "" } else { "s" }
            )
        })?;
        Ok(key)
    } else {
        // Generate new key
        let key = generate_device_key().context("Failed to generate device key")?;

        // Save with restricted permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)
                .context("Failed to create device key file")?
                .write_all(&key)
                .context("Failed to write device key")?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&path, &key).context("Failed to write device key")?;
            // WARNING: On Windows, NTFS default ACLs may allow other local users
            // to read this file.  Windows DPAPI encryption is tracked in issue #62.
            // Until then, Windows users should ensure their home directory is not
            // world-readable (e.g. on a shared machine).
            eprintln!(
                "warning: device key written without OS-level encryption (Windows DPAPI not yet supported, see issue #62)"
            );
        }

        tracing::info!("Generated new device key at {:?}", path);
        Ok(key)
    }
}

/// Check if vault exists.
pub fn vault_exists() -> Result<bool> {
    Ok(vault_path()?.exists())
}
