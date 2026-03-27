//! Internal key derivation functions (Argon2id + HKDF).
//!
//! This module is PRIVATE. External code cannot:
//! - Call derive_master_key directly
//! - Access PasswordKey or MasterKey types

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use secrecy::{ExposeSecret, Secret};
use sha2::Sha256;
use zeroize::Zeroize;

use crate::{Error, KEY_SIZE, Result, SALT_SIZE};

use super::memory::VaultEncryptionKey;

/// Argon2id parameters (per technical design).
const ARGON2_MEMORY: u32 = 65536; // 64 MiB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Internal type for password-derived key.
/// Wrapped in Secret for zeroize-on-drop.
pub(crate) struct PasswordKey(Secret<[u8; KEY_SIZE]>);

impl PasswordKey {
    /// Derive from password using Argon2id.
    pub(crate) fn derive(password: &str, salt: &[u8; SALT_SIZE]) -> Result<Self> {
        let params = Params::new(
            ARGON2_MEMORY,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(KEY_SIZE),
        )
        .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = [0u8; KEY_SIZE];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let result = Self(Secret::new(key));
        // Clear intermediate buffer
        key.zeroize();
        Ok(result)
    }

    /// Access raw bytes (internal use only).
    fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        self.0.expose_secret()
    }
}

/// Internal type for master key.
/// Wrapped in Secret for zeroize-on-drop.
pub(crate) struct MasterKey(Secret<[u8; KEY_SIZE]>);

impl MasterKey {
    /// Derive from password key and device key using HKDF-SHA256.
    pub(crate) fn derive(password_key: &PasswordKey, device_key: &[u8; KEY_SIZE]) -> Result<Self> {
        // Mix password key and device key as input key material
        let mut ikm = [0u8; KEY_SIZE * 2];
        ikm[..KEY_SIZE].copy_from_slice(password_key.as_bytes());
        ikm[KEY_SIZE..].copy_from_slice(device_key);

        // HKDF with the combined key material as IKM
        let hkdf: Hkdf<Sha256> = Hkdf::new(None, &ikm);

        let mut master_key = [0u8; KEY_SIZE];
        hkdf.expand(b"lockit master key", &mut master_key)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        // Clear intermediate buffers
        ikm.zeroize();

        let result = Self(Secret::new(master_key));
        master_key.zeroize();
        Ok(result)
    }

    /// Convert to VaultEncryptionKey for wrapping/unwrapping.
    pub(crate) fn into_wrapping_key(self) -> VaultEncryptionKey {
        VaultEncryptionKey::new(*self.0.expose_secret())
    }
}

/// Derive a recovery key from BIP39 entropy using HKDF-SHA256.
/// The entropy is the 32-byte raw entropy from a 24-word BIP39 mnemonic.
/// This key is used to wrap/unwrap the VEK as a recovery mechanism.
pub(crate) fn derive_recovery_key(entropy: &[u8]) -> Result<VaultEncryptionKey> {
    let hkdf: Hkdf<Sha256> = Hkdf::new(None, entropy);
    let mut recovery_key = [0u8; KEY_SIZE];
    hkdf.expand(b"lockit recovery key v1", &mut recovery_key)
        .map_err(|e| Error::KeyDerivation(e.to_string()))?;
    let key = VaultEncryptionKey::new(recovery_key);
    recovery_key.zeroize();
    Ok(key)
}

/// Derive master key from password, salt, and device key.
/// This is the main entry point for KDF.
pub(crate) fn derive_master_key(
    password: &str,
    salt: &[u8; SALT_SIZE],
    device_key: &[u8; KEY_SIZE],
) -> Result<MasterKey> {
    let password_key = PasswordKey::derive(password, salt)?;
    MasterKey::derive(&password_key, device_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_key_derivation() {
        let salt = [1u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password", &salt).unwrap();
        let pk2 = PasswordKey::derive("password", &salt).unwrap();

        // Same input = same output (deterministic)
        assert_eq!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let salt = [1u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password1", &salt).unwrap();
        let pk2 = PasswordKey::derive("password2", &salt).unwrap();

        assert_ne!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_different_salts_different_keys() {
        let salt1 = [1u8; SALT_SIZE];
        let salt2 = [2u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password", &salt1).unwrap();
        let pk2 = PasswordKey::derive("password", &salt2).unwrap();

        assert_ne!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_master_key_derivation() {
        let salt = [1u8; SALT_SIZE];
        let device_key = [2u8; KEY_SIZE];

        let mk1 = derive_master_key("password", &salt, &device_key).unwrap();
        let mk2 = derive_master_key("password", &salt, &device_key).unwrap();

        // Same inputs = same master key
        let mk1 = mk1.into_wrapping_key();
        let mk2 = mk2.into_wrapping_key();
        assert_eq!(mk1.expose(), mk2.expose());
    }

    #[test]
    fn test_device_key_affects_master_key() {
        let salt = [1u8; SALT_SIZE];
        let device_key1 = [1u8; KEY_SIZE];
        let device_key2 = [2u8; KEY_SIZE];

        let mk1 = derive_master_key("password", &salt, &device_key1)
            .unwrap()
            .into_wrapping_key();
        let mk2 = derive_master_key("password", &salt, &device_key2)
            .unwrap()
            .into_wrapping_key();

        assert_ne!(mk1.expose(), mk2.expose());
    }
}
