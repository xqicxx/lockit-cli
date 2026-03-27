//! lockit-sync — pluggable sync engine for lockit vaults.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use lockit_sync::factory::SyncBackendFactory;
//! use lockit_sync::config::{BackendConfig, S3Config};
//!
//! let cfg = BackendConfig::S3(S3Config {
//!     bucket: "my-lockit-vault".into(),
//!     region: "us-east-1".into(),
//!     ..Default::default()
//! });
//!
//! let backend = SyncBackendFactory::from_config(cfg)?;
//! backend.upload("vault.lockit", &encrypted_bytes).await?;
//! ```
//!
//! # Crate layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`backend`] | `SyncBackend` trait and `SyncMetadata` |
//! | [`config`] | `BackendConfig` and per-backend config structs |
//! | [`factory`] | `SyncBackendFactory::from_config()` |
//! | [`backends::mock`] | In-memory backend for tests |
//! | [`backends::s3`] | S3-compatible backend |

pub mod backend;
pub mod backends;
pub mod config;
pub mod error;
pub mod factory;
pub(crate) mod util;

pub use backend::{SyncBackend, SyncMetadata};
pub use error::{Error, Result};
