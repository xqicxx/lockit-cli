use clap::{Parser, Subcommand};
use lockit_core::vault::VaultPaths;
use std::path::PathBuf;

mod commands;
mod interactive;
mod output;
mod utils;

#[derive(Parser)]
#[command(name = "lockit", about = "Secure credential manager")]
struct Cli {
    #[arg(long, global = true, help = "Path to vault.enc")]
    vault: Option<PathBuf>,
    #[arg(long, global = true, env = "LOCKIT_MASTER_PASSWORD")]
    password: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    #[command(about = "Google OAuth login for cloud sync")]
    Login,
    #[command(about = "Logout and remove sync config")]
    Logout,
    #[command(about = "Show login status")]
    Whoami,
    #[command(about = "Add a credential (interactive or via --json/--stdin/--file)")]
    Add {
        #[arg(long, help = "JSON input (⚠ exposes secrets in history)")]
        json: Option<String>,
        #[arg(long, help = "Read JSON from stdin (recommended)")]
        stdin: bool,
        #[arg(long, help = "Read JSON from file")]
        file: Option<String>,
    },
    List {
        #[arg(long, help = "Output as JSON")]
        json: bool,
        query: Option<String>,
    },
    Show {
        name_or_id: String,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    #[command(about = "Edit a credential interactively")]
    Edit {
        name_or_id: String,
    },
    Delete {
        name_or_id: String,
    },
    Reveal {
        name_or_id: String,
        #[arg(default_value = "value")]
        field: String,
    },
    #[command(about = "Coding plan quota management")]
    CodingPlan {
        #[command(subcommand)]
        cmd: CodingPlanCmd,
    },
    #[command(about = "Cloud sync management")]
    Sync {
        #[command(subcommand)]
        cmd: SyncCmd,
    },
    #[command(about = "Output export statements for shell eval")]
    Env {
        name: String,
    },
    #[command(about = "Run command with injected credentials")]
    Run {
        name: String,
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    Export {
        name: Option<String>,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    Import {
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum CodingPlanCmd {
    List,
    Refresh { provider: Option<String> },
}

#[derive(Subcommand)]
enum SyncCmd {
    Status,
    Push,
    Pull,
    Config,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let paths = match &cli.vault {
        Some(p) => VaultPaths::new(p.clone()),
        None => VaultPaths::platform_default()?,
    };

    match cli.command {
        Commands::Init => commands::init::run(&paths, cli.password),
        Commands::Login => commands::login::run(&paths),
        Commands::Logout => commands::logout::run(&paths),
        Commands::Whoami => commands::whoami::run(&paths),
        Commands::Add { json, stdin, file } => {
            commands::add::run(&paths, cli.password, json, stdin, file)
        }
        Commands::List { json, query } => commands::list::run(&paths, cli.password, json, query),
        Commands::Show { name_or_id, json } => {
            commands::show::run(&paths, cli.password, &name_or_id, json)
        }
        Commands::Edit { name_or_id } => commands::edit::run(&paths, cli.password, &name_or_id),
        Commands::Delete { name_or_id } => commands::delete::run(&paths, cli.password, &name_or_id),
        Commands::Reveal { name_or_id, field } => {
            commands::reveal::run(&paths, cli.password, &name_or_id, &field)
        }
        Commands::CodingPlan { cmd } => match cmd {
            CodingPlanCmd::List => commands::coding_plan::list(&paths, cli.password),
            CodingPlanCmd::Refresh { provider } => {
                commands::coding_plan::refresh(&paths, cli.password, provider)
            }
        },
        Commands::Sync { cmd } => match cmd {
            SyncCmd::Status => commands::sync_cmd::status(&paths),
            SyncCmd::Push => commands::sync_cmd::push(&paths, cli.password),
            SyncCmd::Pull => commands::sync_cmd::pull(&paths, cli.password),
            SyncCmd::Config => commands::sync_cmd::config(&paths),
        },
        Commands::Env { name } => commands::env_cmd::run(&paths, cli.password, &name),
        Commands::Run { name, cmd } => commands::run_cmd::run(&paths, cli.password, &name, &cmd),
        Commands::Export { name, json } => {
            commands::export_cmd::run(&paths, cli.password, name, json)
        }
        Commands::Import { file } => commands::import_cmd::run(&paths, cli.password, &file),
    }
}
