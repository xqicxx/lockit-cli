use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(
    paths: &VaultPaths,
    name_or_id: &str,
    field: &str,
) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    let mut session = unlock_vault(paths, &pw)?;
    let secret = session.reveal_secret(name_or_id, field)?;
    session.save()?;
    println!("{secret}");
    Ok(())
}
