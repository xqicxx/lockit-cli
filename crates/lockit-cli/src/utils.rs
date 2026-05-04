use anyhow::Context;

pub(crate) fn read_password(value: Option<String>, prompt: &str) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password(format!("{prompt}: ")).context("read password"),
    }
}
