use super::{
    SyncBackend, SyncError, SyncManifest,
    GOOGLE_DRIVE_BACKUP_PREFIX, GOOGLE_DRIVE_MANIFEST_FILE, GOOGLE_DRIVE_SYNC_FOLDER,
    GOOGLE_DRIVE_VAULT_FILE,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// Configuration for authenticating with Google Drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    pub access_token: String,
    pub refresh_token: String,
    pub token_expiry: i64,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub sync_key: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub migrated_from_appdata: bool,
}

/// A [`SyncBackend`] implementation that stores the encrypted vault and
/// manifest in a visible `lockit-sync/` folder in the user's Google Drive.
pub struct GoogleDriveBackend {
    config: RefCell<Option<GoogleDriveConfig>>,
}

impl Default for GoogleDriveBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleDriveBackend {
    pub fn new() -> Self {
        Self {
            config: RefCell::new(None),
        }
    }

    pub fn configure(&self, config: GoogleDriveConfig) {
        *self.config.borrow_mut() = Some(config);
    }

    pub fn get_config(&self) -> Option<GoogleDriveConfig> {
        self.config.borrow().clone()
    }

    fn access_token(&self) -> Result<String, SyncError> {
        let mut cfg_ref = self.config.borrow_mut();
        let cfg = cfg_ref.as_mut().ok_or(SyncError::NotConfigured)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if now < cfg.token_expiry - 60 {
            return Ok(cfg.access_token.clone());
        }
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", cfg.client_id.as_str()),
                ("client_secret", cfg.client_secret.as_str()),
                ("refresh_token", cfg.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let new_token = json["access_token"]
            .as_str()
            .ok_or_else(|| SyncError::HttpError("no access_token in refresh response".into()))?;
        cfg.access_token = new_token.to_string();
        cfg.token_expiry = now + json["expires_in"].as_i64().unwrap_or(3600);
        Ok(new_token.to_string())
    }

    fn find_or_create_folder(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
    ) -> Result<String, SyncError> {
        // 1. Try cached folder_id
        if let Some(cached) = self
            .config
            .borrow()
            .as_ref()
            .and_then(|c| c.folder_id.clone())
        {
            let url = format!(
                "https://www.googleapis.com/drive/v3/files/{}?fields=id,trashed",
                cached
            );
            if let Ok(resp) = client.get(&url).bearer_auth(token).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if json["id"].as_str().is_some() && json["trashed"].as_bool() != Some(true) {
                        return Ok(cached);
                    }
                }
            }
        }

        // 2. Search for existing folder
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q=name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false&spaces=drive&fields=files(id)",
            GOOGLE_DRIVE_SYNC_FOLDER
        );
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;

        if let Some(id) = json["files"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|f| f["id"].as_str())
        {
            let id = id.to_string();
            self.config
                .borrow_mut()
                .as_mut()
                .map(|c| c.folder_id = Some(id.clone()));
            return Ok(id);
        }

        // 3. Create the folder
        let metadata = serde_json::json!({
            "name": GOOGLE_DRIVE_SYNC_FOLDER,
            "mimeType": "application/vnd.google-apps.folder"
        });
        let resp = client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .json(&metadata)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let id = json["id"]
            .as_str()
            .ok_or_else(|| SyncError::HttpError("failed to create folder".into()))?
            .to_string();
        self.config
            .borrow_mut()
            .as_mut()
            .map(|c| c.folder_id = Some(id.clone()));
        Ok(id)
    }

    fn find_file(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
        name: &str,
        folder_id: &str,
    ) -> Result<Option<String>, SyncError> {
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q=name='{}' and '{}' in parents and trashed=false&spaces=drive",
            name, folder_id
        );
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(json["files"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|f| f["id"].as_str().map(|s| s.to_string())))
    }

    fn find_or_create_file(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
        name: &str,
        folder_id: &str,
    ) -> Result<String, SyncError> {
        if let Some(id) = self.find_file(client, token, name, folder_id)? {
            return Ok(id);
        }
        let metadata = serde_json::json!({
            "name": name,
            "parents": [folder_id]
        });
        let resp = client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .json(&metadata)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        json["id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SyncError::HttpError("failed to create file".into()))
    }

    fn upload_file_content(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
        file_id: &str,
        data: &[u8],
        mime: &str,
    ) -> Result<(), SyncError> {
        let url = format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
            file_id
        );
        client
            .patch(&url)
            .bearer_auth(token)
            .header("Content-Type", mime)
            .body(data.to_vec())
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(())
    }

    /// Upload a timestamped backup file for Android restore visibility.
    pub fn upload_backup(&self, data: &[u8], timestamp: &str) -> Result<(), SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let folder_id = self.find_or_create_folder(&client, &token)?;

        let backup_name = format!("{}{}.enc", GOOGLE_DRIVE_BACKUP_PREFIX, timestamp);
        let metadata = serde_json::json!({
            "name": backup_name,
            "parents": [folder_id]
        });
        let resp = client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(&token)
            .json(&metadata)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let file_id = json["id"]
            .as_str()
            .ok_or_else(|| SyncError::HttpError("failed to create backup file".into()))?;

        self.upload_file_content(&client, &token, file_id, data, "application/octet-stream")
    }

    /// Migrate data from legacy appDataFolder to the visible lockit-sync/ folder.
    /// Returns true if migration happened.
    pub fn migrate_from_appdata(&self) -> Result<bool, SyncError> {
        {
            let cfg = self.config.borrow();
            if cfg.as_ref().map_or(true, |c| c.migrated_from_appdata) {
                return Ok(false);
            }
        }

        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let folder_id = self.find_or_create_folder(&client, &token)?;

        // Find files in legacy appDataFolder
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q=(name='{}' or name='{}' or name contains '{}') and trashed=false&spaces=appDataFolder&fields=files(id,name)",
            GOOGLE_DRIVE_VAULT_FILE, GOOGLE_DRIVE_MANIFEST_FILE, GOOGLE_DRIVE_BACKUP_PREFIX
        );
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;

        let files = json["files"].as_array();
        if files.is_none() || files.unwrap().is_empty() {
            self.config
                .borrow_mut()
                .as_mut()
                .map(|c| c.migrated_from_appdata = true);
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
            if self.find_file(&client, &token, name, &folder_id)?.is_some() {
                continue;
            }

            // Download from appDataFolder
            let dl_url = format!(
                "https://www.googleapis.com/drive/v3/files/{}?alt=media",
                file_id
            );
            let resp = match client.get(&dl_url).bearer_auth(&token).send() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let bytes = match resp.bytes() {
                Ok(b) => b.to_vec(),
                Err(_) => continue,
            };

            // Upload to visible folder
            let meta = serde_json::json!({
                "name": name,
                "parents": [folder_id]
            });
            if client
                .post("https://www.googleapis.com/drive/v3/files")
                .bearer_auth(&token)
                .json(&meta)
                .body(bytes)
                .header("Content-Type", "application/octet-stream")
                .send()
                .is_ok()
            {
                // Delete from appDataFolder
                let del_url =
                    format!("https://www.googleapis.com/drive/v3/files/{}", file_id);
                let _ = client.delete(&del_url).bearer_auth(&token).send();
                migrated = true;
            }
        }

        self.config
            .borrow_mut()
            .as_mut()
            .map(|c| c.migrated_from_appdata = true);
        Ok(migrated)
    }
}

impl SyncBackend for GoogleDriveBackend {
    fn name(&self) -> &str {
        "google_drive"
    }

    fn is_configured(&self) -> bool {
        self.config.borrow().is_some()
    }

    fn upload_vault(
        &self,
        encrypted_data: &[u8],
        manifest: &SyncManifest,
    ) -> Result<(), SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let folder_id = self.find_or_create_folder(&client, &token)?;

        let vault_id =
            self.find_or_create_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE, &folder_id)?;
        self.upload_file_content(
            &client,
            &token,
            &vault_id,
            encrypted_data,
            "application/octet-stream",
        )?;

        let manifest_json =
            serde_json::to_vec(manifest).map_err(|e| SyncError::HttpError(e.to_string()))?;
        let manifest_id =
            self.find_or_create_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE, &folder_id)?;
        self.upload_file_content(
            &client,
            &token,
            &manifest_id,
            &manifest_json,
            "application/json",
        )?;

        // Best-effort migration from legacy appDataFolder
        let _ = self.migrate_from_appdata();

        Ok(())
    }

    fn download_vault(&self) -> Result<Vec<u8>, SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let folder_id = self.find_or_create_folder(&client, &token)?;
        let file_id = self
            .find_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE, &folder_id)?
            .ok_or(SyncError::NotConfigured)?;
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            file_id
        );
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(resp
            .bytes()
            .map_err(|e| SyncError::HttpError(e.to_string()))?
            .to_vec())
    }

    fn get_manifest(&self) -> Result<Option<SyncManifest>, SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let folder_id = self.find_or_create_folder(&client, &token)?;
        let file_id = match self
            .find_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE, &folder_id)?
        {
            Some(id) => id,
            None => return Ok(None),
        };
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            file_id
        );
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let manifest: SyncManifest = resp
            .json()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(Some(manifest))
    }

    fn delete_sync_data(&self) -> Result<(), SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();

        // Delete from visible folder
        if let Ok(folder_id) = self.find_or_create_folder(&client, &token) {
            for name in &[GOOGLE_DRIVE_VAULT_FILE, GOOGLE_DRIVE_MANIFEST_FILE] {
                if let Some(id) = self.find_file(&client, &token, name, &folder_id)? {
                    let url = format!("https://www.googleapis.com/drive/v3/files/{}", id);
                    client
                        .delete(&url)
                        .bearer_auth(&token)
                        .send()
                        .map_err(|e| SyncError::HttpError(e.to_string()))?;
                }
            }
            // Also delete vault_* backup files
            let url = format!(
                "https://www.googleapis.com/drive/v3/files?q=name contains '{}' and '{}' in parents and trashed=false&spaces=drive&fields=files(id)",
                GOOGLE_DRIVE_BACKUP_PREFIX, folder_id
            );
            if let Ok(resp) = client.get(&url).bearer_auth(&token).send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(files) = json["files"].as_array() {
                        for file in files {
                            if let Some(id) = file["id"].as_str() {
                                let del_url = format!(
                                    "https://www.googleapis.com/drive/v3/files/{}",
                                    id
                                );
                                let _ = client.delete(&del_url).bearer_auth(&token).send();
                            }
                        }
                    }
                }
            }
        }

        // Best-effort cleanup of legacy appDataFolder
        let legacy_url = format!(
            "https://www.googleapis.com/drive/v3/files?q=(name='{}' or name='{}' or name contains '{}') and trashed=false&spaces=appDataFolder&fields=files(id)",
            GOOGLE_DRIVE_VAULT_FILE, GOOGLE_DRIVE_MANIFEST_FILE, GOOGLE_DRIVE_BACKUP_PREFIX
        );
        if let Ok(resp) = client.get(&legacy_url).bearer_auth(&token).send() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(files) = json["files"].as_array() {
                    for file in files {
                        if let Some(id) = file["id"].as_str() {
                            let del_url =
                                format!("https://www.googleapis.com/drive/v3/files/{}", id);
                            let _ = client.delete(&del_url).bearer_auth(&token).send();
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
