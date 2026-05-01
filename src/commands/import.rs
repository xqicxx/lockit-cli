use anyhow::Result;
use crate::storage;

pub fn run(input: &str) -> Result<()> {
    let imported = storage::import_from_file(input)?;
    let count = imported.len();

    // Merge: add all imported credentials that don't already exist
    let existing = storage::read_credentials().unwrap_or_default();
    let existing_names: std::collections::HashSet<String> = existing
        .iter()
        .map(|c| c.name.to_lowercase())
        .collect();

    let mut added = 0;
    let mut merged = existing;

    for cred in imported {
        if !existing_names.contains(&cred.name.to_lowercase()) {
            merged.push(cred);
            added += 1;
        }
    }

    storage::write_credentials(&merged)?;
    println!("Imported {} new credential(s) from {} ({} skipped, already existed).", added, input, count - added);
    Ok(())
}
