use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(
    paths: &VaultPaths,
    name_or_id: &str,
    yes: bool,
) -> anyhow::Result<()> {
    if !yes {
        match inquire::Confirm::new(&format!("Delete '{}'?", name_or_id))
            .with_default(false)
            .prompt()
        {
            Ok(true) => {}
            Ok(false) => {
                println!("Cancelled.");
                return Ok(());
            }
            Err(_) => {
                // Non-TTY fallback: require --yes for non-interactive use
                anyhow::bail!("Not a terminal. Use --yes to confirm deletion.");
            }
        }
    }

    let pw = crate::utils::vault_key();
    let mut session = unlock_vault(paths, &pw)?;
    session.delete_credential(name_or_id)?;
    session.save()?;
    output::success(&format!("Deleted: {name_or_id}"));
    Ok(())
}
