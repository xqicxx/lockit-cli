use anyhow::Result;
use crate::storage;

pub fn run() -> Result<()> {
    let creds = storage::read_credentials()?;

    if creds.is_empty() {
        println!("No credentials found. Use `lockit add` to add one.");
        return Ok(());
    }

    println!("{:<25} {:<12} {:<15} {:<20}", "Name", "Type", "Service", "Key");
    println!("{}", "-".repeat(74));

    for c in &creds {
        println!("{:<25} {:<12} {:<15} {:<20}", c.name, c.r#type, c.service, c.key);
    }

    println!("\nTotal: {} credential(s)", creds.len());
    Ok(())
}
