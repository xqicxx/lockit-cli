use anyhow::Result;
use crate::storage;

pub fn run() -> Result<()> {
    let path = storage::init_vault()?;
    println!("Vault initialized at: {}", path.display());
    Ok(())
}
