use lockit_core::vault::{unlock_vault, VaultPaths};
use std::process::Command;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    name: &str,
    cmd: &[String],
) -> anyhow::Result<()> {
    if cmd.is_empty() {
        anyhow::bail!("No command specified. Usage: lockit run <name> -- <command>");
    }
    let pw = crate::utils::read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name)?;
    let prefix = credential.name.to_uppercase().replace(['-', ' '], "_");

    let mut child = Command::new(&cmd[0]);
    child.args(&cmd[1..]);
    for field_name in credential.fields.keys() {
        let secret = session.reveal_secret(name, field_name)?;
        let env_name = format!("{}_{}", prefix, field_name.to_uppercase());
        child.env(&env_name, secret);
    }
    session.save()?;
    let status = child.status()?;
    std::process::exit(status.code().unwrap_or(1));
}
