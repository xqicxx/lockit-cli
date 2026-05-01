use lockit_core::vault::{init_vault, VaultPaths};

use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Create master password")?;
    init_vault(paths, &pw)?;
    output::success(&format!("Vault initialized at {}", paths.vault_path.display()));
    Ok(())
}
