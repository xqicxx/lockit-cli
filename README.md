# lockit

CLI-first unified credential manager with Zero-Knowledge sync.

Argon2id + AES-256-GCM encryption. BIP39 recovery phrase. Background daemon for fast secret injection.

---

## Key Features

- **Strong crypto**: Argon2id password hashing + AES-256-GCM authenticated encryption
- **Zero-Knowledge sync**: Vault is encrypted client-side before upload; the backend never sees plaintext
- **BIP39 recovery**: 24-word mnemonic lets you recover your vault if you forget your password
- **Background daemon**: `lk daemon start` keeps the vault unlocked in memory for sub-millisecond secret access
- **Shell completion**: `lk generate-completion bash|zsh|fish|powershell`
- **Export / Import**: `.env` and JSON export; import from `.env` files or stdin

---

## Quick Start

```bash
# 1. Install
cargo install --path crates/cli

# 2. Initialize your vault
lk init

# 3. Add and retrieve credentials
lk add myapp --key api_key --value sk-...
lk get myapp api_key
```

---

## Command Reference

| Command | Description |
|---------|-------------|
| `lk init` | Create a new vault (prompts for master password) |
| `lk add <profile> --key <k> --value <v>` | Add or update a credential |
| `lk add <profile> --from-dotenv .env` | Import from a .env file |
| `lk add <profile> --from-env` | Import all current environment variables |
| `lk get <profile> [key]` | Get a value or list all keys in a profile |
| `lk get <profile> --json` | Output as JSON |
| `lk get <profile> --export` | Output as shell export statements |
| `lk list` | List all profiles |
| `lk delete <profile> [key]` | Delete a key or entire profile |
| `lk run --profile <p> -- <cmd>` | Run a command with credentials injected |
| `lk export [profile] [--json]` | Export credentials to stdout |
| `lk import <profile> <file>` | Import credentials from .env file or stdin (-) |
| `lk recover` | Recover vault with BIP39 mnemonic |
| `lk sync push/pull/status/config` | Sync vault with configured backend |
| `lk daemon start/stop/status` | Manage the background daemon |
| `lk generate-completion <shell>` | Print shell completion script |

---

## Architecture

```
+-------------------------------------------------------------+
|  lk CLI (lockit-cli)                                        |
|    +-- init / add / get / list / delete / run / recover     |
|    +-- export / import                                      |
|    +-- sync push | pull | status                            |
|    +-- daemon start | stop | status                         |
+---------------+-----------------------+---------------------+
                | Unix socket (IPC)     | File I/O
                v                       v
+---------------------------+   +----------------------------------+
|  lockit daemon            |   |  ~/.lockit/                      |
|  (lockit-ipc)             |   |    vault.lockit  (encrypted)     |
|                           |   |    device.key                    |
|  Holds unlocked VEK       |   |    credentials  (plaintext INI)  |
|  in memory                |   |    config.toml                   |
+---------------------------+   +----------------------------------+
                |
                v
+---------------------------------------------------------------+
|  lockit-core (crypto engine)                                  |
|    kdf:    Argon2id => Master Key (MK)                        |
|            MK + HKDF-SHA256 => wraps VEK                      |
|    cipher: AES-256-GCM encrypt/decrypt per credential         |
|    vault:  MessagePack serialization with magic header        |
+---------------------------------------------------------------+
                |
                v
+---------------------------------------------------------------+
|  lockit-sync (optional cloud backup)                          |
|    Backends: local | S3-compatible | WebDAV (planned)         |
|    All data encrypted before upload (Zero-Knowledge)          |
+---------------------------------------------------------------+
```

---

## Security Model

1. **Password + Device Key** are combined via HKDF-SHA256 to derive the Master Key (MK)
2. **MK** wraps the Vault Encryption Key (VEK) using AES-256-GCM
3. **VEK** encrypts each credential entry individually with a unique random nonce
4. All keys use `secrecy::Secret<T>` for automatic zeroize-on-drop
5. The vault file begins with a magic header (`LOCKIT01`) and version number

See [SECURITY.md](SECURITY.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

---

## Rust SDK

```toml
[dependencies]
lockit-sdk = { path = "crates/sdk" }
```

```rust
use lockit_sdk::LockitClient;

let client = LockitClient::new()?;
let value = client.get("myapp", "api_key")?;
```

The SDK requires `lk daemon` to be running and the vault to be unlocked.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT — see [LICENSE](LICENSE).
