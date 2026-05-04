use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output::JsonExport;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    name: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Master password")?;
    let session = unlock_vault(paths, &pw)?;
    let credentials: Vec<_> = match name {
        Some(ref n) => vec![session.get_credential_for_export(n)?],
        None => session.list_credentials_for_export().iter().collect(),
    };
    let wrapper = JsonExport { credentials };
    if json {
        println!("{}", serde_json::to_string_pretty(&wrapper)?);
    } else {
        for cred in wrapper.credentials {
            println!("{}", serde_json::to_string_pretty(cred)?);
        }
    }
    Ok(())
}
