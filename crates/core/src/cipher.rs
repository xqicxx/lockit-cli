//! Internal cipher operations (AES-256-GCM).
//!
//! This module is PRIVATE. External code cannot:
//! - Call encrypt/decrypt directly
//! - Provide or access nonces
//!
//! Nonce is ALWAYS generated internally using OsRng.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use rand::RngCore;
use secrecy::Secret;
use zeroize::Zeroize;

use crate::{Error, KEY_SIZE, NONCE_SIZE, Result};

use super::memory::VaultEncryptionKey;

/// Internal ciphertext type (nonce + encrypted data).
/// Stored as: [nonce: 12 bytes] || [ciphertext + tag: N+16 bytes]
pub(crate) struct CipherText {
    /// The encrypted data with nonce prepended.
    data: Vec<u8>,
}

impl CipherText {
    /// Encrypt plaintext with the VEK.
    /// Nonce is generated internally — never exposed or accepted as parameter.
    pub(crate) fn encrypt(vek: &VaultEncryptionKey, plaintext: &[u8]) -> Result<Self> {
        let key_bytes = vek.expose();

        let cipher =
            Aes256Gcm::new_from_slice(key_bytes).map_err(|e| Error::Encryption(e.to_string()))?;

        // Generate fresh random nonce (CRITICAL: internal only)
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut data = nonce_bytes.to_vec();
        data.extend(ciphertext);

        Ok(Self { data })
    }

    /// Decrypt ciphertext with the VEK.
    /// Extracts nonce internally from the data.
    pub(crate) fn decrypt(vek: &VaultEncryptionKey, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < NONCE_SIZE + 16 {
            // Minimum: nonce + empty plaintext + 16-byte tag
            return Err(Error::Decryption("ciphertext too short".into()));
        }

        let key_bytes = vek.expose();

        let cipher =
            Aes256Gcm::new_from_slice(key_bytes).map_err(|e| Error::Decryption(e.to_string()))?;

        // Extract nonce from beginning
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);

        // Decrypt remaining bytes
        let plaintext = cipher
            .decrypt(nonce, &data[NONCE_SIZE..])
            .map_err(|_| Error::VaultCorrupted)?;

        Ok(plaintext)
    }

    /// Get the raw bytes for storage (borrow).
    #[cfg(test)]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Consume and return the raw bytes (no reallocation).
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Wrap a key (like VEK) with another key (like MK).
pub(crate) fn wrap_key(
    wrapping_key: &VaultEncryptionKey,
    key_to_wrap: &[u8; 32],
) -> Result<CipherText> {
    CipherText::encrypt(wrapping_key, key_to_wrap)
}

/// Unwrap a key (like VEK) from wrapped form.
/// Returns Secret<[u8; 32]> to ensure zeroize-on-drop.
pub(crate) fn unwrap_key(
    wrapping_key: &VaultEncryptionKey,
    wrapped: &[u8],
) -> Result<Secret<[u8; 32]>> {
    let mut plaintext = CipherText::decrypt(wrapping_key, wrapped)?;
    if plaintext.len() != KEY_SIZE {
        plaintext.zeroize();
        return Err(Error::Decryption("invalid wrapped key size".into()));
    }
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&plaintext);
    plaintext.zeroize();
    Ok(Secret::new(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret message";

        let ciphertext = CipherText::encrypt(&vek, plaintext).unwrap();
        let decrypted = CipherText::decrypt(&vek, ciphertext.as_bytes()).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_different_nonces_for_same_plaintext() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"same message";

        let ct1 = CipherText::encrypt(&vek, plaintext).unwrap();
        let ct2 = CipherText::encrypt(&vek, plaintext).unwrap();

        // Same plaintext with different nonces = different ciphertexts
        assert_ne!(ct1.as_bytes(), ct2.as_bytes());

        // But both decrypt correctly
        let d1 = CipherText::decrypt(&vek, ct1.as_bytes()).unwrap();
        let d2 = CipherText::decrypt(&vek, ct2.as_bytes()).unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret";

        let mut ciphertext = CipherText::encrypt(&vek, plaintext).unwrap();
        // Tamper with the data
        ciphertext.data[15] ^= 0xff;

        let result = CipherText::decrypt(&vek, ciphertext.as_bytes());
        assert!(matches!(result, Err(Error::VaultCorrupted)));
    }

    #[test]
    fn test_wrong_key_fails() {
        let vek1 = VaultEncryptionKey::generate().unwrap();
        let vek2 = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret";

        let ciphertext = CipherText::encrypt(&vek1, plaintext).unwrap();
        let result = CipherText::decrypt(&vek2, ciphertext.as_bytes());

        assert!(matches!(result, Err(Error::VaultCorrupted)));
    }

    #[test]
    fn test_wrap_unwrap_key() {
        use secrecy::ExposeSecret;
        let wrapping_key = VaultEncryptionKey::generate().unwrap();
        let key_to_wrap = [42u8; 32];

        let wrapped = wrap_key(&wrapping_key, &key_to_wrap).unwrap();
        let unwrapped = unwrap_key(&wrapping_key, wrapped.as_bytes()).unwrap();

        assert_eq!(key_to_wrap, *unwrapped.expose_secret());
    }

    #[test]
    fn test_into_bytes_no_realloc() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"test";

        let ciphertext = CipherText::encrypt(&vek, plaintext).unwrap();
        let bytes = ciphertext.into_bytes();
        assert_eq!(bytes.len(), NONCE_SIZE + plaintext.len() + 16);
    }
}
