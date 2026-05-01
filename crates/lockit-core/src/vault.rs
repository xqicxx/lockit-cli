use crate::credential::{Credential, CredentialDraft, RedactedCredential};
use crate::crypto::{encrypt_vault_bytes, open_vault_bytes, seal_opened_vault_bytes, CryptoParams, VaultMasterKey};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub vault_path: PathBuf,
}

impl VaultPaths {
    pub fn new(vault_path: PathBuf) -> Self {
        Self { vault_path }
    }

    pub fn platform_default() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "lockit", "Lockit").ok_or(VaultError::NoHomeDirectory)?;
        Ok(Self::new(dirs.data_dir().join("vault.enc")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultPayload {
    pub schema_version: u32,
    pub credentials: Vec<Credential>,
    pub audit_events: Vec<AuditEvent>,
}

impl Default for VaultPayload {
    fn default() -> Self {
        Self { schema_version: 2, credentials: Vec::new(), audit_events: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub event: String,
    pub credential_id: Option<String>,
    pub field: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("could not resolve user data directory")]
    NoHomeDirectory,
    #[error("vault already exists")]
    AlreadyExists,
    #[error("vault is not initialized")]
    NotInitialized,
    #[error("vault is locked")]
    Locked,
    #[error("credential not found")]
    CredentialNotFound,
    #[error("field not found")]
    FieldNotFound,
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("crypto error")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("serialization error")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, VaultError>;

pub fn init_vault(paths: &VaultPaths, password: &str) -> Result<()> {
    if paths.vault_path.exists() {
        return Err(VaultError::AlreadyExists);
    }
    if let Some(parent) = paths.vault_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(&VaultPayload::default())?;
    let encrypted = encrypt_vault_bytes(&payload, password, &CryptoParams::default_for_new_vault())?;
    fs::write(&paths.vault_path, encrypted)?;
    Ok(())
}

pub fn unlock_vault(paths: &VaultPaths, password: &str) -> Result<VaultSession> {
    if !paths.vault_path.exists() {
        return Err(VaultError::NotInitialized);
    }
    let encrypted = fs::read(&paths.vault_path)?;
    let opened = open_vault_bytes(&encrypted, password)?;
    let payload = serde_json::from_slice(&opened.plaintext)?;
    Ok(VaultSession {
        paths: paths.clone(),
        payload,
        key: Some(opened.key),
    })
}

#[derive(Debug)]
pub struct VaultSession {
    paths: VaultPaths,
    payload: VaultPayload,
    key: Option<VaultMasterKey>,
}

fn find_field_case_insensitive<'a>(
    fields: &'a BTreeMap<String, String>,
    key: &str,
) -> Option<&'a String> {
    let key_lower = key.to_ascii_lowercase();
    fields.iter().find(|(k, _)| k.to_ascii_lowercase() == key_lower).map(|(_, v)| v)
}

impl VaultSession {
    pub fn is_unlocked(&self) -> bool {
        self.key.is_some()
    }

    pub fn lock(&mut self) {
        self.key = None;
    }

    pub fn list_credentials(&self) -> Vec<RedactedCredential> {
        self.payload.credentials.iter().map(Credential::redacted).collect()
    }

    pub fn search_credentials(&self, query: &str) -> Vec<RedactedCredential> {
        self.payload
            .credentials
            .iter()
            .filter(|credential| credential.matches_query(query))
            .map(Credential::redacted)
            .collect()
    }

    pub fn get_credential(&self, id: &str) -> Result<RedactedCredential> {
        self.payload
            .credentials
            .iter()
            .find(|credential| credential.id == id || credential.name.eq_ignore_ascii_case(id))
            .map(Credential::redacted)
            .ok_or(VaultError::CredentialNotFound)
    }

    pub fn add_credential(&mut self, draft: CredentialDraft) -> Result<String> {
        self.require_unlocked()?;
        let credential = draft.into_credential();
        let id = credential.id.clone();
        self.payload.audit_events.push(AuditEvent {
            event: "credential_created".to_string(),
            credential_id: Some(id.clone()),
            field: None,
            at: Utc::now(),
        });
        self.payload.credentials.push(credential);
        Ok(id)
    }

    pub fn update_credential(&mut self, id: &str, draft: CredentialDraft) -> Result<()> {
        self.require_unlocked()?;
        let credential = self
            .payload
            .credentials
            .iter_mut()
            .find(|credential| credential.id == id || credential.name.eq_ignore_ascii_case(id))
            .ok_or(VaultError::CredentialNotFound)?;
        credential.name = draft.name;
        credential.r#type = draft.r#type;
        credential.service = draft.service;
        credential.key = draft.key;
        credential.fields = draft.fields;
        credential.metadata = draft.metadata;
        credential.tags = draft.tags;
        credential.updated_at = Utc::now();
        self.payload.audit_events.push(AuditEvent {
            event: "credential_updated".to_string(),
            credential_id: Some(credential.id.clone()),
            field: None,
            at: Utc::now(),
        });
        Ok(())
    }

    pub fn delete_credential(&mut self, id: &str) -> Result<()> {
        self.require_unlocked()?;
        let len = self.payload.credentials.len();
        self.payload
            .credentials
            .retain(|credential| credential.id != id && !credential.name.eq_ignore_ascii_case(id));
        if self.payload.credentials.len() == len {
            return Err(VaultError::CredentialNotFound);
        }
        self.payload.audit_events.push(AuditEvent {
            event: "credential_deleted".to_string(),
            credential_id: Some(id.to_string()),
            field: None,
            at: Utc::now(),
        });
        Ok(())
    }

    pub fn reveal_secret(&mut self, id: &str, field: &str) -> Result<String> {
        self.require_unlocked()?;
        let credential = self
            .payload
            .credentials
            .iter()
            .find(|credential| credential.id == id || credential.name.eq_ignore_ascii_case(id))
            .ok_or(VaultError::CredentialNotFound)?;
        let value = find_field_case_insensitive(&credential.fields, field)
            .ok_or(VaultError::FieldNotFound)?
            .clone();
        self.payload.audit_events.push(AuditEvent {
            event: "secret_revealed".to_string(),
            credential_id: Some(credential.id.clone()),
            field: Some(field.to_string()),
            at: Utc::now(),
        });
        Ok(value)
    }

    pub fn copy_secret_event(&mut self, id: &str, field: &str) -> Result<()> {
        self.require_unlocked()?;
        if !self
            .payload
            .credentials
            .iter()
            .any(|credential| credential.id == id || credential.name.eq_ignore_ascii_case(id))
        {
            return Err(VaultError::CredentialNotFound);
        }
        self.payload.audit_events.push(AuditEvent {
            event: "secret_copied".to_string(),
            credential_id: Some(id.to_string()),
            field: Some(field.to_string()),
            at: Utc::now(),
        });
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let key = self.require_unlocked()?;
        let plaintext = serde_json::to_vec_pretty(&self.payload)?;
        let encrypted = seal_opened_vault_bytes(&plaintext, key)?;
        fs::write(&self.paths.vault_path, encrypted)?;
        Ok(())
    }

    fn require_unlocked(&self) -> Result<&VaultMasterKey> {
        self.key.as_ref().ok_or(VaultError::Locked)
    }
}
