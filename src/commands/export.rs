use anyhow::Result;
use crate::storage;

pub fn run(output: &str) -> Result<()> {
    let creds = storage::read_credentials()?;
    storage::export_to_file(&creds, output)?;
    println!("Exported {} credential(s) to {}", creds.len(), output);
    Ok(())
}
