use lockit_core::credential::CredentialDraft;
use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::interactive;
use crate::output;

pub fn run(paths: &VaultPaths, name_or_id: &str) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    let mut session = unlock_vault(paths, &pw)?;
    let existing = session.get_credential(name_or_id)?;

    println!("Editing: {} (type: {})", existing.name, existing.r#type);

    let new_fields = interactive::prompt_fields_interactive(&existing.r#type)?;

    let mut merged = existing.fields.clone();
    for (k, v) in &new_fields {
        if !v.is_empty() {
            merged.insert(k.clone(), v.clone());
        }
    }

    let name = merged.get("name").cloned().unwrap_or(existing.name.clone());
    let service = merged
        .get("service")
        .cloned()
        .unwrap_or(existing.service.clone());
    let key = merged
        .get("key_identifier")
        .cloned()
        .unwrap_or(existing.key.clone());

    let draft = CredentialDraft::new(
        &name,
        existing.r#type,
        &service,
        &key,
        serde_json::to_value(&merged)?,
    );

    session.update_credential(name_or_id, draft)?;
    session.save()?;
    output::success(&format!("Updated: {name_or_id}"));
    Ok(())
}
