use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::credential::Credential;

const MARKDOWN_HEADER: &str = "# Lockit Credentials\n\n| Name | Type | Service | Key | Value | Metadata | Created | Updated |\n|------|------|---------|-----|-------|----------|---------|--------|\n";

pub fn vault_path() -> Result<PathBuf> {
    let dirs = directories::BaseDirs::new().ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
    let dir = dirs.home_dir().join(".lockit");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("lockit.md"))
}

pub fn init_vault() -> Result<PathBuf> {
    let path = vault_path()?;
    if !path.exists() {
        fs::write(&path, MARKDOWN_HEADER)?;
    }
    Ok(path)
}

pub fn vault_exists() -> bool {
    vault_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn read_credentials() -> Result<Vec<Credential>> {
    let path = vault_path()?;
    if !path.exists() {
        return Err(anyhow::anyhow!("Vault not initialized. Run `lockit init` first."));
    }

    let content = fs::read_to_string(&path)?;
    parse_markdown_table(&content)
}

pub fn write_credentials(creds: &[Credential]) -> Result<()> {
    let path = vault_path()?;
    let content = to_markdown_table(creds);
    fs::write(&path, content)?;
    Ok(())
}

pub fn add_credential(cred: Credential) -> Result<()> {
    let mut creds = read_credentials().unwrap_or_default();
    creds.push(cred);
    write_credentials(&creds)
}

pub fn get_credential(name: &str) -> Result<Option<Credential>> {
    let creds = read_credentials()?;
    Ok(creds.into_iter().find(|c| c.name.to_lowercase() == name.to_lowercase()))
}

pub fn delete_credential(name: &str) -> Result<bool> {
    let creds = read_credentials()?;
    let len = creds.len();
    let creds: Vec<_> = creds
        .into_iter()
        .filter(|c| c.name.to_lowercase() != name.to_lowercase())
        .collect();
    if creds.len() < len {
        write_credentials(&creds)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn parse_markdown_table(content: &str) -> Result<Vec<Credential>> {
    let mut creds = Vec::new();
    let mut in_table = false;
    let mut header_indices: Vec<usize> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip header line
        if trimmed.starts_with("| Name |") {
            in_table = true;
            // Parse header to find column indices
            let cols: Vec<&str> = trimmed.split('|').collect();
            for (i, col) in cols.iter().enumerate() {
                match col.trim().to_lowercase().as_str() {
                    "name" => header_indices.push(i),
                    "type" => header_indices.push(i),
                    "service" => header_indices.push(i),
                    "key" => header_indices.push(i),
                    "value" => header_indices.push(i),
                    "metadata" => header_indices.push(i),
                    "created" => header_indices.push(i),
                    "updated" => header_indices.push(i),
                    _ => {}
                }
            }
            continue;
        }

        // Skip separator line
        if in_table && (trimmed.starts_with("|---") || trimmed.starts_with("|-")) {
            continue;
        }

        if in_table && trimmed.starts_with('|') {
            let cols: Vec<&str> = trimmed.split('|').collect();
            if cols.len() < 9 {
                continue;
            }

            let get_col = |idx: usize| -> &str {
                if idx < cols.len() { cols[idx].trim() } else { "" }
            };

            // Default column order: Name, Type, Service, Key, Value, Metadata, Created, Updated
            let name = get_col(1).to_string();
            let r#type = get_col(2).parse().unwrap_or(crate::credential::CredentialType::Custom);
            let service = get_col(3).to_string();
            let key = get_col(4).to_string();
            let value = get_col(5).to_string();
            let metadata: serde_json::Value = serde_json::from_str(get_col(6))
                .unwrap_or(serde_json::json!({}));
            let created_at = chrono::DateTime::parse_from_rfc3339(get_col(7))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let updated_at = chrono::DateTime::parse_from_rfc3339(get_col(8))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            creds.push(Credential {
                id: uuid::Uuid::new_v4(),
                name,
                r#type,
                service,
                key,
                value,
                metadata,
                created_at,
                updated_at,
            });
        }
    }

    Ok(creds)
}

fn to_markdown_table(creds: &[Credential]) -> String {
    let mut output = String::from(MARKDOWN_HEADER);

    for cred in creds {
        let metadata_str = serde_json::to_string(&cred.metadata).unwrap_or_else(|_| "{}".to_string());
        let created = cred.created_at.to_rfc3339();
        let updated = cred.updated_at.to_rfc3339();

        // Escape pipe characters in values
        let escape_pipe = |s: &str| s.replace('|', "\\|");

        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            escape_pipe(&cred.name),
            escape_pipe(&cred.r#type.to_string()),
            escape_pipe(&cred.service),
            escape_pipe(&cred.key),
            escape_pipe(&cred.value),
            escape_pipe(&metadata_str),
            escape_pipe(&created),
            escape_pipe(&updated),
        ));
    }

    output
}

pub fn export_to_file(creds: &[Credential], path: &str) -> Result<()> {
    let content = to_markdown_table(creds);
    fs::write(path, content)?;
    Ok(())
}

pub fn import_from_file(path: &str) -> Result<Vec<Credential>> {
    let content = fs::read_to_string(path)?;
    parse_markdown_table(&content)
}
