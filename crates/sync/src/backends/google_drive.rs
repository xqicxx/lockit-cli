//! Google Drive backend using the REST API.
//!
//! Stores files in a visible `lockit-sync/` folder in the user's Drive root,
//! so both CLI and Android (which use different OAuth client types) can
//! access the same data.  Legacy `appDataFolder` data is migrated on first
//! use and then deleted.

use async_trait::async_trait;
use secrecy::ExposeSecret;
use std::sync::Mutex;

use crate::backend::{SyncBackend, SyncMetadata};
use crate::config::GoogleTokenStore;
use crate::error::{Error, Result};
use crate::http::{async_client, send_async_with_retry};
use crate::util::{sha256_hex, url_encode};

const MIME_OCTET: &str = "application/octet-stream";
const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const FOLDER_NAME: &str = "lockit-sync";
const BACKUP_PREFIX: &str = "vault_";

/// Google Drive sync backend.
///
/// Reads/writes to a visible `lockit-sync/` folder in the user's Drive root
/// so that both the CLI (Web OAuth) and Android (Google Sign-In) can share
/// the same vault data.  Legacy `appDataFolder` content is migrated
/// automatically on first upload.
pub struct GoogleDriveBackend {
    client: reqwest::Client,
    token: GoogleTokenStore,
    config: GoogleDriveSyncConfig,
    folder_id: Mutex<Option<String>>,
}

/// Persistent config for the Google Drive backend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoogleDriveSyncConfig {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub migrated_from_appdata: bool,
}

impl GoogleDriveBackend {
    pub fn new(token: GoogleTokenStore, config: GoogleDriveSyncConfig) -> Result<Self> {
        let folder_id = Mutex::new(config.folder_id.clone());
        Ok(Self {
            client: async_client()?,
            token,
            config,
            folder_id,
        })
    }

    /// Return the current sync config (including cached folder_id).
    pub fn sync_config(&self) -> &GoogleDriveSyncConfig {
        &self.config
    }

    /// Return the current sync config with an up-to-date folder_id.
    pub fn sync_config_with_folder(&self) -> GoogleDriveSyncConfig {
        let mut cfg = self.config.clone();
        cfg.folder_id = self.folder_id.lock().unwrap().clone();
        cfg
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token.access_token.expose_secret())
    }

    /// Find or create the `lockit-sync/` folder in Drive root.
    async fn ensure_folder(&self) -> Result<String> {
        // 1. Try cached folder_id (extract before await)
        let cached_id = self.folder_id.lock().unwrap().clone();
        if let Some(ref cached) = cached_id {
            let url = format!("{}/files/{}?fields=id,trashed", DRIVE_API, cached);
            let request = self.client.get(&url).header("Authorization", self.bearer());
            if let Ok(resp) = send_async_with_retry(request).await
                && let Ok(json) = resp.json::<serde_json::Value>().await
                && json["id"].as_str().is_some()
                && json["trashed"].as_bool() != Some(true)
            {
                return Ok(cached.clone());
            }
        }

        // 2. Search for existing folder
        let query = format!(
            "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
            FOLDER_NAME
        );
        let url = format!(
            "{}/files?q={}&spaces=drive&fields=files(id)",
            DRIVE_API,
            url_encode(&query)
        );
        let request = self.client.get(&url).header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Upload {
                key: FOLDER_NAME.into(),
                reason: format!("Folder search failed: {e}"),
            })?;
        let json: serde_json::Value = resp.json().await.map_err(|e| Error::Upload {
            key: FOLDER_NAME.into(),
            reason: format!("Folder search parse: {e}"),
        })?;

        if let Some(id) = json["files"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|f| f["id"].as_str())
        {
            let id = id.to_string();
            *self.folder_id.lock().unwrap() = Some(id.clone());
            return Ok(id);
        }

        // 3. Create the folder
        let metadata = serde_json::json!({
            "name": FOLDER_NAME,
            "mimeType": "application/vnd.google-apps.folder"
        });
        let request = self
            .client
            .post(format!("{}/files", DRIVE_API))
            .header("Authorization", self.bearer())
            .json(&metadata);
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Upload {
                key: FOLDER_NAME.into(),
                reason: format!("Folder create failed: {e}"),
            })?;
        let json: serde_json::Value = resp.json().await.map_err(|e| Error::Upload {
            key: FOLDER_NAME.into(),
            reason: format!("Folder create parse: {e}"),
        })?;
        let id = json["id"]
            .as_str()
            .ok_or_else(|| Error::Upload {
                key: FOLDER_NAME.into(),
                reason: "No id in folder create response".into(),
            })?
            .to_string();
        *self.folder_id.lock().unwrap() = Some(id.clone());
        Ok(id)
    }

    /// Find a file by name within the lockit-sync folder.
    async fn find_file_in_folder(&self, name: &str, folder_id: &str) -> Result<Option<GoogleFile>> {
        let query = format!(
            "name='{}' and '{}' in parents and trashed=false",
            name, folder_id
        );
        let url = format!(
            "{}/files?q={}&spaces=drive&fields=files(id,name)&orderBy=modifiedTime desc",
            DRIVE_API,
            url_encode(&query)
        );
        let request = self.client.get(&url).header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Download {
                key: name.into(),
                reason: e.to_string(),
            })?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::Download {
            key: name.into(),
            reason: format!("Failed to read response: {e}"),
        })?;
        if !status.is_success() {
            return Err(Error::Download {
                key: name.into(),
                reason: format!("HTTP {status}: {body}"),
            });
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| Error::Download {
                key: name.into(),
                reason: format!("JSON parse: {e}"),
            })?;
        Ok(json["files"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|f| {
                Some(GoogleFile {
                    id: f["id"].as_str()?.to_string(),
                    name: f["name"].as_str()?.to_string(),
                })
            }))
    }

    /// Download raw bytes of a Drive file by ID.
    async fn download_file_by_id(&self, file_id: &str, key_for_error: &str) -> Result<Vec<u8>> {
        let url = format!("{}/files/{file_id}?alt=media", DRIVE_API);
        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Download {
                key: key_for_error.into(),
                reason: e.to_string(),
            })?;
        let status = resp.status();
        let body = resp.bytes().await.map_err(|e| Error::Download {
            key: key_for_error.into(),
            reason: e.to_string(),
        })?;
        if !status.is_success() {
            return Err(Error::Download {
                key: key_for_error.into(),
                reason: format!("HTTP {status}"),
            });
        }
        Ok(body.to_vec())
    }

    /// Upload or update a file in the lockit-sync folder.
    async fn upsert_file_in_folder(
        &self,
        name: &str,
        folder_id: &str,
        mime: &str,
        data: &[u8],
    ) -> Result<()> {
        if let Some(file) = self.find_file_in_folder(name, folder_id).await? {
            // Update existing file
            let url = format!("{}/files/{}?uploadType=media", DRIVE_API, file.id);
            let request = self
                .client
                .patch(&url)
                .header("Authorization", self.bearer())
                .header("Content-Type", mime)
                .body(data.to_vec());
            let resp = send_async_with_retry(request)
                .await
                .map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: e.to_string(),
                })?;
            if !resp.status().is_success() {
                return Err(Error::Upload {
                    key: name.into(),
                    reason: format!("HTTP {}", resp.status()),
                });
            }
        } else {
            // Create new file using multipart upload
            let url = format!("{}/files?uploadType=multipart", DRIVE_API);
            let boundary = "===========lockit_boundary===========";
            let mut body = Vec::new();
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
            body.extend_from_slice(
                format!(
                    r#"{{"name":"{name}","parents":["{folder_id}"],"mimeType":"{mime}"}}"#
                )
                .as_bytes(),
            );
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            let request = self
                .client
                .post(&url)
                .header("Authorization", self.bearer())
                .header(
                    "Content-Type",
                    format!("multipart/related; boundary={boundary}"),
                )
                .body(body);
            let resp = send_async_with_retry(request)
                .await
                .map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: e.to_string(),
                })?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err_body = resp.text().await.map_err(|e| Error::Upload {
                    key: name.into(),
                    reason: format!("Failed to read response: {e}"),
                })?;
                return Err(Error::Upload {
                    key: name.into(),
                    reason: format!("HTTP {status}: {err_body}"),
                });
            }
        }
        Ok(())
    }

    /// Upload a timestamped backup for Android restore visibility.
    pub async fn upload_backup(&self, data: &[u8], timestamp: &str) -> Result<()> {
        let folder_id = self.ensure_folder().await?;
        let backup_name = format!("{}{}.enc", BACKUP_PREFIX, timestamp);
        self.upsert_file_in_folder(&backup_name, &folder_id, MIME_OCTET, data)
            .await
    }

    /// Migrate data from legacy `appDataFolder` to the visible folder.
    /// Returns true if migration happened.
    pub async fn migrate_from_appdata(&self) -> Result<bool> {
        if self.config.migrated_from_appdata {
            return Ok(false);
        }

        let folder_id = self.ensure_folder().await?;

        // Find files in legacy appDataFolder
        let query = format!(
            "(name='vault.enc' or name='manifest.json' or name contains '{}') and trashed=false",
            BACKUP_PREFIX
        );
        let url = format!(
            "{}/files?q={}&spaces=appDataFolder&fields=files(id,name)",
            DRIVE_API,
            url_encode(&query)
        );
        let request = self.client.get(&url).header("Authorization", self.bearer());
        let resp = match send_async_with_retry(request).await {
            Ok(r) => r,
            Err(_) => return Ok(false),
        };
        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(_) => return Ok(false),
        };

        let files = json["files"].as_array();
        if files.is_none() || files.unwrap().is_empty() {
            return Ok(false);
        }

        let mut migrated = false;
        for file in files.unwrap() {
            let file_id = match file["id"].as_str() {
                Some(id) => id,
                None => continue,
            };
            let name = match file["name"].as_str() {
                Some(n) => n,
                None => continue,
            };

            // Skip if already exists in the visible folder
            if self
                .find_file_in_folder(name, &folder_id)
                .await?
                .is_some()
            {
                continue;
            }

            // Download from appDataFolder
            let bytes = match self.download_file_by_id(file_id, name).await {
                Ok(b) => b,
                Err(_) => continue,
            };

            // Upload to visible folder
            if self
                .upsert_file_in_folder(name, &folder_id, MIME_OCTET, &bytes)
                .await
                .is_ok()
            {
                // Delete from appDataFolder
                let del_url = format!("{}/files/{}", DRIVE_API, file_id);
                let request = self
                    .client
                    .delete(&del_url)
                    .header("Authorization", self.bearer());
                let _ = send_async_with_retry(request).await;
                migrated = true;
            }
        }

        Ok(migrated)
    }
}

#[derive(Debug)]
struct GoogleFile {
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[async_trait]
impl SyncBackend for GoogleDriveBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        let folder_id = self.ensure_folder().await?;
        self.upsert_file_in_folder(key, &folder_id, MIME_OCTET, data)
            .await
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let folder_id = self.ensure_folder().await?;
        let file = self
            .find_file_in_folder(key, &folder_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                key: key.to_string(),
            })?;
        self.download_file_by_id(&file.id, key).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let folder_id = self.ensure_folder().await?;
        let query = format!(
            "name contains '{}' and '{}' in parents and trashed=false",
            prefix, folder_id
        );
        let url = format!(
            "{}/files?q={}&spaces=drive&fields=files(name)&orderBy=name",
            DRIVE_API,
            url_encode(&query)
        );
        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::List {
                prefix: prefix.into(),
                reason: e.to_string(),
            })?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::List {
            prefix: prefix.into(),
            reason: e.to_string(),
        })?;
        if !status.is_success() {
            return Err(Error::List {
                prefix: prefix.into(),
                reason: format!("HTTP {status}"),
            });
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| Error::List {
                prefix: prefix.into(),
                reason: e.to_string(),
            })?;
        let files = json["files"].as_array().cloned().unwrap_or_default();
        let mut keys: Vec<String> = files
            .into_iter()
            .filter_map(|f| f["name"].as_str().map(String::from))
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let folder_id = self.ensure_folder().await?;
        let file = self
            .find_file_in_folder(key, &folder_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                key: key.to_string(),
            })?;
        let url = format!("{}/files/{}", DRIVE_API, file.id);
        let request = self
            .client
            .delete(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Delete {
                key: key.into(),
                reason: e.to_string(),
            })?;
        if !resp.status().is_success() {
            return Err(Error::Delete {
                key: key.into(),
                reason: format!("HTTP {}", resp.status()),
            });
        }
        Ok(())
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let folder_id = self.ensure_folder().await?;
        let file = self
            .find_file_in_folder(key, &folder_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                key: key.to_string(),
            })?;

        let url = format!(
            "{}/files/{}?fields=name,size,modifiedTime",
            DRIVE_API, file.id
        );
        let request = self
            .client
            .get(&url)
            .header("Authorization", self.bearer());
        let resp = send_async_with_retry(request)
            .await
            .map_err(|e| Error::Metadata {
                key: key.into(),
                reason: e.to_string(),
            })?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| Error::Metadata {
            key: key.into(),
            reason: e.to_string(),
        })?;
        if !status.is_success() {
            return Err(Error::Metadata {
                key: key.into(),
                reason: format!("HTTP {status}"),
            });
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| Error::Metadata {
                key: key.into(),
                reason: e.to_string(),
            })?;

        let data = self.download_file_by_id(&file.id, key).await?;
        let checksum = sha256_hex(&data);

        let last_modified = json["modifiedTime"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp() as u64)
            .unwrap_or(0);

        Ok(SyncMetadata {
            version: 2,
            last_modified,
            checksum,
            size: json["size"].as_u64().unwrap_or(0),
        })
    }

    fn backend_name(&self) -> &str {
        "google-drive"
    }
}
