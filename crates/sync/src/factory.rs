//! `SyncBackendFactory` — construct a backend from a [`BackendConfig`].

use crate::backend::SyncBackend;
use crate::backends::git::GitBackend;
use crate::backends::local::LocalBackend;
use crate::backends::s3::S3Backend;
use crate::backends::webdav::WebDavBackend;
use crate::config::BackendConfig;
use crate::error::Result;

/// Constructs a boxed [`SyncBackend`] from a configuration value.
///
/// Adding a new backend only requires adding a new arm here and a new
/// `BackendConfig` variant — no other code needs to change.
pub struct SyncBackendFactory;

impl SyncBackendFactory {
    /// Build a backend from `config`.
    ///
    /// Returns `Err(Error::NotImplemented)` for backends that are defined in
    /// `BackendConfig` but not yet available.
    pub fn from_config(config: BackendConfig) -> Result<Box<dyn SyncBackend>> {
        match config {
            BackendConfig::S3(cfg) => Ok(Box::new(S3Backend::new(cfg)?)),
            BackendConfig::Local(cfg) => Ok(Box::new(LocalBackend::new(&cfg.path))),
            BackendConfig::WebDav(cfg) => Ok(Box::new(WebDavBackend::new(&cfg)?)),
            BackendConfig::Git(cfg) => Ok(Box::new(GitBackend::new(&cfg)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackendConfig, LocalConfig, S3Config};

    #[test]
    fn s3_backend_is_constructed() {
        let cfg = BackendConfig::S3(S3Config {
            bucket: "my-bucket".into(),
            prefix: "lockit/".into(),
            endpoint: Some("http://localhost:9000".into()),
            region: "us-east-1".into(),
            path_style: true,
            access_key_id: "minioadmin".into(),
            secret_access_key: "minioadmin".into(),
        });
        let backend = SyncBackendFactory::from_config(cfg).unwrap();
        assert_eq!(backend.backend_name(), "s3");
    }

    #[test]
    fn local_backend_is_constructed() {
        let cfg = BackendConfig::Local(LocalConfig {
            path: "/tmp/lockit-sync".into(),
        });
        let backend = SyncBackendFactory::from_config(cfg).unwrap();
        assert_eq!(backend.backend_name(), "local");
    }
}
