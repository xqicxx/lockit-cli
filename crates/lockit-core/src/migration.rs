use crate::credential::{CredentialDraft, CredentialType};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("invalid metadata json")]
    Metadata(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MigrationError>;

pub fn parse_legacy_markdown(content: &str) -> Result<Vec<CredentialDraft>> {
    let mut drafts = Vec::new();
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("| Name |") {
            in_table = true;
            continue;
        }
        if !in_table || trimmed.starts_with("|---") || trimmed.starts_with("|-") || !trimmed.starts_with('|') {
            continue;
        }

        let cols: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|col| col.trim().replace("\\|", "|"))
            .collect();
        if cols.len() < 8 {
            continue;
        }

        let cred_type = cols[1].parse::<CredentialType>().unwrap_or(CredentialType::Custom);
        let metadata = metadata_map(&cols[5])?;
        let draft = CredentialDraft::new(
            cols[0].clone(),
            cred_type,
            cols[2].clone(),
            cols[3].clone(),
            serde_json::json!({ "value": cols[4].clone() }),
        )
        .with_metadata(metadata);
        drafts.push(draft);
    }

    Ok(drafts)
}

fn metadata_map(json: &str) -> Result<BTreeMap<String, String>> {
    if json.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    let value: serde_json::Value = serde_json::from_str(json)?;
    let map = match value {
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    serde_json::Value::String(value) => value,
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                };
                (key, value)
            })
            .collect(),
        _ => BTreeMap::new(),
    };
    Ok(map)
}
