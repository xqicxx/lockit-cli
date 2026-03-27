//! Internal memory types for secure key handling.
//!
//! This module is PRIVATE. External code cannot access these types.
//! All sensitive keys are wrapped in Secret<T> for zeroize-on-drop.

use secrecy::{ExposeSecret, Secret};

/// Internal wrapper for VEK (Vault Encryption Key).
///
/// Uses `secrecy::Secret` which zeroizes on drop, has no Debug/Display impls
/// (won't leak in logs), and requires explicit `.expose_secret()` to access.
pub(crate) struct VaultEncryptionKey {
    key: Secret<[u8; 32]>,
}

impl VaultEncryptionKey {
    /// Create from raw bytes (after derivation or unwrapping).
    pub(crate) fn new(bytes: [u8; 32]) -> Self {
        Self {
            key: Secret::new(bytes),
        }
    }

    /// Create from Secret<[u8; 32]> (after unwrapping from cipher).
    /// Avoids additional copies on the stack.
    pub(crate) fn from_secret(secret: Secret<[u8; 32]>) -> Self {
        Self { key: secret }
    }

    /// Access for encryption/decryption operations.
    pub(crate) fn expose(&self) -> &[u8; 32] {
        self.key.expose_secret()
    }

    /// Generate a random VEK.
    pub(crate) fn generate() -> crate::Result<Self> {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Ok(Self::new(bytes))
    }
}

// VEK is zeroized on drop via Secret<[u8; 32]>

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vek_generate() {
        let vek1 = VaultEncryptionKey::generate().unwrap();
        let vek2 = VaultEncryptionKey::generate().unwrap();

        // Keys should be different
        assert_ne!(vek1.expose(), vek2.expose());
    }
}
