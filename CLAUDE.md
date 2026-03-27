# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build all crates
cargo build

# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test --package lockit-core

# Run a single test
cargo test --package lockit-core -- test_name

# Lint and format
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings

# Run the CLI
cargo run --package lockit-cli -- init
cargo run --package lockit-cli -- add myprofile --key api_key --value secret
cargo run --package lockit-cli -- get myprofile api_key
cargo run --package lockit-cli -- list
```

## Architecture

lockit is a CLI-first credential manager with Zero-Knowledge sync capabilities.

### Workspace Structure

```
crates/
├── core/     # lockit-core: cryptographic foundation (private modules)
└── cli/      # lockit-cli: CLI tool (binary: lk)
```

### Security Model

- **Dual-key architecture**: Master Password + Device Key → Master Key (MK) via HKDF-SHA256
- **Key hierarchy**: MK wraps VEK (Vault Encryption Key); VEK encrypts all credential data
- **Nonce handling**: Nonces are generated internally and NEVER exposed to callers
- **Memory safety**: All keys wrapped in `secrecy::Secret<T>` for zeroize-on-drop

### lockit-core API

`UnlockedVault` is the single public type. All crypto operations are internal.

```rust
// Public API
pub use vault::UnlockedVault;
pub use secrecy::Secret;
pub fn generate_salt() -> Result<[u8; 16]>;
pub fn generate_device_key() -> Result<[u8; 32]>;

// Constants
pub const KEY_SIZE: usize = 32;    // 256-bit
pub const SALT_SIZE: usize = 16;   // 128-bit
pub const NONCE_SIZE: usize = 12;  // 96-bit for AES-GCM
pub const MAGIC: &[u8; 8] = b"LOCKIT01";
pub const VERSION: u16 = 1;
```

### Private Modules (do not expose)

- `cipher`: AES-256-GCM encryption, key wrapping
- `kdf`: Argon2id password hashing, HKDF master key derivation
- `memory`: `VaultEncryptionKey` wrapper with `Secret<[u8; 32]>`

### KDF Parameters (hardcoded)

| Parameter | Value |
|-----------|-------|
| Algorithm | Argon2id |
| Memory | 64 MiB |
| Iterations | 3 |
| Parallelism | 4 |
| Output | 32 bytes |

### Vault File Format

MessagePack serialized with magic bytes + version header. Entries are individually encrypted with unique nonces.

## Coding Standards

- Edition 2024
- Clippy zero warnings required
- Tests must pass before commit
- Follow existing patterns in the codebase