mod credential;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
use commands::{add, delete, export, get, import, init, list};

#[derive(Parser)]
#[command(name = "lockit", about = "Secure credential manager for AI coding tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize lockit vault
    Init,
    /// Add a new credential
    Add {
        /// Credential name
        name: String,
        /// Credential type (api_key, cookie, token, password, custom)
        #[arg(short, long, default_value = "api_key")]
        r#type: String,
        /// Service name (e.g., openai, anthropic)
        #[arg(short, long)]
        service: String,
        /// Key/identifier
        #[arg(short, long, default_value = "default")]
        key: String,
        /// Secret value
        #[arg(short, long)]
        value: String,
    },
    /// Get a credential by name
    Get {
        /// Credential name
        name: String,
    },
    /// List all credentials
    List,
    /// Delete a credential
    Delete {
        /// Credential name
        name: String,
    },
    /// Export credentials to a markdown file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: String,
    },
    /// Import credentials from a markdown file
    Import {
        /// Input file path
        #[arg(short, long)]
        input: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init::run()?,
        Commands::Add {
            name,
            r#type,
            service,
            key,
            value,
        } => add::run(&name, &r#type, &service, &key, &value)?,
        Commands::Get { name } => get::run(&name)?,
        Commands::List => list::run()?,
        Commands::Delete { name } => delete::run(&name)?,
        Commands::Export { output } => export::run(&output)?,
        Commands::Import { input } => import::run(&input)?,
    }

    Ok(())
}
