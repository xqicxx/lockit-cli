use anyhow::Context;
use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    name_or_id: &str,
    field: &str,
) -> anyhow::Result<()> {
    let pw = read_password(password)?;
    let mut session = unlock_vault(paths, &pw)?;
    let secret = session.reveal_secret(name_or_id, field)?;
    session.save()?;
    println!("{secret}");
    Ok(())
}

fn read_password(value: Option<String>) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password("Master password: ").context("read password"),
    }
}
