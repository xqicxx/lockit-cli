use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, name: Option<String>, json: bool) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Master password")?;
    let session = unlock_vault(paths, &pw)?;
    let credentials = match name {
        Some(n) => { let cred = session.get_credential(&n)?; vec![cred] }
        None => session.list_credentials(),
    };
    if json {
        output::print_json(&credentials);
    } else {
        for cred in &credentials {
            println!("{}", serde_json::to_string_pretty(cred)?);
        }
    }
    Ok(())
}
