pub mod config;
mod key;
mod payload;
mod status;
pub mod state;
mod sync_ops;

use lockit_core::sync::google_drive::{GoogleDriveBackend, GoogleDriveConfig};
use lockit_core::vault::VaultPaths;

pub use config::config;
pub use key::{key_gen, key_set, key_show};
pub use status::status;
pub use sync_ops::{pull, push, sync};

fn load_backend(paths: &VaultPaths) -> GoogleDriveBackend {
    let mut backend = GoogleDriveBackend::new();
    let cfg_path = state::config_path(paths);
    if cfg_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cfg_path) {
            if let Ok(config) = serde_json::from_str::<GoogleDriveConfig>(&data) {
                backend.configure(config);
            }
        }
    }
    backend
}
