use anyhow::Context;
use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    json: bool,
    query: Option<String>,
) -> anyhow::Result<()> {
    let pw = read_password(password)?;
    let session = unlock_vault(paths, &pw)?;

    let credentials = match &query {
        Some(q) => session.search_credentials(q),
        None => session.list_credentials(),
    };

    if json {
        output::print_json(&credentials);
    } else {
        output::print_table(&credentials);
    }

    Ok(())
}

fn read_password(value: Option<String>) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password("Master password: ").context("read password"),
    }
}
