//! Vault encryption key used by sync.

use crate::sync_crypto as crypto;

pub const VAULT_FILE: &str = "vault.enc";
pub const MANIFEST_FILE: &str = "manifest.json";

/// Wrapper around a 256-bit sync key.
#[derive(Clone)]
pub struct VaultKey {
    key: [u8; 32],
}

impl VaultKey {
    /// Create from raw bytes.
    pub fn from_bytes(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Create from Base64-encoded string.
    pub fn from_base64(encoded: &str) -> crate::error::Result<Self> {
        let key = crypto::decode_sync_key(encoded)?;
        Ok(Self { key })
    }

    /// Generate a new random key.
    pub fn generate() -> Self {
        Self {
            key: crypto::generate_sync_key(),
        }
    }

    /// Encode to Base64 for display/sharing.
    pub fn to_base64(&self) -> String {
        crypto::encode_sync_key(&self.key)
    }

    /// Encrypt plaintext bytes.
    pub fn encrypt(&self, plaintext: &[u8]) -> crate::error::Result<Vec<u8>> {
        crypto::encrypt(plaintext, &self.key)
    }

    /// Decrypt encrypted bytes.
    pub fn decrypt(&self, blob: &[u8]) -> crate::error::Result<Vec<u8>> {
        crypto::decrypt(blob, &self.key)
    }
}
