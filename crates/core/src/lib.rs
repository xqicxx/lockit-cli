//! # lockit-core
//!
//! Cryptographic foundation for lockit credential manager.
//!
//! This crate provides a single public type [`UnlockedVault`] that manages
//! all cryptographic operations internally. Lower-level crypto primitives
//! (KDF, cipher) are private and cannot be accessed directly.
//!
//! ## Security Model
//!
//! - All encryption operations are internal — external code never handles raw keys
//! - Nonce generation is automatic and internal — callers cannot inject nonces
//! - [`Secret<Vec<u8>>`] wraps all sensitive data with zeroize-on-drop
//!
//! ## Example
//!
//! ```ignore
//! use lockit_core::{UnlockedVault, Secret};
//!
//! // Create new vault
//! let device_key = lockit_core::generate_device_key()?;
//! let mut vault = UnlockedVault::init("my-password", &device_key)?;
//!
//! // Store a credential
//! vault.set("myapp", "api_key", &Secret::new(b"secret-value".to_vec()))?;
//!
//! // Retrieve a credential
//! let value = vault.get("myapp", "api_key")?;
//!
//! // Save and lock
//! vault.save_to(&path)?;
//! vault.lock();
//! ```

pub mod error;

// Re-export public types only
pub use error::{Error, Result};
pub use secrecy::Secret;

// Public constants
/// Cryptographic key size in bytes (256 bits).
pub const KEY_SIZE: usize = 32;

/// Salt size in bytes (128 bits).
pub const SALT_SIZE: usize = 16;

/// Nonce size in bytes (96 bits for AES-GCM).
pub const NONCE_SIZE: usize = 12;

/// Vault file magic bytes.
pub const MAGIC: &[u8; 8] = b"LOCKIT01";

/// Current vault file format version.
/// v2: single AES-GCM encrypted blob replaces per-entry encryption + HMAC.
/// v3: named (map-based) msgpack encoding; optional BIP39 recovery_wrapped_vek field.
pub const VERSION: u16 = 3;

// Internal modules (not re-exported)
mod cipher;
mod kdf;
mod memory;
mod vault;

// Re-export the main public type
pub use vault::UnlockedVault;

/// Generates a random salt for key derivation.
pub fn generate_salt() -> Result<[u8; SALT_SIZE]> {
    use rand::RngCore;
    let mut salt = [0u8; SALT_SIZE];
    rand::rng().fill_bytes(&mut salt);
    Ok(salt)
}

/// Generates a random device key.
pub fn generate_device_key() -> Result<[u8; KEY_SIZE]> {
    use rand::RngCore;
    let mut key = [0u8; KEY_SIZE];
    rand::rng().fill_bytes(&mut key);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt() {
        let salt = generate_salt().unwrap();
        assert_eq!(salt.len(), SALT_SIZE);
    }

    #[test]
    fn test_generate_device_key() {
        let key = generate_device_key().unwrap();
        assert_eq!(key.len(), KEY_SIZE);
    }

    #[test]
    fn test_salts_are_unique() {
        let salt1 = generate_salt().unwrap();
        let salt2 = generate_salt().unwrap();
        assert_ne!(salt1, salt2);
    }
}
