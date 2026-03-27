# lockit-core Design Specification

**Date:** 2026-03-24
**Issue:** [#2 - lockit-core：加密存储引擎](https://github.com/xqicxx/lockit/issues/2)
**Status:** Approved

---

## 1. Overview

`lockit-core` is the cryptographic foundation for the lockit credential manager. It provides key derivation, encryption/decryption, Vault file format handling, and secure memory management. All crypto operations are internal—external modules never handle raw key material.

## 2. Architecture

### 2.1 Module Structure

```
crates/core/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Public API re-exports
│   ├── error.rs         # Error types
│   ├── kdf.rs           # Argon2id + HKDF key derivation
│   ├── cipher.rs        # AES-256-GCM encryption
│   ├── vault.rs         # VaultFile format and UnlockedVault
│   └── memory.rs        # Internal secret handling
```

### 2.2 Dependency Chain

```
memory.rs (no deps)
    ↓
cipher.rs (uses memory for internal keys)
    ↓
kdf.rs (standalone)
    ↓
vault.rs (uses kdf + cipher + memory)
    ↓
lib.rs (re-exports public API)
```

## 3. Public API

```rust
use secrecy::Secret<Vec<u8>>;

/// Unlocked vault (owns VaultFile + holds VEK securely)
pub struct UnlockedVault { /* private */ }

// === Lifecycle ===

impl UnlockedVault {
    /// Create new vault with password and device key
    pub fn init(password: &str, device_key: &[u8; 32]) -> Result<Self>;

    /// Open existing vault file
    pub fn open(path: &Path, password: &str, device_key: &[u8; 32]) -> Result<Self>;

    /// Save changes to the file this vault was opened from.
    ///
    /// # Errors
    /// - `Error::NoPath` if vault was created with `init()` and no path set
    /// - `Error::Io` if file write fails
    ///
    /// # Atomicity
    /// Writes to a temp file first, then renames to target.
    /// On failure, original file is unchanged.
    pub fn save(&self) -> Result<()>;

    /// Save to a specific path. Updates internal path.
    pub fn save_to(&mut self, path: &Path) -> Result<()>;

    /// Set the default save path (for vaults created with `init()`)
    pub fn set_path(&mut self, path: &Path);

    /// Lock (consume self, zeroize VEK)
    pub fn lock(self);
}

// === Entry Operations ===

impl UnlockedVault {
    /// Set entry value
    pub fn set<K: AsRef<str>>(&mut self, profile: K, key: K, value: &Secret<Vec<u8>>) -> Result<()>;

    /// Get entry value
    pub fn get<K: AsRef<str>>(&self, profile: K, key: K) -> Result<Option<Secret<Vec<u8>>>>;

    /// Delete entry, returns true if existed
    pub fn delete<K: AsRef<str>>(&mut self, profile: K, key: K) -> Result<bool>;

    /// List all profiles
    pub fn profiles(&self) -> Result<Vec<String>>;

    /// List keys in a profile
    pub fn keys<K: AsRef<str>>(&self, profile: K) -> Result<Vec<String>>;

    /// Check if entry exists
    pub fn contains<K: AsRef<str>>(&self, profile: K, key: K) -> Result<bool>;
}

// === Password Management ===

impl UnlockedVault {
    /// Change master password (re-wraps VEK with same salt)
    /// Note: Salt remains unchanged. Only the password-derived key changes.
    pub fn change_password(&mut self, new_password: &str) -> Result<()>;

    /// Get vault file path (if opened from file)
    pub fn path(&self) -> Option<&Path>;
}

// === Helpers ===

pub fn generate_salt() -> Result<[u8; 16]>;
pub fn generate_device_key() -> Result<[u8; 32]>;

pub use error::{Error, Result};
pub use secrecy::Secret;
```

## 4. Internal Design

### 4.1 Key Derivation Flow

```
Master Password + Salt
        │
        ▼
   Argon2id (64 MiB, 3 iter, 4 threads)
        │
        ▼
   Password Key (256 bit)
        │
        │  Device Key (256 bit, from Keychain)
        ▼
   HKDF-SHA256(Password Key || Device Key)
        │
        ▼
   Master Key (MK, 256 bit)
        │
        ▼
   AES-256-GCM unwrap(encrypted_vek) → VEK (256 bit)
```

### 4.2 Vault File Format

```rust
// Serialized with MessagePack
struct VaultFileData {
    magic: [u8; 8],           // b"LOCKIT01"
    version: u16,             // 1
    salt: [u8; 16],           // Argon2id salt
    wrapped_vek: WrappedKey,  // AES-256-GCM encrypted VEK (nonce + tag + ciphertext)
    entries: Vec<Entry>,      // Encrypted entries
}

// Entry-level encryption (profile + key + value as single blob)
struct Entry {
    id: [u8; 16],             // UUID for lookup
    ciphertext: Vec<u8>,      // AES-256-GCM encrypted { profile, key, value }
}

// Internal encrypted blob structure
struct WrappedKey {
    nonce: [u8; 12],          // 96-bit nonce
    ciphertext: Vec<u8>,      // Encrypted key + 128-bit auth tag
}
```

### 4.3 Entry Encryption

Each entry encrypts `{ profile, key, value }` as a single blob:
- Single nonce + tag per entry (28 bytes overhead)
- AEAD authentication covers entire entry
- No separate HMAC needed (AES-GCM provides authentication)

**Entry lookup:** Currently O(n) — decrypt entries one by one to find matching profile/key.
The entry `id` field is a random UUID for internal tracking (not used for lookup).
Future optimization: encrypted index for O(1) lookups (see Section 9).

### 4.4 Nonce Management (Critical)

**AES-GCM security critically depends on never reusing a nonce with the same key.**

#### Engineering Guarantees (Non-Negotiable)

1. **Nonce is NEVER exposed to callers**
   - No public API accepts a nonce parameter
   - No getter exposes nonce values
   - Callers cannot inject or control nonce generation

2. **Nonce is ALWAYS internally generated**
   ```rust
   // Internal only - NOT in public API
   fn encrypt_internal(key: &[u8], plaintext: &[u8]) -> Ciphertext {
       let nonce = generate_random_nonce(); // Internal, always fresh
       aes_gcm_encrypt(key, &nonce, plaintext)
   }
   ```

3. **Type system enforcement**
   - `CipherText` type wraps (nonce, ciphertext, tag) as opaque blob
   - Callers only see `Vec<u8>` result, never individual components
   - Deserialization extracts nonce internally during decrypt

4. **Fresh nonce per encryption**
   - Every `set()` call generates new random nonce
   - No nonce reuse even when overwriting same entry
   - Old ciphertext is replaced entirely (including new nonce)

#### Why This Is Safe

- VEK is unique per vault, rotated only on vault reinitialization
- With 96-bit random nonces and typical vault sizes (<10^6 entries), collision probability is negligible (birthday bound: ~2^-48)
- Each entry gets independent nonce, no counter state to manage

#### If Nonce Is Reused (Catastrophic)

- Confidentiality breaks: XOR of two plaintexts revealed
- Authentication breaks: attacker can forge messages
- **This cannot happen if implementation follows guarantees above**

### 4.5 File Write Atomicity

```
1. Write to temp file: vault.lockit.tmp
2. fsync temp file
3. Rename temp → vault.lockit (atomic on POSIX)
4. fsync parent directory
```

## 5. Security Model

### 5.1 What We Provide (Engineering-Grade Security)

- `secrecy::Secret<T>` wrapper — zeroize on drop, no `Debug`/`Display` impls
- VEK never exposed in logs, panic messages, or debug output
- All sensitive types implement `Zeroize` on drop
- Clear ownership model — `lock()` consumes self

### 5.2 What We Do NOT Guarantee

- `mlock` memory locking — platform-specific, can fail silently
- Protection against memory dumps (requires OS-level protections)
- Protection against cold-boot attacks
- Constant-time operations for all paths

**Rationale:** `mlock` introduces platform complexity, requires elevated privileges, and can fail at runtime. The `secrecy` crate's approach (zeroize + no debug) is sufficient for protecting credentials on disk and preventing accidental exposure.

### 5.3 Zero-Knowledge Guarantee

| Cloud storage sees | Cloud storage never sees |
|-------------------|-------------------------|
| Encrypted entries, salt | Master password, MK, VEK |
| Wrapped VEK, version | Device Key, any credential |

## 6. Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `secrecy` | 0.8 | Secret<T> wrapper with zeroize |
| `zeroize` | 1.8 | Secure memory clearing |
| `argon2` | 0.5 | Argon2id KDF |
| `hkdf` | 0.12 | HKDF-SHA256 |
| `sha2` | 0.10 | SHA-256 for HKDF |
| `aes-gcm` | 0.10 | AES-256-GCM AEAD |
| `rmp-serde` | 1.3 | MessagePack serialization |
| `serde` | 1.0 | Serialization framework |
| `uuid` | 1.10 | Entry IDs |
| `rand` | 0.9 | Secure RNG |
| `thiserror` | 2.0 | Error types |
| `tracing` | 0.1 | Logging |

## 7. Error Types

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Encryption failed: {0}")]
    Encryption(String),

    #[error("Decryption failed: {0}")]
    Decryption(String),

    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    #[error("Invalid salt size: expected {expected}, got {actual}")]
    InvalidSaltSize { expected: usize, actual: usize },

    #[error("Invalid vault file: {0}")]
    InvalidVault(String),

    #[error("Vault file corrupted or tampered")]
    VaultCorrupted,

    #[error("No path set for vault")]
    NoPath,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

## 8. Testing Requirements

- Unit test coverage ≥ 90%
- Test vectors for KDF with known inputs/outputs
- Encryption/decryption round-trip tests
- Tamper detection tests (corrupted ciphertext should fail)
- File format compatibility tests
- Edge cases: empty vault, large entries, special characters in keys

### 8.1 Nonce Correctness Tests (Critical)

These tests verify the security-critical nonce invariant:

| Test | Purpose |
|------|---------|
| `test_nonce_never_exposed` | Verify no public API leaks nonce |
| `test_unique_nonce_per_encrypt` | 1000 encryptions, all nonces unique |
| `test_concurrent_encryption` | Parallel `set()` calls produce unique nonces |
| `test_rng_failure` | Mock RNG failure returns error, doesn't fallback |
| `test_overwrite_new_nonce` | Overwriting entry uses fresh nonce |
| `test_no_nonce_reuse_on_rollback` | Transaction rollback doesn't reuse old nonce |

### 8.2 Fuzzing (Recommended)

- Fuzz `encrypt/decrypt` with arbitrary plaintexts
- Fuzz `VaultFile` deserialization with corrupted data
- Target: no panics, proper error handling on malformed input

## 9. Known Limitations

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| O(n) entry lookup | Performance degrades with large vaults (thousands of entries) | Future: encrypted index for O(1) lookups |
| No mlock | Memory can be swapped to disk | Engineering-grade security, not strong adversary model |
| Random nonces | Theoretical collision risk | 96-bit provides ~2^32 safety margin |
| Entry-level encryption | Must decrypt full entry to access any field | Acceptable for typical entry sizes |

## 10. Future Considerations

- **BIP39 recovery key** — separate module for password recovery
- **Entry indexing** — encrypted index for faster lookups (Option C from design)
- **Compression** — optional compression before encryption for large entries
- **Key rotation** — re-encrypt all entries with new VEK

## 11. Acceptance Criteria

- [ ] `cargo build --release` succeeds on macOS/Linux/Windows
- [ ] All encryption operations internal to crate
- [ ] Stable public API with no raw key exposure
- [ ] `cargo test` passes with ≥ 90% coverage
- [ ] `cargo clippy` zero warnings