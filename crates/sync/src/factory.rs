//! `SyncBackendFactory` — construct a Google Drive backend from tokens.

use crate::backends::google_drive::{GoogleDriveBackend, GoogleDriveSyncConfig};
use crate::backend::SyncBackend;
use crate::config::GoogleTokenStore;
use crate::error::Result;

pub struct SyncBackendFactory;

impl SyncBackendFactory {
    /// Build a Google Drive backend from stored tokens and config.
    pub fn from_token_store(tokens: GoogleTokenStore, config: GoogleDriveSyncConfig) -> Result<Box<dyn SyncBackend>> {
        Ok(Box::new(GoogleDriveBackend::new(tokens, config)?))
    }
}
