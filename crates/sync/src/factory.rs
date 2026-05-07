//! `SyncBackendFactory` — construct a Google Drive backend from tokens.

use crate::backend::SyncBackend;
use crate::backends::google_drive::GoogleDriveBackend;
use crate::config::GoogleTokenStore;
use crate::error::Result;

pub struct SyncBackendFactory;

impl SyncBackendFactory {
    /// Build a Google Drive backend from stored tokens.
    pub fn from_token_store(tokens: GoogleTokenStore) -> Result<Box<dyn SyncBackend>> {
        Ok(Box::new(GoogleDriveBackend::new(tokens)?))
    }
}
