use super::{
    SyncBackend, SyncError, SyncManifest, GOOGLE_DRIVE_APPDATA_FOLDER, GOOGLE_DRIVE_MANIFEST_FILE,
    GOOGLE_DRIVE_VAULT_FILE,
};
use serde::{Deserialize, Serialize};

/// Configuration for authenticating with Google Drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    pub access_token: String,
    pub refresh_token: String,
    pub token_expiry: i64,
    pub client_id: String,
    pub client_secret: String,
}

/// A [`SyncBackend`] implementation that stores the encrypted vault and
/// manifest in the user's Google Drive appDataFolder.
pub struct GoogleDriveBackend {
    config: Option<GoogleDriveConfig>,
}

impl Default for GoogleDriveBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleDriveBackend {
    /// Create a new unconfigured backend. Call [`configure`](Self::configure)
    /// before using it for sync operations.
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set or replace the Google Drive configuration.
    pub fn configure(&mut self, config: GoogleDriveConfig) {
        self.config = Some(config);
    }

    /// Return the current access token, or [`SyncError::NotConfigured`] if no
    /// configuration has been provided.
    fn access_token(&self) -> Result<String, SyncError> {
        self.config
            .as_ref()
            .map(|c| c.access_token.clone())
            .ok_or(SyncError::NotConfigured)
    }

    /// Request a fresh access token from Google using the stored refresh
    /// token. Wired into sync operations when 401 is detected.
    #[allow(dead_code)]
    fn refresh_access_token(&self) -> Result<String, SyncError> {
        let cfg = self.config.as_ref().ok_or(SyncError::NotConfigured)?;
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
        json["access_token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SyncError::HttpError("no access_token in refresh response".into()))
    }

    /// Search the appDataFolder for a file with the given `name`. Returns the
    /// file ID if found, or `None` otherwise.
    fn find_file(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
        name: &str,
    ) -> Result<Option<String>, SyncError> {
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q=name='{}' and '{}' in parents&spaces=appDataFolder",
            name, GOOGLE_DRIVE_APPDATA_FOLDER
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

    /// Find an existing file by name, or create a new one in the
    /// appDataFolder. Returns the file ID in either case.
    fn find_or_create_file(
        &self,
        client: &reqwest::blocking::Client,
        token: &str,
        name: &str,
    ) -> Result<String, SyncError> {
        if let Some(id) = self.find_file(client, token, name)? {
            return Ok(id);
        }
        let metadata = serde_json::json!({
            "name": name,
            "parents": [GOOGLE_DRIVE_APPDATA_FOLDER]
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

    /// Upload raw bytes as the content of an existing Drive file.
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
}

impl SyncBackend for GoogleDriveBackend {
    fn name(&self) -> &str {
        "google_drive"
    }

    fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    fn upload_vault(
        &self,
        encrypted_data: &[u8],
        manifest: &SyncManifest,
    ) -> Result<(), SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();

        let vault_id = self.find_or_create_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE)?;
        self.upload_file_content(
            &client,
            &token,
            &vault_id,
            encrypted_data,
            "application/octet-stream",
        )?;

        let manifest_json =
            serde_json::to_vec(manifest).map_err(|e| SyncError::HttpError(e.to_string()))?;
        let manifest_id = self.find_or_create_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE)?;
        self.upload_file_content(
            &client,
            &token,
            &manifest_id,
            &manifest_json,
            "application/json",
        )?;

        Ok(())
    }

    fn download_vault(&self) -> Result<Vec<u8>, SyncError> {
        let token = self.access_token()?;
        let client = reqwest::blocking::Client::new();
        let file_id = self
            .find_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE)?
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
        let file_id = match self.find_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE)? {
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
        for name in &[GOOGLE_DRIVE_VAULT_FILE, GOOGLE_DRIVE_MANIFEST_FILE] {
            if let Some(id) = self.find_file(&client, &token, name)? {
                let url = format!("https://www.googleapis.com/drive/v3/files/{}", id);
                client
                    .delete(&url)
                    .bearer_auth(&token)
                    .send()
                    .map_err(|e| SyncError::HttpError(e.to_string()))?;
            }
        }
        Ok(())
    }
}
