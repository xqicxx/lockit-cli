# Lockit CLI - Gemini Project Instructions

## Project Overview

Lockit is a secure credential manager with a Rust CLI and core library.
Workspace: `lockit-core` (library) + `lockit-cli` (binary).

## Security Requirements

- AES-256-GCM encryption for vault data
- Argon2id key derivation (OWASP params)
- Secret values redacted in all display output except explicit `reveal`
- Zeroize for sensitive data in memory
- Audit trail logging for all security events

## Architecture

### lockit-core (library)
- `crypto.rs` — AES-256-GCM encrypt/decrypt, argon2id key derivation
- `vault.rs` — VaultPayload, VaultSession, CRUD + audit
- `credential.rs` — 18 credential types, secret redaction
- `credential_field.rs` — Field definitions per credential type
- `sync.rs` — SyncBackend trait, manifest protocol
- `sync/google_drive.rs` — GoogleDriveBackend (appDataFolder API)
- `sync/oauth.rs` — PKCE OAuth2 flow with local redirect server
- `coding_plan/` — Provider fetchers (ChatGPT, Claude, DeepSeek, Mimo, Qwen)
- `migration.rs` — Legacy markdown parser

### lockit-cli (binary)
- 14 subcommands via clap
- JSON + table output (tabled crate)
- Interactive prompts (inquire crate)
- Shell integration (env/run commands)

## ⚠️ CRITICAL: What Gemini MUST Review

**ONLY comment on these CRITICAL issues:**

1. **Panic risks** — unwrap/expect on Result/Option that can fail at runtime
2. **Crypto misuse** — Wrong algorithm usage, missing auth tags, weak params
3. **Unsafe blocks** — Unsound unsafe Rust that violates safety invariants
4. **Resource leaks** — Unclosed files, unflushed buffers, unbounded allocations
5. **Logic bugs** — Wrong conditionals, incorrect algorithms, data flow errors
6. **Security** — Secret leakage through logs/stdout, missing redaction, injection

## 🚫 FORBIDDEN: What Gemini MUST NOT Review

**DO NOT comment on these - they are handled by clippy + rustfmt:**

| Forbidden Topic | Why |
|-----------------|-----|
| Variable naming | clippy handles naming |
| Code formatting | rustfmt handles it |
| Design patterns | Subjective - not critical |
| Syntax sugar | Optional - not critical |
| Minor optimizations | Nitpicking - not critical |
| "Consider refactoring" | Not a crash/security issue |

## Response Rules

- **Found critical issue?** → Comment with specific fix and file:line
- **No critical issues?** → Output `LGTM` only
- **Found style issue?** → Ignore it (clippy's job)
- **Found subjective suggestion?** → Ignore it (not critical)
