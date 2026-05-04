use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod google_drive;
pub mod oauth;

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

pub const GOOGLE_DRIVE_APPDATA_FOLDER: &str = "appDataFolder";
pub const GOOGLE_DRIVE_SYNC_FOLDER: &str = "lockit-sync";
pub const GOOGLE_DRIVE_VAULT_FILE: &str = "vault.enc";
pub const GOOGLE_DRIVE_MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncManifest {
    pub version: u32,
    pub vault_checksum: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub encrypted_size: u64,
    pub schema_version: u32,
}

impl SyncManifest {
    pub fn new(
        vault_checksum: String,
        updated_by: impl Into<String>,
        encrypted_size: u64,
        schema_version: u32,
    ) -> Self {
        Self {
            version: 2,
            vault_checksum,
            updated_at: Utc::now(),
            updated_by: updated_by.into(),
            encrypted_size,
            schema_version,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    NotConfigured,
    BackendError,
    NeverSynced,
    UpToDate,
    LocalAhead,
    CloudAhead,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct SyncInputs {
    pub local_checksum: String,
    pub cloud_manifest: Option<SyncManifest>,
    pub last_sync_checksum: Option<String>,
    pub sync_key_configured: bool,
    pub backend_configured: bool,
}

pub fn compute_sync_status(input: SyncInputs) -> SyncStatus {
    if !input.sync_key_configured {
        return SyncStatus::NotConfigured;
    }
    if !input.backend_configured {
        return SyncStatus::BackendError;
    }
    let Some(cloud) = input.cloud_manifest else {
        return SyncStatus::NeverSynced;
    };

    if cloud.vault_checksum == input.local_checksum {
        return SyncStatus::UpToDate;
    }

    match input.last_sync_checksum {
        None => SyncStatus::CloudAhead,
        Some(last) if cloud.vault_checksum == last => SyncStatus::LocalAhead,
        Some(last) if input.local_checksum == last => SyncStatus::CloudAhead,
        Some(_) => SyncStatus::Conflict,
    }
}

pub fn sha256_checksum(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

pub trait SyncBackend {
    fn name(&self) -> &str;
    fn is_configured(&self) -> bool;
    fn upload_vault(&self, encrypted_data: &[u8], manifest: &SyncManifest)
        -> Result<(), SyncError>;
    fn download_vault(&self) -> Result<Vec<u8>, SyncError>;
    fn get_manifest(&self) -> Result<Option<SyncManifest>, SyncError>;
    fn delete_sync_data(&self) -> Result<(), SyncError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("sync backend is not configured")]
    NotConfigured,
    #[error("invalid sync key")]
    InvalidKey,
    #[error("unsupported sync blob version {0}")]
    UnsupportedVersion(u8),
    #[error("invalid sync blob")]
    InvalidBlob,
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed")]
    DecryptFailed,
    #[error("base64 decode failed")]
    Base64(#[from] base64::DecodeError),
    #[error("HTTP error: {0}")]
    HttpError(String),
}

pub struct SyncCrypto;

impl SyncCrypto {
    pub fn generate_key() -> [u8; KEY_LEN] {
        let mut key = [0u8; KEY_LEN];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    pub fn encode_key(key: &[u8; KEY_LEN]) -> String {
        B64.encode(key)
    }

    pub fn decode_key(encoded: &str) -> Result<[u8; KEY_LEN], SyncError> {
        let decoded = B64.decode(encoded.trim())?;
        if decoded.len() != KEY_LEN {
            return Err(SyncError::InvalidKey);
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(&decoded);
        Ok(key)
    }

    pub fn encrypt(plaintext: &[u8], sync_key: &[u8; KEY_LEN]) -> Result<Vec<u8>, SyncError> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce);
        let cipher = Aes256Gcm::new_from_slice(sync_key).map_err(|_| SyncError::InvalidKey)?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| SyncError::EncryptFailed)?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn decrypt(blob: &[u8], sync_key: &[u8; KEY_LEN]) -> Result<Vec<u8>, SyncError> {
        if blob.len() <= 1 + NONCE_LEN + 16 {
            return Err(SyncError::InvalidBlob);
        }
        if blob[0] != VERSION {
            return Err(SyncError::UnsupportedVersion(blob[0]));
        }
        let nonce = &blob[1..1 + NONCE_LEN];
        let ciphertext = &blob[1 + NONCE_LEN..];
        let cipher = Aes256Gcm::new_from_slice(sync_key).map_err(|_| SyncError::InvalidKey)?;
        cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| SyncError::DecryptFailed)
    }
}
