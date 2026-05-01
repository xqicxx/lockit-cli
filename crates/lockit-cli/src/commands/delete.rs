use anyhow::Context;
use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str) -> anyhow::Result<()> {
    let pw = match password {
        Some(p) => p,
        None => rpassword::prompt_password("Master password: ").context("read password")?,
    };
    let mut session = unlock_vault(paths, &pw)?;
    session.delete_credential(name_or_id)?;
    session.save()?;
    output::success(&format!("Deleted: {name_or_id}"));
    Ok(())
}
