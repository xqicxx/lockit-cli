use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const VAULT_ENVELOPE_VERSION: u8 = 2;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CryptoParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl CryptoParams {
    pub fn default_for_new_vault() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultEnvelope {
    pub version: u8,
    pub cipher: String,
    pub kdf: String,
    pub params: CryptoParams,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug)]
pub struct OpenedVaultBytes {
    pub plaintext: Vec<u8>,
    pub key: VaultMasterKey,
}

#[derive(Debug)]
pub struct VaultMasterKey {
    pub params: CryptoParams,
    pub salt: Vec<u8>,
    key: [u8; KEY_LEN],
}

impl VaultMasterKey {
    pub fn key_bytes(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    pub fn from_parts(params: CryptoParams, salt: Vec<u8>, key: [u8; KEY_LEN]) -> Self {
        Self { params, salt, key }
    }
}

impl Drop for VaultMasterKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid vault envelope")]
    InvalidEnvelope,
    #[error("unsupported vault version {0}")]
    UnsupportedVersion(u8),
    #[error("invalid base64 data")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("invalid crypto parameters")]
    InvalidParams,
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed")]
    DecryptFailed,
    #[error("serialization failed")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

pub fn encrypt_vault_bytes(
    plaintext: &[u8],
    password: &str,
    params: &CryptoParams,
) -> Result<Vec<u8>> {
    let mut salt = vec![0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut key = derive_key(password, &salt, params)?;
    let result = seal_with_key(plaintext, &key, &salt, params);
    key.zeroize();
    result
}

pub fn decrypt_vault_bytes(encrypted: &[u8], password: &str) -> Result<Vec<u8>> {
    open_vault_bytes(encrypted, password).map(|opened| opened.plaintext)
}

pub fn open_vault_bytes(encrypted: &[u8], password: &str) -> Result<OpenedVaultBytes> {
    let envelope: VaultEnvelope = serde_json::from_slice(encrypted)?;
    if envelope.version != VAULT_ENVELOPE_VERSION {
        return Err(CryptoError::UnsupportedVersion(envelope.version));
    }
    if envelope.cipher != "AES-256-GCM" || envelope.kdf != "argon2id" {
        return Err(CryptoError::InvalidEnvelope);
    }

    let salt = B64.decode(envelope.salt)?;
    let nonce = B64.decode(envelope.nonce)?;
    let ciphertext = B64.decode(envelope.ciphertext)?;
    if nonce.len() != NONCE_LEN {
        return Err(CryptoError::InvalidEnvelope);
    }

    let key = derive_key(password, &salt, &envelope.params)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| CryptoError::InvalidParams)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptFailed)?;

    Ok(OpenedVaultBytes {
        plaintext,
        key: VaultMasterKey {
            params: envelope.params,
            salt,
            key,
        },
    })
}

pub fn seal_opened_vault_bytes(plaintext: &[u8], master_key: &VaultMasterKey) -> Result<Vec<u8>> {
    seal_with_key(
        plaintext,
        &master_key.key,
        &master_key.salt,
        &master_key.params,
    )
}

fn seal_with_key(
    plaintext: &[u8],
    key: &[u8; KEY_LEN],
    salt: &[u8],
    params: &CryptoParams,
) -> Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidParams)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| CryptoError::EncryptFailed)?;

    let envelope = VaultEnvelope {
        version: VAULT_ENVELOPE_VERSION,
        cipher: "AES-256-GCM".to_string(),
        kdf: "argon2id".to_string(),
        params: params.clone(),
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        ciphertext: B64.encode(ciphertext),
    };
    serde_json::to_vec_pretty(&envelope).map_err(Into::into)
}

fn derive_key(password: &str, salt: &[u8], params: &CryptoParams) -> Result<[u8; KEY_LEN]> {
    let params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::InvalidParams)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| CryptoError::InvalidParams)?;
    Ok(key)
}
