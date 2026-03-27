//! Vault management (PUBLIC API).
//!
//! [`UnlockedVault`] is the single public type that exposes all operations.
//! All crypto is handled internally — callers never handle keys or nonces.
//!
//! ## Design
//!
//! Credentials are held in a `HashMap` in memory after `open()`.
//! On `save()`, the entire map is serialized once and AES-256-GCM encrypted.
//! AES-GCM's AEAD tag provides file-level integrity — no separate HMAC needed.
//!
//! ## BIP39 Recovery
//!
//! Use `init_with_recovery()` instead of `init()` to get a 24-word mnemonic.
//! The mnemonic encodes a recovery key that can unwrap the VEK independently
//! of the master password. Use `recover_with_mnemonic()` to reset the password.
//!
//! Complexity: set/get/delete are O(1); save is O(N) but runs only on write.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bip39::Mnemonic;
use rand::RngCore;
use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::{
    MAGIC, SALT_SIZE, VERSION,
    cipher::{self, CipherText},
    error::{Error, Result},
    generate_salt, kdf,
    memory::VaultEncryptionKey,
};

/// Serializable form of a single credential (used for msgpack encoding).
///
/// The `expires_at` field is optional and defaults to `None` so that vaults
/// written by older versions of lockit (which lack the field) can still be
/// read — `rmp_serde` with `#[serde(default)]` fills in `None` when the field
/// is absent from the msgpack map.
#[derive(Serialize, Deserialize)]
struct CredentialEntry {
    profile: String,
    key: String,
    value: Vec<u8>,
    /// Unix timestamp (seconds since epoch) after which this credential should
    /// be considered expired.  `None` means no expiry.
    #[serde(default)]
    expires_at: Option<u64>,
}

/// Vault file data for serialization.
/// Uses named (map-based) msgpack encoding for forward compatibility.
#[derive(Serialize, Deserialize)]
struct VaultFileData {
    magic: [u8; 8],
    version: u16,
    salt: [u8; SALT_SIZE],
    wrapped_vek: Vec<u8>,
    /// AES-256-GCM encrypted msgpack blob of `Vec<CredentialEntry>`.
    /// AEAD authentication tag provides file-level tamper detection.
    encrypted_entries: Vec<u8>,
    /// Optional recovery-key-wrapped VEK for BIP39 mnemonic recovery.
    /// Present only when vault was initialized with `init_with_recovery()`.
    #[serde(default)]
    recovery_wrapped_vek: Option<Vec<u8>>,
}

/// Unlocked vault — holds credentials in memory and provides all CRUD operations.
pub struct UnlockedVault {
    file: VaultFileData,
    vek: VaultEncryptionKey,
    device_key: Secret<[u8; 32]>,
    path: Option<PathBuf>,
    dirty: bool,
    /// Decrypted credentials loaded into memory on open(). O(1) access.
    /// Value is `(raw_bytes, expires_at_unix_secs)`.
    credentials: HashMap<(String, String), (Vec<u8>, Option<u64>)>,
}

impl Drop for UnlockedVault {
    /// Zeroize all credential values when the vault is dropped.
    fn drop(&mut self) {
        for (value, _) in self.credentials.values_mut() {
            value.zeroize();
        }
    }
}

impl UnlockedVault {
    /// Create a new vault with password and device key (no recovery phrase).
    pub fn init(password: &str, device_key: &[u8; 32]) -> Result<Self> {
        let salt = generate_salt()?;
        let mk = kdf::derive_master_key(password, &salt, device_key)?.into_wrapping_key();
        let vek = VaultEncryptionKey::generate()?;
        let wrapped_vek = cipher::wrap_key(&mk, vek.expose())?;

        let mut entries_bytes = rmp_serde::to_vec_named(&Vec::<CredentialEntry>::new())
            .map_err(|e| Error::Encryption(e.to_string()))?;
        let encrypted_entries = CipherText::encrypt(&vek, &entries_bytes)?;
        entries_bytes.zeroize();

        let file = VaultFileData {
            magic: *MAGIC,
            version: VERSION,
            salt,
            wrapped_vek: wrapped_vek.into_bytes(),
            encrypted_entries: encrypted_entries.into_bytes(),
            recovery_wrapped_vek: None,
        };

        Ok(Self {
            file,
            vek,
            device_key: Secret::new(*device_key),
            path: None,
            dirty: false,
            credentials: HashMap::new(),
        })
    }

    /// Create a new vault with password, device key, and BIP39 recovery phrase.
    /// Returns `(vault, mnemonic_phrase)`. The phrase must be shown to the user
    /// exactly once — it is NOT stored anywhere in the vault file.
    pub fn init_with_recovery(password: &str, device_key: &[u8; 32]) -> Result<(Self, String)> {
        let mut vault = Self::init(password, device_key)?;

        // Generate 32 bytes of cryptographic entropy → 24-word BIP39 mnemonic.
        let mut raw_entropy = [0u8; 32];
        rand::rng().fill_bytes(&mut raw_entropy);
        let mnemonic = Mnemonic::from_entropy(&raw_entropy)
            .map_err(|e| Error::Encryption(format!("BIP39 generation failed: {}", e)))?;
        let entropy = mnemonic.to_entropy();
        let recovery_key = kdf::derive_recovery_key(&entropy)?;
        let recovery_wrapped_vek = cipher::wrap_key(&recovery_key, vault.vek.expose())?;
        vault.file.recovery_wrapped_vek = Some(recovery_wrapped_vek.into_bytes());

        Ok((vault, mnemonic.to_string()))
    }

    /// Open an existing vault file.
    pub fn open(path: &Path, password: &str, device_key: &[u8; 32]) -> Result<Self> {
        let data = std::fs::read(path)?;

        let mut file: VaultFileData =
            rmp_serde::from_slice(&data).map_err(|e| Error::InvalidVault(e.to_string()))?;

        if &file.magic != MAGIC {
            return Err(Error::InvalidVault(
                "not a lockit vault file (invalid magic bytes)".into(),
            ));
        }
        if file.version > VERSION {
            return Err(Error::InvalidVault(format!(
                "vault was created by a newer version of lockit (format v{}, \
                 this binary supports up to v{}). Please upgrade lockit.",
                file.version, VERSION
            )));
        }
        if file.version < VERSION {
            // Older format: load and auto-upgrade to current VERSION on next save.
            // All new fields use #[serde(default)] so deserialization is safe.
            tracing::warn!(
                old = file.version,
                new = VERSION,
                "vault format upgrade: will be written as v{} on next save",
                VERSION
            );
            file.version = VERSION;
        }

        let mk = kdf::derive_master_key(password, &file.salt, device_key)?.into_wrapping_key();

        // Unwrap VEK — AES-GCM auth failure here means wrong password.
        let vek = VaultEncryptionKey::from_secret(
            cipher::unwrap_key(&mk, &file.wrapped_vek).map_err(|_| Error::IncorrectPassword)?,
        );

        // Decrypt entries — auth failure here means file corruption/tampering.
        let mut entries_bytes = CipherText::decrypt(&vek, &file.encrypted_entries)?;
        let entries: Vec<CredentialEntry> =
            rmp_serde::from_slice(&entries_bytes).map_err(|_| Error::VaultCorrupted)?;
        entries_bytes.zeroize();

        let credentials = entries
            .into_iter()
            .map(|e| ((e.profile, e.key), (e.value, e.expires_at)))
            .collect();

        Ok(Self {
            file,
            vek,
            device_key: Secret::new(*device_key),
            path: Some(path.to_path_buf()),
            dirty: false,
            credentials,
        })
    }

    /// Recover vault access using a BIP39 mnemonic, resetting the master password.
    /// Writes the updated vault file atomically.
    pub fn recover_with_mnemonic(
        path: &Path,
        mnemonic_phrase: &str,
        new_password: &str,
        device_key: &[u8; 32],
    ) -> Result<()> {
        let data = std::fs::read(path)?;
        let mut file: VaultFileData =
            rmp_serde::from_slice(&data).map_err(|e| Error::InvalidVault(e.to_string()))?;

        if &file.magic != MAGIC {
            return Err(Error::InvalidVault("invalid magic bytes".into()));
        }

        let recovery_wrapped_vek = file
            .recovery_wrapped_vek
            .as_ref()
            .ok_or_else(|| Error::InvalidVault("vault has no recovery key".into()))?;

        let mnemonic: Mnemonic = mnemonic_phrase
            .trim()
            .parse()
            .map_err(|_| Error::InvalidVault("invalid mnemonic phrase".into()))?;
        let entropy = mnemonic.to_entropy();
        let recovery_key = kdf::derive_recovery_key(&entropy)?;

        let vek = VaultEncryptionKey::from_secret(
            cipher::unwrap_key(&recovery_key, recovery_wrapped_vek)
                .map_err(|_| Error::InvalidVault("mnemonic does not match this vault".into()))?,
        );

        // Re-wrap VEK with new password and fresh salt.
        let new_salt = generate_salt()?;
        let mk = kdf::derive_master_key(new_password, &new_salt, device_key)?.into_wrapping_key();
        let new_wrapped_vek = cipher::wrap_key(&mk, vek.expose())?;

        file.salt = new_salt;
        file.wrapped_vek = new_wrapped_vek.into_bytes();
        file.version = VERSION;

        let serialized =
            rmp_serde::to_vec_named(&file).map_err(|e| Error::InvalidVault(e.to_string()))?;
        atomic_write(path, &serialized)?;

        Ok(())
    }

    /// Save to the file this vault was opened from.
    pub fn save(&self) -> Result<()> {
        let path = self.path.as_ref().ok_or(Error::NoPath)?;
        self.save_to_internal(path)
    }

    /// Save to a specific path. Updates internal path.
    pub fn save_to(&mut self, path: &Path) -> Result<()> {
        self.save_to_internal(path)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    fn save_to_internal(&self, path: &Path) -> Result<()> {
        // Collect and sort entries for deterministic output.
        let mut entries: Vec<CredentialEntry> = self
            .credentials
            .iter()
            .map(|((profile, key), (value, expires_at))| CredentialEntry {
                profile: profile.clone(),
                key: key.clone(),
                value: value.clone(),
                expires_at: *expires_at,
            })
            .collect();
        entries.sort_by(|a, b| a.profile.cmp(&b.profile).then(a.key.cmp(&b.key)));

        // Serialize, encrypt, then zeroize the plaintext buffer immediately.
        let mut entries_bytes =
            rmp_serde::to_vec_named(&entries).map_err(|e| Error::InvalidVault(e.to_string()))?;
        let encrypted_entries = CipherText::encrypt(&self.vek, &entries_bytes)?;
        entries_bytes.zeroize();

        let file_data = VaultFileData {
            magic: self.file.magic,
            version: self.file.version,
            salt: self.file.salt,
            wrapped_vek: self.file.wrapped_vek.clone(),
            encrypted_entries: encrypted_entries.into_bytes(),
            recovery_wrapped_vek: self.file.recovery_wrapped_vek.clone(),
        };

        let data =
            rmp_serde::to_vec_named(&file_data).map_err(|e| Error::InvalidVault(e.to_string()))?;
        atomic_write(path, &data)
    }

    /// Set the default save path.
    pub fn set_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
    }

    /// Lock the vault (consume self, zeroize credentials and VEK via Drop).
    pub fn lock(self) {
        drop(self);
    }

    /// Set a credential value. O(1).
    pub fn set<K: AsRef<str>>(
        &mut self,
        profile: K,
        key: K,
        value: &Secret<Vec<u8>>,
    ) -> Result<()> {
        self.set_with_expiry(profile, key, value, None)
    }

    /// Set a credential value with an optional expiry timestamp. O(1).
    ///
    /// `expires_at` is a Unix timestamp (seconds since epoch); `None` means
    /// the credential never expires.
    pub fn set_with_expiry<K: AsRef<str>>(
        &mut self,
        profile: K,
        key: K,
        value: &Secret<Vec<u8>>,
        expires_at: Option<u64>,
    ) -> Result<()> {
        self.credentials.insert(
            (profile.as_ref().to_string(), key.as_ref().to_string()),
            (value.expose_secret().clone(), expires_at),
        );
        self.dirty = true;
        Ok(())
    }

    /// Get a credential value. O(1).
    pub fn get<K: AsRef<str>>(&self, profile: K, key: K) -> Result<Option<Secret<Vec<u8>>>> {
        Ok(self
            .credentials
            .get(&(profile.as_ref().to_string(), key.as_ref().to_string()))
            .map(|(v, _)| Secret::new(v.clone())))
    }

    /// Get the expiry timestamp of a credential. Returns `None` if the
    /// credential does not exist or has no expiry set.
    pub fn get_expiry<K: AsRef<str>>(&self, profile: K, key: K) -> Option<u64> {
        self.credentials
            .get(&(profile.as_ref().to_string(), key.as_ref().to_string()))
            .and_then(|(_, exp)| *exp)
    }

    /// Return all credentials whose `expires_at` is before `now_unix_secs`,
    /// sorted by (profile, key).
    pub fn expired_credentials(&self, now_unix_secs: u64) -> Vec<(String, String, u64)> {
        let mut result: Vec<(String, String, u64)> = self
            .credentials
            .iter()
            .filter_map(|((p, k), (_, exp))| {
                exp.filter(|&ts| ts <= now_unix_secs)
                    .map(|ts| (p.clone(), k.clone(), ts))
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        result
    }

    /// Return all credentials that expire within the next `warn_secs` seconds
    /// (but have not yet expired), sorted by expiry ascending.
    pub fn expiring_soon(&self, now_unix_secs: u64, warn_secs: u64) -> Vec<(String, String, u64)> {
        let deadline = now_unix_secs + warn_secs;
        let mut result: Vec<(String, String, u64)> = self
            .credentials
            .iter()
            .filter_map(|((p, k), (_, exp))| {
                exp.filter(|&ts| ts > now_unix_secs && ts <= deadline)
                    .map(|ts| (p.clone(), k.clone(), ts))
            })
            .collect();
        result.sort_by_key(|r| r.2);
        result
    }

    /// Delete a credential. Returns `true` if it existed. O(1).
    pub fn delete<K: AsRef<str>>(&mut self, profile: K, key: K) -> Result<bool> {
        let removed = self
            .credentials
            .remove(&(profile.as_ref().to_string(), key.as_ref().to_string()))
            .is_some();
        if removed {
            self.dirty = true;
        }
        Ok(removed)
    }

    /// List all profiles.
    pub fn profiles(&self) -> Result<Vec<String>> {
        let profiles: std::collections::HashSet<String> =
            self.credentials.keys().map(|(p, _)| p.clone()).collect();
        Ok(profiles.into_iter().collect())
    }

    /// List keys in a profile.
    pub fn keys<K: AsRef<str>>(&self, profile: K) -> Result<Vec<String>> {
        let profile = profile.as_ref();
        Ok(self
            .credentials
            .keys()
            .filter(|(p, _)| p == profile)
            .map(|(_, k)| k.clone())
            .collect())
    }

    /// Check if a credential exists. O(1).
    pub fn contains<K: AsRef<str>>(&self, profile: K, key: K) -> Result<bool> {
        Ok(self.get(profile, key)?.is_some())
    }

    /// Total number of credentials stored across all profiles.
    pub fn credential_count(&self) -> usize {
        self.credentials.len()
    }

    /// Change master password (generates new salt to prevent correlation attacks).
    pub fn change_password(&mut self, new_password: &str) -> Result<()> {
        let new_salt = generate_salt()?;
        let mk = kdf::derive_master_key(new_password, &new_salt, self.device_key.expose_secret())?
            .into_wrapping_key();
        let wrapped_vek = cipher::wrap_key(&mk, self.vek.expose())?;
        self.file.salt = new_salt;
        self.file.wrapped_vek = wrapped_vek.into_bytes();
        self.dirty = true;
        Ok(())
    }

    /// Get vault file path.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Atomic file write: write to temp, then rename (POSIX atomic).
fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let temp_path = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&temp_path, data)?;
    }

    std::fs::rename(&temp_path, path)?;

    // fsync the parent directory so the renamed directory entry is durable.
    // Without this, a crash after rename but before the OS flushes the directory
    // journal could leave the vault file absent even though the data is on disk.
    #[cfg(unix)]
    {
        if let Some(parent) = path.parent() {
            let dir = std::fs::File::open(parent)?;
            dir.sync_all()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_init_and_unlock() {
        let device_key = [1u8; 32];
        let vault = UnlockedVault::init("password", &device_key).unwrap();
        assert!(vault.path().is_none());
    }

    #[test]
    fn test_set_and_get() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("myapp", "api_key", &Secret::new(b"secret".to_vec()))
            .unwrap();

        let value = vault.get("myapp", "api_key").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().expose_secret(), b"secret");
    }

    #[test]
    fn test_get_nonexistent() {
        let device_key = [1u8; 32];
        let vault = UnlockedVault::init("password", &device_key).unwrap();
        let value = vault.get("nonexistent", "key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_delete() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("myapp", "key", &Secret::new(b"value".to_vec()))
            .unwrap();

        let deleted = vault.delete("myapp", "key").unwrap();
        assert!(deleted);

        let value = vault.get("myapp", "key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_profiles() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("app1", "key", &Secret::new(b"v".to_vec()))
            .unwrap();
        vault
            .set("app2", "key", &Secret::new(b"v".to_vec()))
            .unwrap();

        let profiles = vault.profiles().unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains(&"app1".to_string()));
        assert!(profiles.contains(&"app2".to_string()));
    }

    #[test]
    fn test_keys() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("myapp", "key1", &Secret::new(b"v".to_vec()))
            .unwrap();
        vault
            .set("myapp", "key2", &Secret::new(b"v".to_vec()))
            .unwrap();

        let keys = vault.keys("myapp").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_save_and_open() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("myapp", "key", &Secret::new(b"value".to_vec()))
            .unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        let vault2 = UnlockedVault::open(temp_file.path(), "password", &device_key).unwrap();
        let value = vault2.get("myapp", "key").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().expose_secret(), b"value");
    }

    #[test]
    fn test_wrong_password_fails() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        let result = UnlockedVault::open(temp_file.path(), "wrong", &device_key);
        assert!(matches!(result, Err(Error::IncorrectPassword)));
    }

    #[test]
    fn test_overwrite_entry() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault
            .set("app", "key", &Secret::new(b"v1".to_vec()))
            .unwrap();
        vault
            .set("app", "key", &Secret::new(b"v2".to_vec()))
            .unwrap();

        let value = vault.get("app", "key").unwrap().unwrap();
        assert_eq!(value.expose_secret(), b"v2");
    }

    #[test]
    fn test_change_password_generates_new_salt() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        let original_salt = vault.file.salt;
        vault.change_password("newpassword").unwrap();
        assert_ne!(vault.file.salt, original_salt);

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        let vault2 = UnlockedVault::open(temp_file.path(), "newpassword", &device_key).unwrap();
        assert!(vault2.get("any", "key").unwrap().is_none());
    }

    #[test]
    fn test_init_with_recovery_phrase() {
        let device_key = [1u8; 32];
        let (vault, mnemonic) = UnlockedVault::init_with_recovery("password", &device_key).unwrap();

        // Mnemonic should be 24 words
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 24);

        // Recovery key should be stored in vault file
        assert!(vault.file.recovery_wrapped_vek.is_some());
    }

    #[test]
    fn test_recover_with_mnemonic() {
        let device_key = [1u8; 32];
        let (mut vault, mnemonic) =
            UnlockedVault::init_with_recovery("old-password", &device_key).unwrap();

        vault
            .set("app", "key", &Secret::new(b"secret".to_vec()))
            .unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        // Recover with mnemonic
        UnlockedVault::recover_with_mnemonic(
            temp_file.path(),
            &mnemonic,
            "new-password",
            &device_key,
        )
        .unwrap();

        // Open with new password should succeed
        let recovered = UnlockedVault::open(temp_file.path(), "new-password", &device_key).unwrap();
        let value = recovered.get("app", "key").unwrap().unwrap();
        assert_eq!(value.expose_secret(), b"secret");

        // Old password should no longer work
        let result = UnlockedVault::open(temp_file.path(), "old-password", &device_key);
        assert!(matches!(result, Err(Error::IncorrectPassword)));
    }

    #[test]
    fn test_wrong_mnemonic_fails() {
        let device_key = [1u8; 32];
        let (mut vault, _mnemonic) =
            UnlockedVault::init_with_recovery("password", &device_key).unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        // Try recovery with a different valid mnemonic
        let (_, other_mnemonic) =
            UnlockedVault::init_with_recovery("password", &device_key).unwrap();
        let result = UnlockedVault::recover_with_mnemonic(
            temp_file.path(),
            &other_mnemonic,
            "new-password",
            &device_key,
        );
        assert!(result.is_err());
    }

    /// Verify vault file is created with restrictive permissions (Issue #26).
    #[cfg(unix)]
    #[test]
    fn test_vault_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();
        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        let metadata = std::fs::metadata(temp_file.path()).unwrap();
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o600,
            "vault file must be readable only by owner"
        );
    }
}
