//! CLI argument definitions for lockit.

use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// lockit — CLI-first unified credential manager.
#[derive(Parser, Debug)]
#[command(name = "lk")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, global = true, default_value = "warn")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new vault with a master password and BIP39 recovery phrase.
    Init,

    /// Add a credential to a profile.
    Add {
        /// Profile name (allowed: a-z, 0-9, hyphens, underscores).
        profile: String,

        /// Key name.
        #[arg(short, long)]
        key: Option<String>,

        /// Value (if omitted with --key, prompts securely without echo).
        #[arg(short, long)]
        value: Option<String>,

        /// Import all current environment variables into the profile.
        #[arg(long)]
        from_env: bool,

        /// Import key=value pairs from a .env file.
        #[arg(long, value_name = "FILE")]
        from_dotenv: Option<String>,

        /// Skip soft vault-size warnings (does not bypass the 10 000-entry hard limit).
        #[arg(long)]
        force: bool,

        /// Expiry date for this credential (ISO-8601: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ).
        /// After this date `lk expire` will list it as expired.
        #[arg(long, value_name = "DATE")]
        expires: Option<String>,
    },

    /// Get a credential value or list all keys in a profile.
    Get {
        /// Profile name.
        profile: String,

        /// Key name (omit to list all keys in the profile).
        key: Option<String>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Output as `export KEY=value` shell statements (eval-safe).
        #[arg(long)]
        export: bool,
    },

    /// List all profiles.
    List,

    /// Delete a key or an entire profile.
    Delete {
        /// Profile name.
        profile: String,

        /// Key name (omit to delete the entire profile).
        key: Option<String>,
    },

    /// Run a command with credentials injected as environment variables.
    ///
    /// Example: lk run --profile myapp -- env | grep API
    ///
    /// If no --profile is given, lockit will look for a .lockitrc file in the
    /// current directory and its parents and use the `profile` key from that file.
    Run {
        /// Profile(s) to inject (repeatable: --profile a --profile b).
        /// Falls back to .lockitrc in the current directory tree if omitted.
        #[arg(short, long, action = clap::ArgAction::Append)]
        profile: Vec<String>,

        /// Add a prefix to all injected variable names (e.g. --prefix MYAPP → MYAPP_API_KEY).
        #[arg(long)]
        prefix: Option<String>,

        /// Clear the environment before injecting — do not inherit parent vars.
        #[arg(long)]
        no_inherit: bool,

        /// Command and arguments to run.
        #[arg(last = true, required = true)]
        cmd: Vec<String>,
    },

    /// Recover vault access using a BIP39 mnemonic and set a new password.
    Recover,

    /// Manage the lockit background daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Sync the vault with a configured remote backend.
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },

    /// Export all credentials to stdout in .env format (or --json).
    Export {
        /// Profile name (omit to export all profiles).
        profile: Option<String>,
        /// Output as JSON instead of .env format.
        #[arg(long)]
        json: bool,
    },

    /// Import credentials from a file or stdin.
    ///
    /// Supported formats: dotenv (default), json, csv, 1password-csv.
    ///
    /// For `json` format the file should be the output of `lk export --json`.
    /// The PROFILE argument is used when the JSON is a flat {key:value} map;
    /// if the JSON contains multiple profiles (the normal export format) each
    /// profile is imported automatically and the PROFILE argument is ignored.
    Import {
        /// Profile name to import into (required for dotenv/csv/1password-csv).
        profile: String,
        /// Path to file (use '-' for stdin).
        file: String,
        /// Skip soft vault-size warnings (does not bypass the 10 000-entry hard limit).
        #[arg(long)]
        force: bool,

        /// Input format.
        #[arg(long, value_enum, default_value = "dotenv")]
        format: ImportFormat,
    },

    /// Unlock the vault and optionally the daemon session.
    ///
    /// Non-interactive (CI) use:  echo "$PASS" | lk unlock --stdin
    /// Biometric use (macOS):     lk unlock --biometric
    /// Save for biometric use:    lk unlock --save-biometric
    Unlock {
        /// Read the vault password from stdin instead of prompting interactively.
        /// Useful for CI/headless environments.
        #[arg(long)]
        stdin: bool,

        /// Unlock using Touch ID / Face ID (macOS only).
        /// Requires a prior `lk unlock --save-biometric` to store the password
        /// in the macOS Keychain.
        #[arg(long, conflicts_with_all = ["stdin", "save_biometric"])]
        biometric: bool,

        /// Prompt for the vault password and save it to the macOS Keychain
        /// protected by Touch ID / Face ID (biometryAny + device passcode).
        /// After this, use `lk unlock --biometric` for passwordless unlocking.
        #[arg(long, conflicts_with = "stdin")]
        save_biometric: bool,

        /// Remove the saved biometric credential from the macOS Keychain.
        #[arg(long, conflicts_with_all = ["stdin", "biometric", "save_biometric"])]
        clear_biometric: bool,
    },

    /// List expired or soon-to-expire credentials.
    ///
    /// Exits non-zero if any credentials are already expired.
    Expire {
        /// Also warn about credentials expiring within this many days (default 30).
        #[arg(long, value_name = "DAYS", default_value = "30")]
        warn_days: u64,
    },

    /// Show vault file metadata (format version, path, size).
    Vault,

    /// Generate shell completion script.
    #[command(hide = true)]
    GenerateCompletion {
        /// Shell type.
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Actions for the sync subcommand.
#[derive(Subcommand, Debug)]
pub enum SyncAction {
    /// Push vault to Google Drive.
    Push,
    /// Pull vault from Google Drive.
    Pull,
    /// Show sync status.
    Status,
    /// Show sync config.
    Config,
    /// Bidirectional sync: automatically push or pull as needed.
    /// Resolves conflicts using the --strategy flag.
    Sync {
        /// Conflict resolution strategy (default: last-write-wins).
        #[arg(long, value_enum, default_value = "last-write-wins")]
        strategy: SyncStrategy,
    },
    /// Check if remote vault changed since last sync (for polling / scripts).
    /// Exits 0 and prints remote checksum if changed, exits 1 if unchanged.
    Poll,
    /// Log in to Google Drive (OAuth).
    Login,
    /// Log out from Google Drive (remove stored tokens).
    Logout,
    /// Generate a new Sync Key or show the existing one.
    Key,
    /// Set the Sync Key from Base64 (to sync with Android or another device).
    SetKey {
        /// Base64-encoded 32-byte sync key.
        key: String,
    },
}

/// Conflict resolution strategy for sync operations.
#[derive(clap::ValueEnum, Debug, Clone, Copy, Default)]
pub enum SyncStrategy {
    /// Keep local version, overwrite remote.
    KeepLocal,
    /// Keep remote version, overwrite local.
    KeepRemote,
    /// Newer timestamp wins.
    #[default]
    LastWriteWins,
}

/// Import file format.
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum ImportFormat {
    /// KEY=VALUE lines, optionally prefixed with `export ` (default).
    Dotenv,
    /// JSON output from `lk export --json`.
    /// Flat `{"KEY":"val"}` → single profile; nested `{"profile":{"KEY":"val"}}` → multi-profile.
    Json,
    /// Two-column CSV with header `key,value`.
    Csv,
    /// 1Password item export CSV (Title,Username,Password,URL,Notes columns).
    #[value(name = "1password-csv")]
    OnePasswordCsv,
}

/// Actions for the daemon subcommand.
#[derive(Subcommand, Debug)]
pub enum DaemonAction {
    /// Start the daemon in the background.
    Start,

    /// Stop the running daemon.
    Stop,

    /// Show daemon status.
    Status,

    /// Run the daemon in the foreground (used internally by `start`).
    #[command(hide = true)]
    Run,
}
