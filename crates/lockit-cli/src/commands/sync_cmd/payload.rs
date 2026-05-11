use anyhow::Context;
use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::{sha256_checksum, SyncCrypto, SyncManifest};
use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::utils::vault_key;

/// Unlock vault → serialize VaultPayload JSON → optionally encrypt with sync key → upload.
///
/// Returns `(upload_bytes, vault_file_checksum, manifest)`.
/// `vault_file_checksum` is SHA-256 of the raw vault file on disk (for status tracking).
pub fn prepare_upload(
    paths: &VaultPaths,
    sync_config: Option<&GoogleDriveConfig>,
) -> anyhow::Result<(Vec<u8>, String, SyncManifest)> {
    let vault_file_checksum = if paths.vault_path.exists() {
        let raw = std::fs::read(&paths.vault_path).context("Failed to read vault file")?;
        sha256_checksum(&raw)
    } else {
        String::new()
    };

    let session = unlock_vault(paths, &vault_key()).context("Failed to unlock vault")?;
    let payload_json =
        serde_json::to_vec(&session.payload).context("Failed to serialize vault payload")?;

    let upload_bytes = maybe_encrypt(&payload_json, sync_config)?;
    let cloud_checksum = sha256_checksum(&upload_bytes);
    let manifest = SyncManifest::new(cloud_checksum, "lockit-cli", upload_bytes.len() as u64, 2);

    Ok((upload_bytes, vault_file_checksum, manifest))
}

/// Download → optionally decrypt → verify checksum → write to vault.
///
/// Returns `(vault_file_checksum, cloud_checksum)` where `vault_file_checksum`
/// is SHA-256 of the raw vault file on disk after import (for status tracking).
///
/// Handles two formats:
/// - VaultPayload JSON (from Android or current CLI) → import into local vault
/// - Raw encrypted vault bytes (from old CLI) → write directly
pub fn materialize_download(
    paths: &VaultPaths,
    sync_config: Option<&GoogleDriveConfig>,
    manifest: &SyncManifest,
    cloud_bytes: Vec<u8>,
) -> anyhow::Result<(String, String)> {
    if sha256_checksum(&cloud_bytes) != manifest.vault_checksum {
        anyhow::bail!(
            "Checksum mismatch: expected {}, got {}",
            manifest.vault_checksum,
            sha256_checksum(&cloud_bytes)
        );
    }

    let plain_bytes = maybe_decrypt(&cloud_bytes, sync_config, manifest.schema_version)?;

    if is_json(&plain_bytes) {
        import_payload_json(paths, &plain_bytes)?;
    } else {
        // Legacy: raw vault bytes, write directly
        std::fs::write(&paths.vault_path, &plain_bytes).context("Failed to write vault file")?;
    }

    let vault_file_checksum = if paths.vault_path.exists() {
        let raw = std::fs::read(&paths.vault_path).context("Failed to read vault file after import")?;
        sha256_checksum(&raw)
    } else {
        String::new()
    };

    Ok((vault_file_checksum, manifest.vault_checksum.clone()))
}

fn is_json(data: &[u8]) -> bool {
    data.first().is_some_and(|&b| b == b'{')
}

/// Parse VaultPayload JSON and import into local vault.
/// Returns `(vault_file_checksum, json_checksum)`.
fn import_payload_json(
    paths: &VaultPaths,
    json_bytes: &[u8],
) -> anyhow::Result<(String, String)> {
    // Parse as raw JSON first, normalize PascalCase types from Android to snake_case
    let mut raw: serde_json::Value =
        serde_json::from_slice(json_bytes).context("Failed to parse VaultPayload JSON")?;
    if let Some(creds) = raw.get_mut("credentials").and_then(|c| c.as_array_mut()) {
        for cred in creds {
            if let Some(t) = cred.get("type").and_then(|v| v.as_str()) {
                let ct = lockit_core::credential::CredentialType::from_flexible(t);
                cred["type"] = serde_json::json!(ct);
            }
        }
    }

    let payload: lockit_core::vault::VaultPayload =
        serde_json::from_value(raw).context("Failed to parse VaultPayload JSON")?;

    let mut session = unlock_vault(paths, &vault_key()).context("Failed to unlock vault")?;

    // Delete existing credentials, replace with payload
    let existing = session.list_credentials();
    for cred in &existing {
        session.delete_credential(&cred.id)?;
    }

    // Add each credential from the payload
    for cred in &payload.credentials {
        let draft = lockit_core::credential::CredentialDraft::new(
            &cred.name,
            cred.r#type.clone(),
            &cred.service,
            &cred.key,
            serde_json::to_value(&cred.fields)?,
        )
        .with_metadata(cred.metadata.clone());
        session.add_credential(draft)?;
    }

    session.save().context("Failed to save vault")?;

    let vault_file_checksum = if paths.vault_path.exists() {
        let raw = std::fs::read(&paths.vault_path).context("Failed to read vault file after import")?;
        sha256_checksum(&raw)
    } else {
        String::new()
    };
    let json_checksum = sha256_checksum(json_bytes);
    Ok((vault_file_checksum, json_checksum))
}

fn maybe_encrypt(
    data: &[u8],
    sync_config: Option<&GoogleDriveConfig>,
) -> anyhow::Result<Vec<u8>> {
    let Some(key_b64) = sync_config.and_then(|c| c.sync_key.as_ref()) else {
        return Ok(data.to_vec());
    };
    let sync_key = SyncCrypto::decode_key(key_b64)
        .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;
    SyncCrypto::encrypt(data, &sync_key)
        .map_err(|e| anyhow::anyhow!("Sync encryption failed: {e}"))
}

fn maybe_decrypt(
    data: &[u8],
    sync_config: Option<&GoogleDriveConfig>,
    schema_version: u32,
) -> anyhow::Result<Vec<u8>> {
    if schema_version < 2 {
        return Ok(data.to_vec());
    }
    let Some(key_b64) = sync_config.and_then(|c| c.sync_key.as_ref()) else {
        return Ok(data.to_vec());
    };
    let sync_key = SyncCrypto::decode_key(key_b64)
        .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;
    SyncCrypto::decrypt(data, &sync_key)
        .map_err(|e| anyhow::anyhow!("Sync decryption failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lockit_core::credential::{CredentialDraft, CredentialType};
    use lockit_core::vault::{init_vault, unlock_vault};
    use tempfile::tempdir;

    fn test_paths() -> (tempfile::TempDir, VaultPaths) {
        let dir = tempdir().unwrap();
        let paths = VaultPaths::new(dir.path().join("vault.enc"));
        init_vault(&paths, &vault_key()).unwrap();
        let mut session = unlock_vault(&paths, &vault_key()).unwrap();
        session
            .add_credential(CredentialDraft::new(
                "OPENAI",
                CredentialType::ApiKey,
                "openai",
                "default",
                serde_json::json!({ "secret_value": "sk-test" }),
            ))
            .unwrap();
        session.save().unwrap();
        (dir, paths)
    }

    #[test]
    fn upload_produces_vault_payload_json() {
        let (_dir, paths) = test_paths();
        let (upload_bytes, _, _) = prepare_upload(&paths, None).unwrap();
        // Should be valid JSON starting with {
        assert!(upload_bytes.starts_with(b"{"));
        let payload: lockit_core::vault::VaultPayload =
            serde_json::from_slice(&upload_bytes).unwrap();
        assert_eq!(payload.schema_version, 2);
        assert_eq!(payload.credentials.len(), 1);
        assert_eq!(payload.credentials[0].name, "OPENAI");
    }

    #[test]
    fn download_imports_json_payload_into_vault() {
        let (_dir, paths) = test_paths();

        // Create a VaultPayload JSON to download
        let json = br#"{"schema_version":2,"credentials":[{"id":"test-id","name":"DEPLOY","type":"token","service":"deploy","key":"ci","fields":{"token_value":"tok-123"},"metadata":{},"tags":[],"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}],"audit_events":[]}"#;
        let manifest = SyncManifest::new(
            sha256_checksum(json),
            "android",
            json.len() as u64,
            2,
        );

        let (_, _) =
            materialize_download(&paths, None, &manifest, json.to_vec()).unwrap();

        // Verify credential was imported
        let session = unlock_vault(&paths, &vault_key()).unwrap();
        let creds = session.list_credentials();
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].name, "DEPLOY");
    }
}
