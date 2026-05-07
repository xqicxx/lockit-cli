//! Backend configuration — Google Drive only.

use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Sync backend config.  Only Google Drive is supported.
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleDriveConfig {
    /// OAuth client ID (from Google Cloud Console).
    pub client_id: String,
    /// OAuth client secret (from Google Cloud Console).
    pub client_secret: String,
}

/// Google OAuth tokens stored separately from config.
#[derive(Debug, Clone)]
pub struct GoogleTokenStore {
    /// Access token (may expire).
    pub access_token: Secret<String>,
    /// Refresh token (long-lived).
    pub refresh_token: Secret<String>,
    /// Access token expiry (Unix timestamp).
    pub expires_at: Option<u64>,
}

impl Serialize for GoogleTokenStore {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = ser.serialize_struct("GoogleTokenStore", 3)?;
        s.serialize_field("access_token", self.access_token.expose_secret())?;
        s.serialize_field("refresh_token", self.refresh_token.expose_secret())?;
        s.serialize_field("expires_at", &self.expires_at)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for GoogleTokenStore {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            access_token: String,
            refresh_token: String,
            expires_at: Option<u64>,
        }
        let raw = Raw::deserialize(de)?;
        Ok(GoogleTokenStore {
            access_token: Secret::new(raw.access_token),
            refresh_token: Secret::new(raw.refresh_token),
            expires_at: raw.expires_at,
        })
    }
}

/// Load tokens from a TOML file.
pub fn load_tokens(path: &std::path::Path) -> Result<Option<GoogleTokenStore>, std::io::Error> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let tokens: GoogleTokenStore = toml::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(Some(tokens))
}

/// Save tokens to a TOML file.
pub fn save_tokens(
    path: &std::path::Path,
    tokens: &GoogleTokenStore,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(tokens)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(path, content)?;
    // Restrict permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}
