use crate::commands::sync_cmd::state::config_path;
use crate::output;

pub fn run(paths: &lockit_core::vault::VaultPaths) -> anyhow::Result<()> {
    let cfg_file = config_path(paths);

    if cfg_file.exists() {
        std::fs::remove_file(&cfg_file)?;
        output::success("Logged out. Sync config removed.");
    } else {
        println!("Already logged out.");
    }
    Ok(())
}
