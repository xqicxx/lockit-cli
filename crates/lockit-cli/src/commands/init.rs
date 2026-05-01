use anyhow::Context;
use lockit_core::vault::{init_vault, VaultPaths};

use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let pw = match password {
        Some(p) => p,
        None => rpassword::prompt_password("Create master password: ").context("read password")?,
    };
    init_vault(paths, &pw)?;
    output::success(&format!("Vault initialized at {}", paths.vault_path.display()));
    Ok(())
}
