use anyhow::Context;
use lockit_core::credential_field::credential_fields_for;
use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    name_or_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    let pw = read_password(password)?;
    let session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name_or_id)?;

    if json {
        output::print_json(&[credential]);
    } else {
        display_credential(&credential);
    }

    Ok(())
}

fn display_credential(credential: &lockit_core::credential::RedactedCredential) {
    let short_id: String = credential.id.chars().take(8).collect();
    println!("ID:      {short_id}");
    println!("Name:    {}", credential.name);
    println!("Type:    {}", credential.r#type);
    println!("Service: {}", credential.service);

    let field_defs = credential_fields_for(&credential.r#type);
    if field_defs.is_empty() {
        return;
    }

    println!();
    for field in &field_defs {
        let key = field_label_key(field.label);
        if let Some(value) = credential.fields.get(&key) {
            if !value.is_empty() {
                println!("  {}: {}", field.label, value);
            }
        }
    }
}

fn field_label_key(label: &str) -> String {
    label.to_lowercase().replace(' ', "_")
}

fn read_password(value: Option<String>) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password("Master password: ").context("read password"),
    }
}
