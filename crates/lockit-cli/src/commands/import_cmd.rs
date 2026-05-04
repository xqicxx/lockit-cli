use lockit_core::credential::CredentialDraft;
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::path::PathBuf;

pub fn run(paths: &VaultPaths, password: Option<String>, file: &PathBuf) -> anyhow::Result<()> {
    let pw = crate::utils::read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &pw)?;
    let content = std::fs::read_to_string(file)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;

    // Export format: {"credentials": [...]}
    let items: Vec<serde_json::Value> = if let Some(arr) = value
        .get("credentials")
        .and_then(|c| c.as_array())
    {
        arr.clone()
    }
    // Array format: [{...}, ...] (Android backup)
    else if let Some(arr) = value.as_array() {
        arr.clone()
    }
    // Legacy markdown
    else {
        let drafts = lockit_core::migration::parse_legacy_markdown(&content)?;
        let count = drafts.len();
        for draft in drafts {
            session.add_credential(draft)?;
        }
        session.save()?;
        println!("Imported {count} credentials from legacy format");
        return Ok(());
    };

    let mut count = 0;
    for item in &items {
        let name = item["name"].as_str().unwrap_or("imported");
        let r#type: lockit_core::credential::CredentialType = item["type"]
            .as_str()
            .unwrap_or("custom")
            .parse()
            .unwrap_or_default();
        let service = item["service"].as_str().unwrap_or("");
        let key = item["key"].as_str().unwrap_or("default");
        let draft = CredentialDraft::new(name, r#type, service, key, item["fields"].clone());
        session.add_credential(draft)?;
        count += 1;
    }
    session.save()?;
    println!("Imported {count} credentials");
    Ok(())
}
