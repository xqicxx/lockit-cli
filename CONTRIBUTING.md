# Contributing to lockit

Thank you for your interest in contributing!

## Development Environment

1. Install Rust (stable toolchain):
   ```bash
   curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository:
   ```bash
   git clone https://github.com/xqicxx/lockit
   cd lockit
   ```

3. Build everything:
   ```bash
   cargo build --workspace
   ```

4. Run all tests:
   ```bash
   cargo test --workspace
   ```

## Code Standards

- **Formatting**: `cargo fmt` — enforced in CI
- **Linting**: `cargo clippy --all-targets --all-features -- -D warnings` — zero warnings required
- **Tests**: all new code must have tests; existing tests must continue to pass
- **Edition**: Rust 2024
- **Error handling**: use `anyhow` for application errors, `thiserror` for library errors

## PR Workflow

1. Fork the repository and create a feature branch from `main`
2. Make your changes, following the code standards above
3. Run `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings`
4. Run `cargo test --workspace` and confirm all tests pass
5. Open a pull request against `main` with a clear description
6. Address any review feedback

## Workspace Structure

```
crates/
  core/   lockit-core  — cryptographic engine (private API)
  cli/    lockit-cli   — lk binary
  ipc/    lockit-ipc   — daemon IPC protocol + transport
  sync/   lockit-sync  — SyncBackend trait + backends
  sdk/    lockit-sdk   — synchronous Rust client library
```

## Running the CLI During Development

```bash
cargo run --package lockit-cli -- init
cargo run --package lockit-cli -- add myprofile --key api_key --value secret
cargo run --package lockit-cli -- get myprofile api_key
```
