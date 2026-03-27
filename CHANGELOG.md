# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `UnlockedVault::save_if_dirty()` — conditional save for future auto-sync (#86)
- POSIX exit codes: `NOT_FOUND=4`, `VAULT_LOCKED=69`, `AUTH_FAILED=77` (#97, #74)
- `lk get` now returns exit code 4 when key is not found (#66)
- All `lk` error messages routed to stderr; credential values stay on stdout (#66)
- `lk run` propagates `128+signal` on Unix when child killed by signal (#87)
- `lk sync push` conflict detection: refuses to overwrite newer remote (#72)
- `SyncMetadata.last_modified/checksum/size` now `Option<T>` — missing metadata
  is explicit instead of silently defaulting to epoch/empty (#79)
- IPC socket file permissions explicitly set to `0600` after bind (#94, #50)
- Recovery phrase on auto-init (`lk add` with no vault) moved to stderr with
  visual warning box — consistent with `lk init` security level (#95, #51)
- `shell_quote` extracted to `crates/cli/src/shell.rs` with 30+ injection tests;
  adds coverage for `|`, `&`, `<`, `>`, `(`, `)`, `*`, `?`, `[`, `]`, `#`, `~` (#100, #81)
- `SECURITY.md` Known Limitations section: AES-256-GCM cipher memory not zeroized,
  tracking RustCrypto upstream (#96, #53)
- WebDAV sync backend (`reqwest` + `quick-xml` PROPFIND) (#15)
- Git sync backend (`git2`, SSH agent auth, auto-clone) (#16)
- IPC peer UID authentication via `SO_PEERCRED` — rejects connections from
  other UIDs (#48)
- Exponential backoff rate limiting on unlock attempts (10 s → 300 s cap) (#49)
- Auto-remove plaintext `credentials` file on startup; `lk add/delete` no longer
  writes credentials to disk in plaintext (#52)
- `docs/plans/` — architecture plans for all 5 crates (#89)
- `docs/VISION.md` — product positioning and roadmap (#75)
- `cargo-dist` release workflow for multi-platform precompiled binaries (#22)

### Changed
- Daemon tokio runtime changed from `new_multi_thread()` to `new_current_thread()` —
  IPC + timer workload does not benefit from a thread pool (#80)
- Daemon singleton check now uses socket `connect()` test instead of PID file read,
  eliminating the TOCTOU race condition (#84)

### Fixed
- Removed auto-write of plaintext `~/.lockit/credentials` file (#52)

---

## [0.1.0] - 2026-03-25

### Added
- `lockit-core`: AES-256-GCM vault encryption, Argon2id KDF, atomic writes,
  BIP39 24-word recovery phrases, dual-key architecture (password + device key)
- `lockit-ipc`: Unix Domain Socket IPC with MessagePack framing
- `lockit-cli` (`lk`): `init`, `add`, `get`, `list`, `delete`, `run`, `export`,
  `import`, `recover`, `sync`, `daemon` commands; shell completion
- `lockit-sync`: `SyncBackend` trait, local and S3 backends, `SyncManager`
- `lockit-sdk`: `Vault` wrapper for embedding lockit in Rust applications
- GitHub Actions CI: lint + test on Linux/macOS/Windows
- `SECURITY.md`: threat model, key hierarchy, security design
