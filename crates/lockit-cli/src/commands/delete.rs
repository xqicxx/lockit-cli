use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &pw)?;
    session.delete_credential(name_or_id)?;
    session.save()?;
    output::success(&format!("Deleted: {name_or_id}"));
    Ok(())
}
