use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(paths: &VaultPaths, name: &str) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    let mut session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name)?;
    let prefix = credential.name.to_uppercase().replace(['-', ' '], "_");

    for field_name in credential.fields.keys() {
        let secret = session.reveal_secret(name, field_name)?;
        let safe_field = crate::utils::sanitize_env_name(field_name);
        let env_name = format!("{}_{}", prefix, safe_field);
        let escaped = secret.replace('\'', "'\\''");
        println!("export {}='{}'", env_name, escaped);
    }

    session.save()?;
    Ok(())
}
