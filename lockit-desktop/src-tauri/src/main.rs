use lockit_core::credential::{CredentialDraft, CredentialType, RedactedCredential};
use lockit_core::sync::{compute_sync_status, sha256_checksum, SyncInputs, SyncStatus};
use lockit_core::vault::{init_vault as core_init_vault, unlock_vault as core_unlock_vault, VaultPaths, VaultSession};
use serde::Deserialize;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    paths: VaultPaths,
    session: Mutex<Option<VaultSession>>,
    last_sync_checksum: Mutex<Option<String>>,
}

#[derive(Debug, thiserror::Error)]
enum CommandError {
    #[error("vault is locked")]
    Locked,
    #[error("{0}")]
    Message(String),
}

impl serde::Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct CredentialInput {
    name: String,
    r#type: String,
    service: String,
    key: String,
    value: String,
}

#[tauri::command]
fn init_vault(password: String, state: State<AppState>) -> Result<(), CommandError> {
    core_init_vault(&state.paths, &password).map_err(err)
}

#[tauri::command]
fn unlock_vault(password: String, state: State<AppState>) -> Result<(), CommandError> {
    let session = core_unlock_vault(&state.paths, &password).map_err(err)?;
    *state.session.lock().map_err(lock_err)? = Some(session);
    Ok(())
}

#[tauri::command]
fn lock_vault(state: State<AppState>) -> Result<(), CommandError> {
    if let Some(session) = state.session.lock().map_err(lock_err)?.as_mut() {
        session.lock();
    }
    *state.session.lock().map_err(lock_err)? = None;
    Ok(())
}

#[tauri::command]
fn list_credentials(state: State<AppState>) -> Result<Vec<RedactedCredential>, CommandError> {
    let guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_ref().ok_or(CommandError::Locked)?;
    Ok(session.list_credentials())
}

#[tauri::command]
fn search_credentials(query: String, state: State<AppState>) -> Result<Vec<RedactedCredential>, CommandError> {
    let guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_ref().ok_or(CommandError::Locked)?;
    Ok(session.search_credentials(&query))
}

#[tauri::command]
fn add_credential(input: CredentialInput, state: State<AppState>) -> Result<String, CommandError> {
    let mut guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_mut().ok_or(CommandError::Locked)?;
    let draft = input.into_draft()?;
    let id = session.add_credential(draft).map_err(err)?;
    session.save().map_err(err)?;
    Ok(id)
}

#[tauri::command]
fn update_credential(id: String, input: CredentialInput, state: State<AppState>) -> Result<(), CommandError> {
    let mut guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_mut().ok_or(CommandError::Locked)?;
    session.update_credential(&id, input.into_draft()?).map_err(err)?;
    session.save().map_err(err)
}

#[tauri::command]
fn delete_credential(id: String, state: State<AppState>) -> Result<(), CommandError> {
    let mut guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_mut().ok_or(CommandError::Locked)?;
    session.delete_credential(&id).map_err(err)?;
    session.save().map_err(err)
}

#[tauri::command]
fn reveal_secret(id: String, field: String, state: State<AppState>) -> Result<String, CommandError> {
    let mut guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_mut().ok_or(CommandError::Locked)?;
    let value = session.reveal_secret(&id, &field).map_err(err)?;
    session.save().map_err(err)?;
    Ok(value)
}

#[tauri::command]
fn copy_secret_event(id: String, field: String, state: State<AppState>) -> Result<(), CommandError> {
    let mut guard = state.session.lock().map_err(lock_err)?;
    let session = guard.as_mut().ok_or(CommandError::Locked)?;
    session.copy_secret_event(&id, &field).map_err(err)?;
    session.save().map_err(err)
}

#[tauri::command]
fn sync_status(state: State<AppState>) -> Result<String, CommandError> {
    let encrypted = std::fs::read(&state.paths.vault_path).map_err(err)?;
    let local_checksum = sha256_checksum(&encrypted);
    let status = compute_sync_status(SyncInputs {
        local_checksum,
        cloud_manifest: None,
        last_sync_checksum: state.last_sync_checksum.lock().map_err(lock_err)?.clone(),
        sync_key_configured: false,
        backend_configured: false,
    });
    Ok(format_status(status).to_string())
}

#[tauri::command]
fn sync_push() -> Result<(), CommandError> {
    Err(CommandError::Message(
        "Google Drive appDataFolder backend is scaffolded for v1 but OAuth is not configured yet".to_string(),
    ))
}

#[tauri::command]
fn sync_pull() -> Result<(), CommandError> {
    Err(CommandError::Message(
        "Google Drive appDataFolder backend is scaffolded for v1 but OAuth is not configured yet".to_string(),
    ))
}

#[tauri::command]
fn sync_resolve_conflict(_resolution: String) -> Result<(), CommandError> {
    Err(CommandError::Message("No active sync conflict".to_string()))
}

impl CredentialInput {
    fn into_draft(self) -> Result<CredentialDraft, CommandError> {
        let cred_type = self.r#type.parse::<CredentialType>().map_err(CommandError::Message)?;
        Ok(CredentialDraft::new(
            self.name,
            cred_type,
            self.service,
            self.key,
            serde_json::json!({ "value": self.value }),
        ))
    }
}

fn format_status(status: SyncStatus) -> &'static str {
    match status {
        SyncStatus::NotConfigured => "NOT_CONFIGURED",
        SyncStatus::BackendError => "BACKEND_ERROR",
        SyncStatus::NeverSynced => "NEVER_SYNCED",
        SyncStatus::UpToDate => "UP_TO_DATE",
        SyncStatus::LocalAhead => "LOCAL_AHEAD",
        SyncStatus::CloudAhead => "CLOUD_AHEAD",
        SyncStatus::Conflict => "CONFLICT",
    }
}

fn err(error: impl std::error::Error) -> CommandError {
    CommandError::Message(error.to_string())
}

fn lock_err<T>(error: std::sync::PoisonError<T>) -> CommandError {
    CommandError::Message(error.to_string())
}

fn main() {
    let paths = VaultPaths::platform_default().expect("resolve Lockit data directory");
    tauri::Builder::default()
        .manage(AppState { paths, session: Mutex::new(None), last_sync_checksum: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            init_vault,
            unlock_vault,
            lock_vault,
            list_credentials,
            search_credentials,
            add_credential,
            update_credential,
            delete_credential,
            reveal_secret,
            copy_secret_event,
            sync_status,
            sync_push,
            sync_pull,
            sync_resolve_conflict,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lockit desktop");
}
