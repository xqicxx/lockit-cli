use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    name_or_id: &str,
    field: &str,
) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &pw)?;
    let secret = session.reveal_secret(name_or_id, field)?;
    session.save()?;
    println!("{secret}");
    Ok(())
}
