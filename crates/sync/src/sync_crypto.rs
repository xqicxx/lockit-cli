//! Sync Key encryption/decryption — matches Android `SyncCrypto`.
//!
//! Format: Version(1 byte) + Nonce(12 bytes) + AES-256-GCM ciphertext

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};

const VERSION: u8 = 0x01;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;
const KEY_SIZE: usize = 32;

/// Generate a new 256-bit Sync Key.
pub fn generate_sync_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    rand::fill(&mut key);
    key
}

/// Encode Sync Key to Base64 for QR code / manual input.
pub fn encode_sync_key(key: &[u8; KEY_SIZE]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(key)
}

/// Decode Sync Key from Base64.
pub fn decode_sync_key(encoded: &str) -> Result<[u8; KEY_SIZE], crate::Error> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| crate::Error::Config(format!("Invalid Base64 sync key: {e}")))?;
    let len = bytes.len();
    bytes
        .try_into()
        .map_err(|_| crate::Error::Config(format!("Sync Key must be {KEY_SIZE} bytes, got {len}")))
}

/// Encrypt plaintext bytes with Sync Key.
/// Returns formatted blob: Version + Nonce + Ciphertext.
/// SAFETY: Propagates encryption failure instead of panicking.
pub fn encrypt(plaintext: &[u8], sync_key: &[u8; KEY_SIZE]) -> Result<Vec<u8>, crate::Error> {
    let nonce_bytes: [u8; NONCE_SIZE] = {
        let mut buf = [0u8; NONCE_SIZE];
        rand::fill(&mut buf);
        buf
    };
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(sync_key));
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| crate::Error::Config(format!("AES-256-GCM encryption failed: {e}")))?;

    let mut output = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
    output.push(VERSION);
    output.extend_from_slice(nonce_bytes.as_slice());
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt vault.enc blob with Sync Key.
pub fn decrypt(blob: &[u8], sync_key: &[u8; KEY_SIZE]) -> Result<Vec<u8>, crate::Error> {
    if blob.len() < 1 + NONCE_SIZE + TAG_SIZE {
        return Err(crate::Error::Config("Encrypted blob too short".into()));
    }
    if blob[0] != VERSION {
        return Err(crate::Error::Config(format!(
            "Unsupported vault.enc version: {}, expected {}",
            blob[0], VERSION
        )));
    }

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(sync_key));
    let nonce = Nonce::from_slice(&blob[1..1 + NONCE_SIZE]);
    cipher
        .decrypt(nonce, &blob[1 + NONCE_SIZE..])
        .map_err(|e| crate::Error::Config(format!("Decryption failed: {e}")))
}

/// Check if blob is valid vault.enc format.
pub fn is_valid_encrypted_blob(blob: &[u8]) -> bool {
    blob.len() > 1 + NONCE_SIZE + TAG_SIZE && blob[0] == VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = generate_sync_key();
        let plaintext = b"test vault data";
        let encrypted = encrypt(plaintext, &key).unwrap();
        assert!(is_valid_encrypted_blob(&encrypted));
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let key = generate_sync_key();
        let encoded = encode_sync_key(&key);
        let decoded = decode_sync_key(&encoded).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1 = generate_sync_key();
        let key2 = generate_sync_key();
        let encrypted = encrypt(b"secret", &key1).unwrap();
        assert!(decrypt(&encrypted, &key2).is_err());
    }
}
