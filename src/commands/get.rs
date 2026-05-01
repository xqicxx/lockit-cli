use anyhow::Result;
use crate::storage;

pub fn run(name: &str) -> Result<()> {
    let cred = storage::get_credential(name)?;

    match cred {
        Some(c) => {
            println!("Name:    {}", c.name);
            println!("Type:    {}", c.r#type);
            println!("Service: {}", c.service);
            println!("Key:     {}", c.key);
            println!("Value:   {}", c.value);
            println!("Created: {}", c.created_at);
            println!("Updated: {}", c.updated_at);
        }
        None => {
            eprintln!("Credential '{}' not found.", name);
        }
    }

    Ok(())
}
