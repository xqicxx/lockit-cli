use anyhow::Result;
use crate::storage;

pub fn run(name: &str) -> Result<()> {
    let deleted = storage::delete_credential(name)?;

    if deleted {
        println!("Credential '{}' deleted.", name);
    } else {
        eprintln!("Credential '{}' not found.", name);
    }

    Ok(())
}
