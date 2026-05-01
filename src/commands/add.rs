use anyhow::Result;
use crate::credential::{Credential, CredentialType};
use crate::storage;

pub fn run(name: &str, r#type: &str, service: &str, key: &str, value: &str) -> Result<()> {
    if !storage::vault_exists() {
        storage::init_vault()?;
    }

    let cred_type: CredentialType = r#type.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let cred = Credential::new(
        name.to_string(),
        cred_type,
        service.to_string(),
        key.to_string(),
        value.to_string(),
    );

    storage::add_credential(cred)?;
    println!("Credential '{}' added successfully.", name);
    Ok(())
}
