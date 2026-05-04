# lockit

[中文版](README_CN.md) | Secure credential manager with an encrypted vault, cloud sync, and AI coding plan quota tracking.

## Architecture

```
crates/
├── lockit-core/     # Core library (crypto, vault, sync, coding-plan)
└── lockit-cli/      # CLI binary (14 subcommands via clap)
```

## Quick Start

```bash
# Build
cargo build --release

# Initialize a vault
lockit init
#  → Creates ~/.local/share/lockit/vault.enc

# Or specify a custom path
lockit --vault ./my-vault.enc init
```

## Core Workflow

```bash
# Add a credential (interactive)
lockit add

# Add via stdin
echo '{"type":"api_key","name":"OPENAI","fields":{"secret_value":"sk-abc"}}' | lockit add --stdin

# List credentials (table)
lockit list

# List credentials (JSON)
lockit list --json

# Show a credential by name or ID prefix
lockit show openai

# Reveal a specific field
lockit reveal openai secret_value

# Edit interactively
lockit edit openai

# Delete
lockit delete openai
```

## Shell Integration

```bash
# Print export statements for shell eval
lockit env OPENAI
# → export OPENAI_SECRET_VALUE='sk-abc'

# Run a command with injected environment variables
lockit run OPENAI -- curl -H "Authorization: Bearer $OPENAI_SECRET_VALUE" api.example.com
```

## Export & Import

```bash
# Export all credentials (backup)
lockit export --json > backup.json

# Import from backup
lockit import backup.json
```

## Cloud Sync (Google Drive)

```bash
# Login via OAuth (opens browser)
lockit login

# Check login status
lockit whoami

# Sync operations
lockit sync status
lockit sync push
lockit sync pull
lockit sync config
```

## Coding Plan Quota

```bash
# List coding plan credentials
lockit coding-plan list

# Refresh quota for a specific provider
lockit coding-plan refresh openai
```

## Security

- **AES-256-GCM** authenticated encryption
- **Argon2id** key derivation
- All values redacted in display output except explicit `reveal`
- Atomic vault writes (temp file + rename)
- Audit trail logging for all security events

## Environment Variables

| Variable | Description |
|----------|-------------|
| `LOCKIT_MASTER_PASSWORD` | Vault password (avoids interactive prompt) |

## Development

```bash
# Run tests
cargo test

# Run linter
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt --all
```

## License

MIT
