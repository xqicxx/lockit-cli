use anyhow::Context;
use lockit_core::sync::google_drive::GoogleDriveConfig;
use lockit_core::sync::{sha256_checksum, SyncCrypto, SyncManifest};
use lockit_core::vault::VaultPaths;
use zeroize::Zeroize;

pub struct PreparedUpload {
    pub upload_bytes: Vec<u8>,
    pub local_checksum: String,
    pub manifest: SyncManifest,
}

pub struct PulledVault {
    pub local_bytes: Vec<u8>,
    pub local_checksum: String,
    pub cloud_checksum: String,
}

pub fn prepare_upload(
    paths: &VaultPaths,
    pw: Option<String>,
    sync_config: Option<&GoogleDriveConfig>,
    updated_by: &str,
) -> anyhow::Result<PreparedUpload> {
    let password = crate::utils::read_password(pw, "Master password")?;
    let session =
        lockit_core::vault::unlock_vault(paths, &password).context("Failed to unlock vault")?;

    let (upload_bytes, local_checksum, schema_version) =
        if let Some(key_b64) = sync_config.and_then(|c| c.sync_key.as_ref()) {
            let sync_key = SyncCrypto::decode_key(key_b64)
                .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;
            let mut payload_json = serde_json::to_vec(&session.payload)
                .context("Failed to serialize vault payload")?;
            let local_checksum = sha256_checksum(&payload_json);
            let encrypted = SyncCrypto::encrypt(&payload_json, &sync_key)
                .map_err(|e| anyhow::anyhow!("Sync encryption failed: {e}"))?;
            payload_json.zeroize();
            (encrypted, local_checksum, 2)
        } else {
            let bytes = std::fs::read(&paths.vault_path).context("Failed to read vault file")?;
            let local_checksum = sha256_checksum(&bytes);
            (bytes, local_checksum, 1)
        };

    let cloud_checksum = sha256_checksum(&upload_bytes);
    let manifest = SyncManifest::new(
        cloud_checksum,
        updated_by,
        upload_bytes.len() as u64,
        schema_version,
    );

    Ok(PreparedUpload {
        upload_bytes,
        local_checksum,
        manifest,
    })
}

pub fn materialize_download(
    pw: Option<String>,
    sync_config: Option<&GoogleDriveConfig>,
    manifest: &SyncManifest,
    cloud_bytes: Vec<u8>,
) -> anyhow::Result<PulledVault> {
    let downloaded_checksum = sha256_checksum(&cloud_bytes);
    if downloaded_checksum != manifest.vault_checksum {
        anyhow::bail!(
            "Checksum mismatch: downloaded data does not match manifest. Expected {}, got {}",
            manifest.vault_checksum,
            downloaded_checksum
        );
    }

    let (local_bytes, local_checksum) = match manifest.schema_version {
        v if v >= 2 => {
            let key_b64 = sync_config
                .and_then(|c| c.sync_key.as_ref())
                .ok_or_else(|| anyhow::anyhow!("Cloud vault is encrypted with a sync key, but none is configured locally. Use 'lockit sync key-set <KEY>' first."))?;
            let sync_key = SyncCrypto::decode_key(key_b64)
                .map_err(|e| anyhow::anyhow!("Invalid sync key: {e}"))?;
            let mut payload_json = SyncCrypto::decrypt(&cloud_bytes, &sync_key)
                .map_err(|e| anyhow::anyhow!("Sync decryption failed: {e}"))?;
            let local_checksum = sha256_checksum(&payload_json);
            let mut password = crate::utils::read_password(pw, "Master password")?;
            let params = lockit_core::crypto::CryptoParams::default_for_new_vault();
            let encrypted =
                lockit_core::crypto::encrypt_vault_bytes(&payload_json, &password, &params)
                    .context("Failed to re-encrypt vault")?;
            payload_json.zeroize();
            password.zeroize();
            (encrypted, local_checksum)
        }
        _ => {
            let local_checksum = sha256_checksum(&cloud_bytes);
            (cloud_bytes, local_checksum)
        }
    };

    Ok(PulledVault {
        local_bytes,
        local_checksum,
        cloud_checksum: manifest.vault_checksum.clone(),
    })
}
