use anyhow::Context;

pub(crate) fn read_password(value: Option<String>, prompt: &str) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password(format!("{prompt}: ")).context("read password"),
    }
}

pub(crate) fn sanitize_env_name(name: &str) -> String {
    name.to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}
