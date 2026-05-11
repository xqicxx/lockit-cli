/// Internal vault key — user never sees this. Vault is still encrypted on disk.
pub(crate) fn vault_key() -> String {
    "lockit-default-vault-key-2024".to_string()
}

pub(crate) fn field_label_key(label: &str) -> String {
    label.to_lowercase().replace(' ', "_")
}

pub(crate) fn sanitize_env_name(name: &str) -> String {
    name.to_uppercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
