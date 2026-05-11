mod config;
mod payload;
pub mod state;
mod status;
mod sync_ops;

use lockit_core::sync::google_drive::{GoogleDriveBackend, GoogleDriveConfig};
use lockit_core::vault::VaultPaths;

pub use config::{config, key};
pub use status::status;
pub use sync_ops::{pull, push, sync};

fn load_backend(paths: &VaultPaths) -> GoogleDriveBackend {
    let backend = GoogleDriveBackend::new();
    let cfg_path = state::config_path(paths);
    if let Ok(data) = std::fs::read_to_string(&cfg_path) {
        if let Ok(cfg) = serde_json::from_str::<GoogleDriveConfig>(&data) {
            backend.configure(cfg);
        }
    }
    backend
}
