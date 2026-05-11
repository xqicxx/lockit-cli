use lockit_core::vault::{unlock_vault, VaultPaths};

use crate::output;

pub fn run(
    paths: &VaultPaths,
    json: bool,
    query: Option<String>,
) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
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
