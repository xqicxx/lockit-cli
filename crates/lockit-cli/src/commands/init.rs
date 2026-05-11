use lockit_core::vault::{init_vault, VaultPaths};

use crate::output;

pub fn run(paths: &VaultPaths) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    init_vault(paths, &pw)?;
    output::success(&format!(
        "Vault initialized at {}",
        paths.vault_path.display()
    ));
    Ok(())
}
