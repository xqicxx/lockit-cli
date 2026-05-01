mod output;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lockit_core::credential::{CredentialDraft, CredentialType};
use lockit_core::migration::parse_legacy_markdown;
use lockit_core::vault::{init_vault, unlock_vault, VaultPaths};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lockit", about = "Secure credential manager for AI coding tools", version)]
struct Cli {
    #[arg(long, global = true, help = "Path to vault.enc; defaults to platform data directory")]
    vault: Option<PathBuf>,
    #[arg(long, global = true, env = "LOCKIT_MASTER_PASSWORD", help = "Master password; defaults to secure prompt")]
    password: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Add {
        name: String,
        #[arg(short, long, default_value = "api_key")]
        r#type: String,
        #[arg(short, long)]
        service: String,
        #[arg(short, long, default_value = "default")]
        key: String,
        #[arg(short, long)]
        value: String,
    },
    List,
    Get {
        name_or_id: String,
        #[arg(long, help = "Reveal the raw secret value instead of redacted metadata")]
        show: bool,
        #[arg(long, default_value = "value")]
        field: String,
    },
    Search {
        query: String,
    },
    Delete {
        name_or_id: String,
    },
    Update {
        name_or_id: String,
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        r#type: String,
        #[arg(short, long)]
        service: String,
        #[arg(short, long, default_value = "default")]
        key: String,
        #[arg(short, long)]
        value: String,
    },
    ImportLegacy {
        #[arg(short, long)]
        input: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = match cli.vault {
        Some(path) => VaultPaths::new(path),
        None => VaultPaths::platform_default()?,
    };

    match cli.command {
        Commands::Init => {
            let password = read_password(cli.password, "Create master password")?;
            init_vault(&paths, &password)?;
            println!("Vault initialized at {}", paths.vault_path.display());
        }
        Commands::Add { name, r#type, service, key, value } => {
            let mut session = unlocked(&paths, cli.password)?;
            let draft = draft(name, &r#type, service, key, value)?;
            let id = session.add_credential(draft)?;
            session.save()?;
            println!("Credential added: {id}");
        }
        Commands::List => {
            let session = unlocked(&paths, cli.password)?;
            let credentials = session.list_credentials();
            if credentials.is_empty() {
                println!("No credentials found.");
            } else {
                println!("{:<36} {:<22} {:<16} {:<18} VALUE", "ID", "NAME", "TYPE", "SERVICE");
                for credential in credentials {
                    let value = credential.fields.get("value").cloned().unwrap_or_default();
                    println!(
                        "{:<36} {:<22} {:<16} {:<18} {}",
                        credential.id, credential.name, credential.r#type, credential.service, value
                    );
                }
            }
        }
        Commands::Get { name_or_id, show, field } => {
            let mut session = unlocked(&paths, cli.password)?;
            if show {
                println!("{}", session.reveal_secret(&name_or_id, &field)?);
                session.save()?;
            } else {
                let credential = session.get_credential(&name_or_id)?;
                println!("Name:    {}", credential.name);
                println!("Type:    {}", credential.r#type);
                println!("Service: {}", credential.service);
                println!("Key:     {}", credential.key);
                for (field, value) in credential.fields {
                    println!("Field:   {field}={value}");
                }
            }
        }
        Commands::Search { query } => {
            let session = unlocked(&paths, cli.password)?;
            for credential in session.search_credentials(&query) {
                let value = credential.fields.get("value").cloned().unwrap_or_default();
                println!("{}\t{}\t{}\t{}", credential.id, credential.name, credential.r#type, value);
            }
        }
        Commands::Delete { name_or_id } => {
            let mut session = unlocked(&paths, cli.password)?;
            session.delete_credential(&name_or_id)?;
            session.save()?;
            println!("Credential deleted: {name_or_id}");
        }
        Commands::Update { name_or_id, name, r#type, service, key, value } => {
            let mut session = unlocked(&paths, cli.password)?;
            session.update_credential(&name_or_id, draft(name, &r#type, service, key, value)?)?;
            session.save()?;
            println!("Credential updated: {name_or_id}");
        }
        Commands::ImportLegacy { input } => {
            let mut session = unlocked(&paths, cli.password)?;
            let content = std::fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
            let drafts = parse_legacy_markdown(&content)?;
            let count = drafts.len();
            for draft in drafts {
                session.add_credential(draft)?;
            }
            session.save()?;
            println!("Imported {count} legacy credential(s).");
        }
    }

    Ok(())
}

fn unlocked(paths: &VaultPaths, password_value: Option<String>) -> Result<lockit_core::vault::VaultSession> {
    let password = read_password(password_value, "Master password")?;
    Ok(unlock_vault(paths, &password)?)
}

fn read_password(value: Option<String>, prompt: &str) -> Result<String> {
    match value {
        Some(value) => Ok(value),
        None => rpassword::prompt_password(format!("{prompt}: ")).context("read master password"),
    }
}

fn draft(name: String, r#type: &str, service: String, key: String, value: String) -> Result<CredentialDraft> {
    let cred_type = r#type.parse::<CredentialType>().map_err(anyhow::Error::msg)?;
    Ok(CredentialDraft::new(
        name,
        cred_type,
        service,
        key,
        serde_json::json!({ "value": value }),
    ))
}
