use anyhow::Context;
use inquire::Select;
use lockit_core::credential::CredentialType;
use lockit_core::credential_field::{credential_fields_for, CredentialField};
use std::collections::BTreeMap;

pub fn select_credential_type() -> Result<CredentialType, anyhow::Error> {
    let types = CredentialType::all();
    let names: Vec<String> = types
        .iter()
        .map(|t| format!("{} — {}", t.name(), t.description()))
        .collect();
    let selection = Select::new("Credential type:", names).prompt()?;

    types
        .into_iter()
        .find(|t| selection.starts_with(&format!("{} —", t.name())))
        .context("credential type not found")
}

pub fn prompt_fields_interactive(
    ct: &CredentialType,
) -> Result<BTreeMap<String, String>, anyhow::Error> {
    let fields = credential_fields_for(ct);
    let mut values = BTreeMap::new();

    for field in &fields {
        let answer = prompt_single_field(field)?;
        if !answer.is_empty() {
            let key = crate::utils::field_label_key(field.label);
            values.insert(key, answer);
        }
    }

    Ok(values)
}

fn prompt_single_field(field: &CredentialField) -> Result<String, anyhow::Error> {
    if field.is_dropdown() {
        prompt_dropdown(field)
    } else if field.secret {
        let prompt = format!("{}: ", field.label);
        Ok(rpassword::prompt_password(prompt).context("read secret field")?)
    } else {
        let key = crate::utils::field_label_key(field.label);
        let default = if key == "key_identifier" {
            Some("default")
        } else {
            None
        };
        let prompt_text = format!("{}:", field.label);
        let mut p = inquire::Text::new(&prompt_text).with_placeholder(field.placeholder);
        if let Some(d) = default {
            p = p.with_default(d);
        }
        Ok(p.prompt()?)
    }
}

fn prompt_dropdown(field: &CredentialField) -> Result<String, anyhow::Error> {
    let mut options: Vec<&str> = field.presets.to_vec();
    options.push("(custom)");
    let selection = Select::new(&format!("{}:", field.label), options).prompt()?;
    if selection == "(custom)" {
        Ok(inquire::Text::new(&format!("{}:", field.label))
            .with_placeholder(field.placeholder)
            .prompt()?)
    } else {
        Ok(selection.to_string())
    }
}
