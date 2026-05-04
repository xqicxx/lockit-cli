use anyhow::Context;
use lockit_core::credential::CredentialDraft;
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::collections::BTreeMap;
use std::io::Read;

use crate::interactive;
use crate::output;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    json_input: Option<String>,
    stdin_input: bool,
    file_input: Option<String>,
) -> anyhow::Result<()> {
    let (cred_type, fields) = read_credential_input(json_input, stdin_input, file_input)?;

    let name = fields.get("name").cloned().unwrap_or_default();
    let service = fields.get("service").cloned().unwrap_or_default();
    let key = fields
        .get("key_identifier")
        .cloned()
        .unwrap_or_else(|| "default".into());

    let password = crate::utils::read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &password)?;
    let draft = CredentialDraft::new(
        &name,
        cred_type.clone(),
        &service,
        &key,
        serde_json::to_value(&fields)?,
    );
    let id = session.add_credential(draft)?;
    session.save()?;
    output::success(&format!("Credential added: {}", &id[..8]));
    Ok(())
}

fn read_credential_input(
    json_input: Option<String>,
    stdin_input: bool,
    file_input: Option<String>,
) -> anyhow::Result<(
    lockit_core::credential::CredentialType,
    BTreeMap<String, String>,
)> {
    if stdin_input {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        parse_json_fields(&buf)
    } else if let Some(path) = file_input {
        let content = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
        parse_json_fields(&content)
    } else if let Some(json) = json_input {
        crate::output::error("--json exposes secrets in shell history. Prefer --stdin or --file.");
        parse_json_fields(&json)
    } else {
        let ct = interactive::select_credential_type()?;
        let fields = interactive::prompt_fields_interactive(&ct)?;
        Ok((ct, fields))
    }
}

fn parse_json_fields(
    json: &str,
) -> anyhow::Result<(
    lockit_core::credential::CredentialType,
    BTreeMap<String, String>,
)> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let cred_type: lockit_core::credential::CredentialType = v["type"]
        .as_str()
        .unwrap_or("custom")
        .parse()
        .unwrap_or(lockit_core::credential::CredentialType::Custom);
    let mut fields: BTreeMap<String, String> = v["fields"]
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, val)| (k.clone(), json_val_to_string(val)))
                .collect()
        })
        .unwrap_or_default();

    // Carry top-level name/service/key into fields if not already present
    for top_key in &["name", "service"] {
        if !fields.contains_key(*top_key) {
            if let Some(val) = v.get(*top_key).and_then(|v| v.as_str()) {
                fields.insert(top_key.to_string(), val.to_string());
            }
        }
    }
    if !fields.contains_key("key_identifier") {
        if let Some(val) = v.get("key").and_then(|v| v.as_str()) {
            fields.insert("key_identifier".to_string(), val.to_string());
        }
    }

    Ok((cred_type, fields))
}

fn json_val_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}
