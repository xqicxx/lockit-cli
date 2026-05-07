//! lockit-sync — Google Drive sync backend for lockit vaults.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use lockit_sync::google_auth::login;
//! use lockit_sync::factory::SyncBackendFactory;
//!
//! let tokens = login("client_id", "client_secret")?;
//! let backend = SyncBackendFactory::from_token_store(tokens)?;
//! ```
//!
//! # Encrypted sync (compatible with Android)
//!
//! ```rust,ignore
//! use lockit_sync::engine::{SmartSyncEngine, vault_key::VaultKey};
//! use lockit_sync::manifest::SyncManifest;
//! use lockit_sync::sync_crypto;
//!
//! // Generate or load the shared sync key
//! let key = VaultKey::generate();
//! println!("Sync Key: {}", key.to_base64());
//!
//! let engine = SmartSyncEngine::new_with_key(backend, state, vault_path, key);
//! let outcome = engine.sync().await?;
//! ```
//!
//! # Crate layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`backend`] | `SyncBackend` trait and `SyncMetadata` |
//! | [`backends::google_drive`] | Google Drive backend (appDataFolder) |
//! | [`backends::mock`] | In-memory backend for tests |
//! | [`auth`] | Google OAuth 2.0 login and token refresh |
//! | [`config`] | `GoogleDriveConfig` and `GoogleTokenStore` |
//! | [`factory`] | `SyncBackendFactory::from_token_store()` |
//! | [`state`] | `SyncState` — persistent checksum tracking |
//! | [`engine::conflict`] | `ConflictDetector`, `ResolveStrategy`, `SyncOutcome` |
//! | [`engine::sync`] | `SmartSyncEngine` — high-level sync orchestration |
//! | [`manifest`] | `SyncManifest` — cloud metadata (Android-compatible) |
//! | [`sync_crypto`] | AES-256-GCM vault encryption (Android-compatible) |

pub mod auth;
pub mod backend;
pub mod backends;
pub mod config;
pub mod engine;
pub mod error;
pub mod factory;
pub mod manifest;
pub mod state;
pub mod sync_crypto;
pub(crate) mod http;
pub(crate) mod util;

pub use backend::{SyncBackend, SyncMetadata};
pub use engine::conflict;
pub use engine::{ConflictDetector, ResolveStrategy, SyncConflict, SyncOutcome};
pub use engine::{SmartSyncEngine, SyncError};
pub use error::{Error, Result};
pub use manifest::SyncManifest;
pub use state::SyncState;

pub mod google_auth {
    //! Backward-compatible Google auth exports.

    pub use crate::auth::login::login;
    pub use crate::auth::token::{is_token_valid, refresh_tokens};
}

/// Compute the SHA-256 digest of `data` as lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    util::sha256_hex(data)
}
