use lockit_core::credential::RedactedCredential;
use serde::Serialize;
use tabled::Tabled;

#[derive(Tabled)]
pub struct CredentialRow {
    pub id: String,
    pub name: String,
    #[tabled(rename = "TYPE")]
    pub cred_type: String,
    pub service: String,
    pub value: String,
}

impl CredentialRow {
    pub fn from_redacted(c: &RedactedCredential) -> Self {
        let value = c.fields.values().next().cloned().unwrap_or_default();
        Self {
            id: c.id.chars().take(8).collect(),
            name: c.name.clone(),
            cred_type: c.r#type.to_string(),
            service: c.service.clone(),
            value,
        }
    }
}

#[derive(Serialize)]
pub struct JsonOutput {
    pub credentials: Vec<RedactedCredential>,
}

pub fn print_table(credentials: &[RedactedCredential]) {
    let rows: Vec<CredentialRow> = credentials.iter().map(CredentialRow::from_redacted).collect();
    if rows.is_empty() {
        println!("(empty)");
        return;
    }
    let mut table = tabled::Table::new(rows);
    table.with(tabled::settings::Style::modern_rounded());
    println!("{table}");
}

pub fn print_json(credentials: &[RedactedCredential]) {
    let output = JsonOutput { credentials: credentials.to_vec() };
    match serde_json::to_string_pretty(&output) {
        Ok(json) => println!("{json}"),
        Err(e) => error(&format!("JSON serialization failed: {e}")),
    }
}

pub fn success(msg: &str) {
    println!("\x1b[32m✓\x1b[0m {msg}");
}

pub fn error(msg: &str) {
    eprintln!("\x1b[31m✗\x1b[0m {msg}");
}
