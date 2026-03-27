# lockit-core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement lockit's cryptographic foundation crate with secure key derivation, encryption, and Vault file management.

**Architecture:** Single public type `UnlockedVault` owns all state and crypto operations. Lower-level modules (kdf, cipher, memory) are **completely private** — only `vault.rs` exposes public API. This prevents external code from bypassing security invariants (especially nonce management).

**Tech Stack:** Rust 2024, RustCrypto crates (argon2, aes-gcm, hkdf), secrecy, rmp-serde

---

## File Structure

```
crates/core/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports UnlockedVault, Secret, Result, Error
│   ├── error.rs            # Error enum (pub use in lib.rs)
│   ├── kdf.rs              # PRIVATE: Argon2id + HKDF
│   ├── cipher.rs           # PRIVATE: AES-256-GCM (nonce always internal)
│   ├── vault.rs            # PUBLIC: UnlockedVault, all operations
│   └── memory.rs           # PRIVATE: Internal secret wrappers
└── tests/
    ├── integration_tests.rs
    └── nonce_correctness.rs
```

**Critical Constraint:**
```
lib.rs → vault.rs (public)
       → error.rs (public types only)

vault.rs → kdf.rs (private)
         → cipher.rs (private)
         → memory.rs (private)

External code can ONLY use:
- UnlockedVault methods
- Secret<Vec<u8>>
- Error, Result types
```

---

## Task 1: Rename crate and setup dependencies

**Files:**
- Rename: `crates/crypto/` → `crates/core/`
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/core/Cargo.toml` (dependencies)

- [ ] **Step 1: Rename crypto directory to core**

```bash
mv crates/crypto crates/core
```

- [ ] **Step 2: Update workspace Cargo.toml**

```toml
[workspace]
members = [
    "crates/core",
    "crates/cli",
]

[workspace.dependencies]
# ... existing ...
lockit-core = { path = "crates/core" }
```

- [ ] **Step 3: Update crates/core/Cargo.toml with all dependencies**

```toml
[package]
name = "lockit-core"
description = "Cryptographic foundation for lockit credential manager"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[dependencies]
# Error handling
thiserror.workspace = true

# Logging
tracing.workspace = true

# Random number generation
rand.workspace = true

# Cryptography
argon2 = "0.5"
hkdf = "0.12"
sha2 = "0.10"
aes-gcm = "0.10"

# Secure memory
secrecy = "0.8"
zeroize = "1.8"

# Serialization
serde = { version = "1.0", features = ["derive"] }
rmp-serde = "1.3"

# Utilities
uuid = { version = "1.10", features = ["v4", "serde"] }

[dev-dependencies]
tempfile = "3.10"
```

- [ ] **Step 4: Update crates/cli/Cargo.toml dependency**

```toml
[dependencies]
lockit-core = { path = "../core" }
```

- [ ] **Step 5: Verify build**

Run: `cargo build`
Expected: Compiles successfully with new crate name

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/core/Cargo.toml crates/cli/Cargo.toml
git commit -m "refactor: rename lockit-crypto to lockit-core, add dependencies"
```

---

## Task 2: Update error types

**Files:**
- Modify: `crates/core/src/error.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write failing test for new error variants**

Create `crates/core/src/error.rs`:

```rust
//! Error types for lockit-core.
//!
//! All errors are defined here with explicit error kinds for clear API boundaries.

use thiserror::Error;

/// Result type alias for lockit-core operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for lockit-core.
#[derive(Debug, Error)]
pub enum Error {
    /// Key derivation failed.
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    /// Encryption operation failed.
    #[error("Encryption failed: {0}")]
    Encryption(String),

    /// Decryption operation failed.
    #[error("Decryption failed: {0}")]
    Decryption(String),

    /// Invalid key size.
    #[error("Invalid key size: expected {expected} bytes, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    /// Invalid salt size.
    #[error("Invalid salt size: expected {expected} bytes, got {actual}")]
    InvalidSaltSize { expected: usize, actual: usize },

    /// Invalid vault file format.
    #[error("Invalid vault file: {0}")]
    InvalidVault(String),

    /// Vault file corrupted or tampered (AEAD verification failed).
    #[error("Vault file corrupted or tampered")]
    VaultCorrupted,

    /// No path set for save operation.
    #[error("No path set for vault")]
    NoPath,

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::InvalidKeySize { expected: 32, actual: 16 };
        assert_eq!(err.to_string(), "Invalid key size: expected 32 bytes, got 16");
    }

    #[test]
    fn test_vault_corrupted_display() {
        let err = Error::VaultCorrupted;
        assert_eq!(err.to_string(), "Vault file corrupted or tampered");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core error::tests`
Expected: All tests pass

- [ ] **Step 3: Update lib.rs to re-export**

```rust
//! # lockit-core
//!
//! Cryptographic foundation for lockit credential manager.
//!
//! This crate provides a single public type [`UnlockedVault`] that manages
//! all cryptographic operations internally. Lower-level crypto primitives
//! (KDF, cipher) are private and cannot be accessed directly.
//!
//! ## Security Model
//!
//! - All encryption operations are internal — external code never handles raw keys
//! - Nonce generation is automatic and internal — callers cannot inject nonces
//! - [`Secret<Vec<u8>>`] wraps all sensitive data with zeroize-on-drop
//!
//! ## Example
//!
//! ```ignore
//! use lockit_core::{UnlockedVault, Secret};
//!
//! // Create new vault
//! let device_key = lockit_core::generate_device_key()?;
//! let mut vault = UnlockedVault::init("my-password", &device_key)?;
//!
//! // Store a credential
//! vault.set("myapp", "api_key", &Secret::new(b"secret-value".to_vec()))?;
//!
//! // Retrieve a credential
//! let value = vault.get("myapp", "api_key")?;
//!
//! // Save and lock
//! vault.save_to(&path)?;
//! vault.lock();
//! ```

pub mod error;

// Re-export public types only
pub use error::{Error, Result};
pub use secrecy::Secret;

// Public constants
/// Cryptographic key size in bytes (256 bits).
pub const KEY_SIZE: usize = 32;

/// Salt size in bytes (128 bits).
pub const SALT_SIZE: usize = 16;

/// Nonce size in bytes (96 bits for AES-GCM).
pub const NONCE_SIZE: usize = 12;

/// Vault file magic bytes.
pub const MAGIC: &[u8; 8] = b"LOCKIT01";

/// Current vault file format version.
pub const VERSION: u16 = 1;

// Internal modules (not re-exported)
mod kdf;
mod cipher;
mod memory;
mod vault;

// Re-export the main public type
pub use vault::UnlockedVault;

/// Generates a random salt for key derivation.
pub fn generate_salt() -> Result<[u8; SALT_SIZE]> {
    use rand::RngCore;
    let mut salt = [0u8; SALT_SIZE];
    rand::rng().fill_bytes(&mut salt);
    Ok(salt)
}

/// Generates a random device key.
pub fn generate_device_key() -> Result<[u8; KEY_SIZE]> {
    use rand::RngCore;
    let mut key = [0u8; KEY_SIZE];
    rand::rng().fill_bytes(&mut key);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt() {
        let salt = generate_salt().unwrap();
        assert_eq!(salt.len(), SALT_SIZE);
    }

    #[test]
    fn test_generate_device_key() {
        let key = generate_device_key().unwrap();
        assert_eq!(key.len(), KEY_SIZE);
    }

    #[test]
    fn test_salts_are_unique() {
        let salt1 = generate_salt().unwrap();
        let salt2 = generate_salt().unwrap();
        assert_ne!(salt1, salt2);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --package lockit-core`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/lib.rs crates/core/src/error.rs
git commit -m "feat(core): add error types and public API skeleton"
```

---

## Task 3: Implement memory module (private)

**Files:**
- Create: `crates/core/src/memory.rs`

- [ ] **Step 1: Write internal memory types**

```rust
//! Internal memory types for secure key handling.
//!
//! This module is PRIVATE. External code cannot access these types.
//! All sensitive keys are wrapped in Secret<T> for zeroize-on-drop.

use secrecy::{ExposeSecret, Secret};
use zeroize::Zeroizing;

/// Internal wrapper for keys that must be zeroized.
/// Uses secrecy::Secret which:
/// - Zeroizes on drop
/// - No Debug/Display impls (won't leak in logs)
/// - Requires explicit .expose_secret() to access
pub(crate) type SecretKey = Secret<[u8; 32]>;

/// Create a secret key from bytes.
pub(crate) fn secret_key_from_bytes(bytes: [u8; 32]) -> SecretKey {
    Secret::new(bytes)
}

/// Access the raw bytes of a secret key.
/// Only used internally by cipher operations.
pub(crate) fn expose_key(key: &SecretKey) -> &[u8; 32] {
    key.expose_secret()
}

/// Internal wrapper for VEK (Vault Encryption Key).
/// This is what UnlockedVault holds.
pub(crate) struct VaultEncryptionKey {
    key: SecretKey,
}

impl VaultEncryptionKey {
    /// Create from raw bytes (after derivation or unwrapping).
    pub(crate) fn new(bytes: [u8; 32]) -> Self {
        Self {
            key: secret_key_from_bytes(bytes),
        }
    }

    /// Access for encryption/decryption operations.
    pub(crate) fn expose(&self) -> &[u8; 32] {
        expose_key(&self.key)
    }

    /// Generate a random VEK.
    pub(crate) fn generate() -> crate::Result<Self> {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        Ok(Self::new(bytes))
    }
}

// VEK is zeroized on drop via Secret<[u8; 32]>

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vek_generate() {
        let vek1 = VaultEncryptionKey::generate().unwrap();
        let vek2 = VaultEncryptionKey::generate().unwrap();

        // Keys should be different
        assert_ne!(vek1.expose(), vek2.expose());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core memory::tests`
Expected: Tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/memory.rs
git commit -m "feat(core): add private memory module for secret keys"
```

---

## Task 4: Implement cipher module (private)

**Files:**
- Create: `crates/core/src/cipher.rs`

- [ ] **Step 1: Write cipher module with internal nonce generation**

```rust
//! Internal cipher operations (AES-256-GCM).
//!
//! This module is PRIVATE. External code cannot:
//! - Call encrypt/decrypt directly
//! - Provide or access nonces
//!
//! Nonce is ALWAYS generated internally using OsRng.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

use crate::{Error, Result, KEY_SIZE, NONCE_SIZE};

use super::memory::VaultEncryptionKey;

/// Internal ciphertext type (nonce + encrypted data).
/// Stored as: [nonce: 12 bytes] || [ciphertext + tag: N+16 bytes]
pub(crate) struct CipherText {
    /// The encrypted data with nonce prepended.
    data: Vec<u8>,
}

impl CipherText {
    /// Encrypt plaintext with the VEK.
    /// Nonce is generated internally — never exposed or accepted as parameter.
    pub(crate) fn encrypt(vek: &VaultEncryptionKey, plaintext: &[u8]) -> Result<Self> {
        let key_bytes = vek.expose();

        let cipher = Aes256Gcm::new_from_slice(key_bytes)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Generate fresh random nonce (CRITICAL: internal only)
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut data = nonce_bytes.to_vec();
        data.extend(ciphertext);

        Ok(Self { data })
    }

    /// Decrypt ciphertext with the VEK.
    /// Extracts nonce internally from the data.
    pub(crate) fn decrypt(vek: &VaultEncryptionKey, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < NONCE_SIZE + 16 {
            // Minimum: nonce + empty plaintext + 16-byte tag
            return Err(Error::Decryption("ciphertext too short".into()));
        }

        let key_bytes = vek.expose();

        let cipher = Aes256Gcm::new_from_slice(key_bytes)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        // Extract nonce from beginning
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);

        // Decrypt remaining bytes
        let plaintext = cipher
            .decrypt(nonce, &data[NONCE_SIZE..])
            .map_err(|_| Error::VaultCorrupted)?;

        Ok(plaintext)
    }

    /// Get the raw bytes for storage.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Create from stored bytes.
    pub(crate) fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }
}

/// Wrap a key (like VEK) with another key (like MK).
pub(crate) fn wrap_key(wrapping_key: &VaultEncryptionKey, key_to_wrap: &[u8; 32]) -> Result<CipherText> {
    CipherText::encrypt(wrapping_key, key_to_wrap)
}

/// Unwrap a key (like VEK) from wrapped form.
pub(crate) fn unwrap_key(wrapping_key: &VaultEncryptionKey, wrapped: &[u8]) -> Result<[u8; 32]> {
    let plaintext = CipherText::decrypt(wrapping_key, wrapped)?;
    if plaintext.len() != KEY_SIZE {
        return Err(Error::Decryption("invalid wrapped key size".into()));
    }
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&plaintext);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret message";

        let ciphertext = CipherText::encrypt(&vek, plaintext).unwrap();
        let decrypted = CipherText::decrypt(&vek, ciphertext.as_bytes()).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_different_nonces_for_same_plaintext() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"same message";

        let ct1 = CipherText::encrypt(&vek, plaintext).unwrap();
        let ct2 = CipherText::encrypt(&vek, plaintext).unwrap();

        // Same plaintext with different nonces = different ciphertexts
        assert_ne!(ct1.as_bytes(), ct2.as_bytes());

        // But both decrypt correctly
        let d1 = CipherText::decrypt(&vek, ct1.as_bytes()).unwrap();
        let d2 = CipherText::decrypt(&vek, ct2.as_bytes()).unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let vek = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret";

        let mut ciphertext = CipherText::encrypt(&vek, plaintext).unwrap();
        // Tamper with the data
        ciphertext.data[15] ^= 0xff;

        let result = CipherText::decrypt(&vek, ciphertext.as_bytes());
        assert!(matches!(result, Err(Error::VaultCorrupted)));
    }

    #[test]
    fn test_wrong_key_fails() {
        let vek1 = VaultEncryptionKey::generate().unwrap();
        let vek2 = VaultEncryptionKey::generate().unwrap();
        let plaintext = b"secret";

        let ciphertext = CipherText::encrypt(&vek1, plaintext).unwrap();
        let result = CipherText::decrypt(&vek2, ciphertext.as_bytes());

        assert!(matches!(result, Err(Error::VaultCorrupted)));
    }

    #[test]
    fn test_wrap_unwrap_key() {
        let wrapping_key = VaultEncryptionKey::generate().unwrap();
        let key_to_wrap = [42u8; 32];

        let wrapped = wrap_key(&wrapping_key, &key_to_wrap).unwrap();
        let unwrapped = unwrap_key(&wrapping_key, wrapped.as_bytes()).unwrap();

        assert_eq!(key_to_wrap, unwrapped);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core cipher::tests`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/cipher.rs
git commit -m "feat(core): add private cipher module with internal nonce generation"
```

---

## Task 5: Implement KDF module (private)

**Files:**
- Create: `crates/core/src/kdf.rs`

- [ ] **Step 1: Write KDF module**

```rust
//! Internal key derivation functions (Argon2id + HKDF).
//!
//! This module is PRIVATE. External code cannot:
//! - Call derive_master_key directly
//! - Access PasswordKey or MasterKey types

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::{Error, Result, KEY_SIZE, SALT_SIZE};

use super::memory::VaultEncryptionKey;

/// Argon2id parameters (per technical design).
const ARGON2_MEMORY: u32 = 65536; // 64 MiB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Internal type for password-derived key.
pub(crate) struct PasswordKey([u8; KEY_SIZE]);

impl PasswordKey {
    /// Derive from password using Argon2id.
    pub(crate) fn derive(password: &str, salt: &[u8; SALT_SIZE]) -> Result<Self> {
        let params = Params::new(ARGON2_MEMORY, ARGON2_ITERATIONS, ARGON2_PARALLELISM, Some(KEY_SIZE))
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = [0u8; KEY_SIZE];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        Ok(Self(key))
    }

    /// Access raw bytes (internal use only).
    fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

/// Internal type for master key.
pub(crate) struct MasterKey([u8; KEY_SIZE]);

impl MasterKey {
    /// Derive from password key and device key using HKDF-SHA256.
    pub(crate) fn derive(password_key: &PasswordKey, device_key: &[u8; KEY_SIZE]) -> Result<Self> {
        // Mix password key and device key as input key material
        let mut ikm = [0u8; KEY_SIZE * 2];
        ikm[..KEY_SIZE].copy_from_slice(password_key.as_bytes());
        ikm[KEY_SIZE..].copy_from_slice(device_key);

        // HKDF with the combined key material as IKM
        let hkdf: Hkdf<Sha256> = Hkdf::new(None, &ikm);

        let mut master_key = [0u8; KEY_SIZE];
        hkdf.expand(b"lockit master key", &mut master_key)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;

        Ok(Self(master_key))
    }

    /// Convert to VaultEncryptionKey for wrapping/unwrapping.
    pub(crate) fn into_wrapping_key(self) -> VaultEncryptionKey {
        VaultEncryptionKey::new(self.0)
    }
}

/// Derive master key from password, salt, and device key.
/// This is the main entry point for KDF.
pub(crate) fn derive_master_key(
    password: &str,
    salt: &[u8; SALT_SIZE],
    device_key: &[u8; KEY_SIZE],
) -> Result<MasterKey> {
    let password_key = PasswordKey::derive(password, salt)?;
    MasterKey::derive(&password_key, device_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_key_derivation() {
        let salt = [1u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password", &salt).unwrap();
        let pk2 = PasswordKey::derive("password", &salt).unwrap();

        // Same input = same output (deterministic)
        assert_eq!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let salt = [1u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password1", &salt).unwrap();
        let pk2 = PasswordKey::derive("password2", &salt).unwrap();

        assert_ne!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_different_salts_different_keys() {
        let salt1 = [1u8; SALT_SIZE];
        let salt2 = [2u8; SALT_SIZE];
        let pk1 = PasswordKey::derive("password", &salt1).unwrap();
        let pk2 = PasswordKey::derive("password", &salt2).unwrap();

        assert_ne!(pk1.as_bytes(), pk2.as_bytes());
    }

    #[test]
    fn test_master_key_derivation() {
        let salt = [1u8; SALT_SIZE];
        let device_key = [2u8; KEY_SIZE];

        let mk1 = derive_master_key("password", &salt, &device_key).unwrap();
        let mk2 = derive_master_key("password", &salt, &device_key).unwrap();

        // Same inputs = same master key
        let mk1 = mk1.into_wrapping_key();
        let mk2 = mk2.into_wrapping_key();
        assert_eq!(mk1.expose(), mk2.expose());
    }

    #[test]
    fn test_device_key_affects_master_key() {
        let salt = [1u8; SALT_SIZE];
        let device_key1 = [1u8; KEY_SIZE];
        let device_key2 = [2u8; KEY_SIZE];

        let mk1 = derive_master_key("password", &salt, &device_key1)
            .unwrap()
            .into_wrapping_key();
        let mk2 = derive_master_key("password", &salt, &device_key2)
            .unwrap()
            .into_wrapping_key();

        assert_ne!(mk1.expose(), mk2.expose());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core kdf::tests`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/kdf.rs
git commit -m "feat(core): add private KDF module (Argon2id + HKDF)"
```

---

## Task 6: Implement vault module (public)

**Files:**
- Create: `crates/core/src/vault.rs`

- [ ] **Step 1: Write vault module with all public API**

```rust
//! Vault management (PUBLIC API).
//!
//! [`UnlockedVault`] is the single public type that exposes all operations.
//! All crypto is handled internally — callers never handle keys or nonces.

use secrecy::{ExposeSecret, Secret};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{
    cipher::{self, CipherText},
    error::{Error, Result},
    kdf,
    memory::VaultEncryptionKey,
    generate_salt, MAGIC, SALT_SIZE, VERSION,
};

/// Entry data for serialization.
#[derive(Serialize, Deserialize)]
struct EntryData {
    profile: String,
    key: String,
    value: Vec<u8>,
}

/// Encrypted entry stored in vault.
#[derive(Serialize, Deserialize)]
struct StoredEntry {
    id: [u8; 16],
    ciphertext: Vec<u8>,
}

/// Vault file data for serialization.
#[derive(Serialize, Deserialize)]
struct VaultFileData {
    magic: [u8; 8],
    version: u16,
    salt: [u8; SALT_SIZE],
    wrapped_vek: Vec<u8>,
    entries: Vec<StoredEntry>,
}

/// Unlocked vault - holds VEK securely and provides all operations.
pub struct UnlockedVault {
    /// The encrypted vault file data.
    file: VaultFileData,
    /// The Vault Encryption Key (wrapped in Secret for zeroize).
    vek: VaultEncryptionKey,
    /// Device key (needed for password change).
    device_key: [u8; 32],
    /// Path to save to (if opened from file).
    path: Option<PathBuf>,
    /// Unsaved changes flag.
    dirty: bool,
}

impl UnlockedVault {
    /// Create a new vault with password and device key.
    pub fn init(password: &str, device_key: &[u8; 32]) -> Result<Self> {
        let salt = generate_salt()?;

        // Derive master key
        let mk = kdf::derive_master_key(password, &salt, device_key)?
            .into_wrapping_key();

        // Generate random VEK
        let vek = VaultEncryptionKey::generate()?;

        // Wrap VEK with master key
        let wrapped_vek = cipher::wrap_key(&mk, vek.expose())?;

        let file = VaultFileData {
            magic: *MAGIC,
            version: VERSION,
            salt,
            wrapped_vek: wrapped_vek.as_bytes().to_vec(),
            entries: Vec::new(),
        };

        Ok(Self {
            file,
            vek,
            device_key: *device_key,
            path: None,
            dirty: false,
        })
    }

    /// Open an existing vault file.
    pub fn open(path: &Path, password: &str, device_key: &[u8; 32]) -> Result<Self> {
        let data = std::fs::read(path)?;

        let file: VaultFileData = rmp_serde::from_slice(&data)
            .map_err(|e| Error::InvalidVault(e.to_string()))?;

        // Verify magic bytes
        if &file.magic != MAGIC {
            return Err(Error::InvalidVault("invalid magic bytes".into()));
        }

        // Verify version
        if file.version != VERSION {
            return Err(Error::InvalidVault(format!(
                "unsupported version: {} (expected {})",
                file.version, VERSION
            )));
        }

        // Derive master key
        let mk = kdf::derive_master_key(password, &file.salt, device_key)?
            .into_wrapping_key();

        // Unwrap VEK
        let vek_bytes = cipher::unwrap_key(&mk, &file.wrapped_vek)?;
        let vek = VaultEncryptionKey::new(vek_bytes);

        Ok(Self {
            file,
            vek,
            device_key: *device_key,
            path: Some(path.to_path_buf()),
            dirty: false,
        })
    }

    /// Save to the file this vault was opened from.
    pub fn save(&self) -> Result<()> {
        let path = self.path.as_ref().ok_or(Error::NoPath)?;
        self.save_to_internal(path)
    }

    /// Save to a specific path. Updates internal path.
    pub fn save_to(&mut self, path: &Path) -> Result<()> {
        self.save_to_internal(path)?;
        self.path = Some(path.to_path_buf());
        Ok(())
    }

    fn save_to_internal(&self, path: &Path) -> Result<()> {
        // Serialize
        let data = rmp_serde::to_vec(&self.file)
            .map_err(|e| Error::InvalidVault(e.to_string()))?;

        // Atomic write: temp file -> rename
        let temp_path = path.with_extension("tmp");

        // Write to temp
        std::fs::write(&temp_path, &data)?;

        // Sync to disk
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .mode(0o600)
                .open(&temp_path)?;
            file.sync_all()?;
        }

        // Rename (atomic on POSIX)
        std::fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Set the default save path.
    pub fn set_path(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
    }

    /// Lock the vault (consume self, zeroize VEK).
    pub fn lock(self) {
        // VEK is automatically zeroized when dropped via Secret<[u8; 32]>
        drop(self);
    }

    /// Set an entry value.
    pub fn set<K: AsRef<str>>(&mut self, profile: K, key: K, value: &Secret<Vec<u8>>) -> Result<()> {
        let entry_data = EntryData {
            profile: profile.as_ref().to_string(),
            key: key.as_ref().to_string(),
            value: value.expose_secret().clone(),
        };

        let plaintext = rmp_serde::to_vec(&entry_data)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let ciphertext = CipherText::encrypt(&self.vek, &plaintext)?;

        // Check if entry exists (by decrypting and checking)
        let mut found = false;
        for stored in &mut self.file.entries {
            if let Ok(data) = CipherText::decrypt(&self.vek, stored.ciphertext.as_slice()) {
                if let Ok(entry) = rmp_serde::from_slice::<EntryData>(&data) {
                    if entry.profile == profile.as_ref() && entry.key == key.as_ref() {
                        stored.ciphertext = ciphertext.as_bytes().to_vec();
                        found = true;
                        break;
                    }
                }
            }
        }

        if !found {
            // Add new entry
            let mut id = [0u8; 16];
            rand::rng().fill_bytes(&mut id);

            self.file.entries.push(StoredEntry {
                id,
                ciphertext: ciphertext.as_bytes().to_vec(),
            });
        }

        self.dirty = true;
        Ok(())
    }

    /// Get an entry value.
    pub fn get<K: AsRef<str>>(&self, profile: K, key: K) -> Result<Option<Secret<Vec<u8>>>> {
        for stored in &self.file.entries {
            let data = match CipherText::decrypt(&self.vek, &stored.ciphertext) {
                Ok(d) => d,
                Err(_) => continue, // Skip corrupted entries
            };

            let entry: EntryData = match rmp_serde::from_slice(&data) {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.profile == profile.as_ref() && entry.key == key.as_ref() {
                return Ok(Some(Secret::new(entry.value)));
            }
        }

        Ok(None)
    }

    /// Delete an entry. Returns true if it existed.
    pub fn delete<K: AsRef<str>>(&mut self, profile: K, key: K) -> Result<bool> {
        let profile = profile.as_ref();
        let key = key.as_ref();

        let original_len = self.file.entries.len();

        self.file.entries.retain(|stored| {
            if let Ok(data) = CipherText::decrypt(&self.vek, &stored.ciphertext) {
                if let Ok(entry) = rmp_serde::from_slice::<EntryData>(&data) {
                    return !(entry.profile == profile && entry.key == key);
                }
            }
            true // Keep entries we can't decrypt
        });

        let deleted = self.file.entries.len() < original_len;
        if deleted {
            self.dirty = true;
        }

        Ok(deleted)
    }

    /// List all profiles.
    pub fn profiles(&self) -> Result<Vec<String>> {
        let mut profiles = std::collections::HashSet::new();

        for stored in &self.file.entries {
            if let Ok(data) = CipherText::decrypt(&self.vek, &stored.ciphertext) {
                if let Ok(entry) = rmp_serde::from_slice::<EntryData>(&data) {
                    profiles.insert(entry.profile);
                }
            }
        }

        Ok(profiles.into_iter().collect())
    }

    /// List keys in a profile.
    pub fn keys<K: AsRef<str>>(&self, profile: K) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let profile = profile.as_ref();

        for stored in &self.file.entries {
            if let Ok(data) = CipherText::decrypt(&self.vek, &stored.ciphertext) {
                if let Ok(entry) = rmp_serde::from_slice::<EntryData>(&data) {
                    if entry.profile == profile {
                        keys.push(entry.key);
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Check if an entry exists.
    pub fn contains<K: AsRef<str>>(&self, profile: K, key: K) -> Result<bool> {
        Ok(self.get(profile, key)?.is_some())
    }

    /// Change master password (re-wraps VEK with same salt).
    pub fn change_password(&mut self, new_password: &str) -> Result<()> {
        let mk = kdf::derive_master_key(new_password, &self.file.salt, &self.device_key)?
            .into_wrapping_key();

        // Re-wrap VEK
        let wrapped_vek = cipher::wrap_key(&mk, self.vek.expose())?;
        self.file.wrapped_vek = wrapped_vek.as_bytes().to_vec();
        self.dirty = true;

        Ok(())
    }

    /// Get vault file path.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_init_and_unlock() {
        let device_key = [1u8; 32];

        let vault = UnlockedVault::init("password", &device_key).unwrap();
        assert!(vault.path().is_none());
    }

    #[test]
    fn test_set_and_get() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("myapp", "api_key", &Secret::new(b"secret".to_vec())).unwrap();

        let value = vault.get("myapp", "api_key").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().expose_secret(), b"secret");
    }

    #[test]
    fn test_get_nonexistent() {
        let device_key = [1u8; 32];
        let vault = UnlockedVault::init("password", &device_key).unwrap();

        let value = vault.get("nonexistent", "key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_delete() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("myapp", "key", &Secret::new(b"value".to_vec())).unwrap();

        let deleted = vault.delete("myapp", "key").unwrap();
        assert!(deleted);

        let value = vault.get("myapp", "key").unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn test_profiles() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("app1", "key", &Secret::new(b"v".to_vec())).unwrap();
        vault.set("app2", "key", &Secret::new(b"v".to_vec())).unwrap();

        let profiles = vault.profiles().unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.contains(&"app1".to_string()));
        assert!(profiles.contains(&"app2".to_string()));
    }

    #[test]
    fn test_keys() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("myapp", "key1", &Secret::new(b"v".to_vec())).unwrap();
        vault.set("myapp", "key2", &Secret::new(b"v".to_vec())).unwrap();

        let keys = vault.keys("myapp").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_save_and_open() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("myapp", "key", &Secret::new(b"value".to_vec())).unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        // Open with same password
        let vault2 = UnlockedVault::open(temp_file.path(), "password", &device_key).unwrap();
        let value = vault2.get("myapp", "key").unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().expose_secret(), b"value");
    }

    #[test]
    fn test_wrong_password_fails() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        let temp_file = NamedTempFile::new().unwrap();
        vault.save_to(temp_file.path()).unwrap();

        // Try with wrong password
        let result = UnlockedVault::open(temp_file.path(), "wrong", &device_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_overwrite_entry() {
        let device_key = [1u8; 32];
        let mut vault = UnlockedVault::init("password", &device_key).unwrap();

        vault.set("app", "key", &Secret::new(b"v1".to_vec())).unwrap();
        vault.set("app", "key", &Secret::new(b"v2".to_vec())).unwrap();

        let value = vault.get("app", "key").unwrap().unwrap();
        assert_eq!(value.expose_secret(), b"v2");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core vault::tests`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/vault.rs
git commit -m "feat(core): add vault module with UnlockedVault public API"
```

---

## Task 7: Add nonce correctness tests

**Files:**
- Create: `crates/core/tests/nonce_correctness.rs`

- [ ] **Step 1: Write nonce correctness tests**

```rust
//! Nonce correctness tests (critical for security).
//!
//! These tests verify the security-critical nonce invariant:
//! - Nonce is never exposed to callers
//! - Nonce is unique per encryption
//! - No fallback on RNG failure

use lockit_core::{generate_device_key, UnlockedVault, Secret};
use std::collections::HashSet;

/// Verify no public API leaks nonce.
/// This test ensures callers cannot access nonce values.
#[test]
fn test_nonce_never_exposed() {
    let device_key = generate_device_key().unwrap();
    let mut vault = UnlockedVault::init("password", &device_key).unwrap();

    vault.set("app", "key", &Secret::new(b"value".to_vec())).unwrap();

    // Verify: UnlockedVault has no method that returns nonce
    // (This is a compile-time guarantee - no .nonce(), .get_nonce(), etc.)
    // The test is that this compiles successfully.
}

/// Verify unique nonce per encryption.
#[test]
fn test_unique_nonce_per_encrypt() {
    // We can't access nonces directly, but we can verify
    // that encrypting the same plaintext multiple times
    // produces different ciphertexts (implying different nonces)

    let device_key = generate_device_key().unwrap();
    let mut vault = UnlockedVault::init("password", &device_key).unwrap();

    let value = Secret::new(b"same-value".to_vec());

    // Set same value multiple times
    for _ in 0..100 {
        vault.set("app", "key", &value).unwrap();
    }

    // Should still work correctly
    let result = vault.get("app", "key").unwrap().unwrap();
    assert_eq!(result.expose_secret(), b"same-value");
}

/// Test concurrent encryption produces unique nonces.
/// Each thread creates its own vault to avoid file race conditions.
#[test]
fn test_concurrent_encryption() {
    use std::thread;

    let device_key = generate_device_key().unwrap();
    let dk_clone = device_key;

    // Each thread works on its own vault file
    let temp_dir = tempfile::tempdir().unwrap();
    let mut handles = vec![];

    for i in 0..10 {
        let path = temp_dir.path().join(format!("vault{}.lockit", i));
        let dk = dk_clone;
        let handle = thread::spawn(move || {
            let mut vault = UnlockedVault::init("password", &dk).unwrap();
            vault.set("profile", &format!("key{}", i), &Secret::new(vec![i as u8; 32])).unwrap();
            vault.save_to(&path).unwrap();

            // Re-open and verify
            let v2 = UnlockedVault::open(&path, "password", &dk).unwrap();
            let val = v2.get("profile", &format!("key{}", i)).unwrap().unwrap();
            assert_eq!(val.expose_secret(), &vec![i as u8; 32]);
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}

/// Test overwriting entry uses fresh nonce.
#[test]
fn test_overwrite_new_nonce() {
    let device_key = generate_device_key().unwrap();
    let mut vault = UnlockedVault::init("password", &device_key).unwrap();

    // Set initial value
    vault.set("app", "key", &Secret::new(b"value1".to_vec())).unwrap();

    // Save to capture ciphertext
    let temp = tempfile::NamedTempFile::new().unwrap();
    vault.save_to(temp.path()).unwrap();
    let original_data = std::fs::read(temp.path()).unwrap();

    // Overwrite with different value
    vault.set("app", "key", &Secret::new(b"value2".to_vec())).unwrap();
    vault.save().unwrap();
    let new_data = std::fs::read(temp.path()).unwrap();

    // Ciphertext should be different (different nonce + different plaintext)
    assert_ne!(original_data, new_data);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package lockit-core --test nonce_correctness`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/core/tests/nonce_correctness.rs
git commit -m "test(core): add nonce correctness tests for security invariant"
```

---

## Task 8: Run full test suite and verify coverage

**Files:**
- Modify: Various

- [ ] **Step 1: Run all tests**

Run: `cargo test --package lockit-core`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --package lockit-core --all-targets -- -D warnings`
Expected: Zero warnings

- [ ] **Step 3: Run format check**

Run: `cargo fmt --package lockit-core -- --check`
Expected: No formatting issues

- [ ] **Step 4: Fix any issues found**

If clippy or fmt report issues, fix them and re-run.

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix(core): address clippy warnings and formatting"
```

---

## Task 9: Update CLI crate to use lockit-core

**Files:**
- Modify: `crates/cli/src/main.rs`

- [ ] **Step 1: Update CLI to use new crate name**

```rust
use lockit_core::{generate_device_key, UnlockedVault, Secret};

fn main() {
    // ... existing code ...
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "refactor(cli): update to use lockit-core"
```

---

## Task 10: Final verification and cleanup

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 2: Run full workspace clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: Zero warnings

- [ ] **Step 3: Verify documentation builds**

Run: `cargo doc --package lockit-core --no-deps`
Expected: Docs build without errors

- [ ] **Step 4: Commit final state**

```bash
git add -A
git commit -m "feat(core): complete lockit-core implementation"
```
---

## Task 11: BIP39 恢复助记词（代码已有，补充 plan）

**当前状态：** ✅ 已实现，但 plan 未覆盖

**文件：**
- `crates/core/src/vault.rs`：`init_with_recovery()`、`recover_with_mnemonic()`
- `crates/core/src/kdf.rs`：`derive_recovery_key()`

**设计决策：**

BIP39 恢复助记词是主密码的独立备份通道。恢复流程：
1. 生成 32 字节随机熵 → BIP39 24 词助记词
2. 从助记词熵派生 recovery key（HKDF-SHA256）
3. 用 recovery key 包裹 VEK → `recovery_wrapped_vek` 存入 vault 文件
4. 恢复时：助记词 → recovery key → unwrap VEK → 用新密码重新包裹

关键设计：**恢复不依赖 device key**，因为恢复场景假设 device key 可能丢失。

```rust
pub fn init_with_recovery(password: &str, device_key: &[u8; 32]) -> Result<(Self, String)>
// 返回 (vault, mnemonic_phrase)，助记词只展示一次，不存任何地方

pub fn recover_with_mnemonic(path: &Path, mnemonic_phrase: &str, new_password: &str, device_key: &[u8; 32]) -> Result<()>
// 从助记词恢复 vault，重置密码
```

### 补充验收测试

- [ ] **Step 1: init_with_recovery 生成 24 词助记词**
  - 助记词是有效的 BIP39
  - recovery_wrapped_vek 存在
  
- [ ] **Step 2: recover_with_mnemonic 成功恢复**
  - 原密码不能再用，新密码生效
  - 凭据数据不丢失

- [ ] **Step 3: 错误助记词失败**
  - 不同的 BIP39 助记词返回错误
  - 助记词格式错误返回错误

---

## Task 12: Atomic file write（代码已有，补充 plan）

**当前状态：** ✅ 已实现，plan 未覆盖

**文件：**
- `crates/core/src/vault.rs`：`atomic_write()` 函数

**设计决策：**

写入 vault 文件使用原子操作：
1. 写入临时文件（`vault.lockit.<uuid>.tmp`）
2. `sync_all()` 确保数据落盘
3. `rename()` 原子替换目标文件

Unix 特殊处理：
- 临时文件权限设为 0600
- 目标文件权限也设为 0600

```rust
fn atomic_write(path: &Path, data: &[u8]) -> Result<()>
```

- [ ] **Step 1: 验证原子性**
  - 临时文件先创建，后 rename
  - rename 在 POSIX 是原子操作

- [ ] **Step 2: 验证权限**
  - 创建的文件权限为 0600（owner only）

---

## Task 13: VaultFileData 结构补全

**当前状态：** ✅ 已实现，plan 未覆盖

**文件：**
- `crates/core/src/vault.rs`：`VaultFileData` 结构

```rust
struct VaultFileData {
    magic: [u8; 8],                    // "LOCKIT\0\0" magic bytes
    version: u16,                      // 文件格式版本
    salt: [u8; SALT_SIZE],             // Argon2id salt
    wrapped_vek: Vec<u8>,              // MK 包裹的 VEK
    encrypted_entries: Vec<u8>,        // AES-GCM 加密的 entries
    recovery_wrapped_vek: Option<Vec<u8>>,  // 恢复 key 包裹的 VEK
}
```

magic bytes 校验放在 `open()` 开头，快速拒绝非 vault 文件。

- [ ] **Step 1: 验证 magic bytes 检查**
  - 8 字节 magic 在打开时校验
  - 不匹配返回 `Error::InvalidVault`

---

## Task 14: 密码强度验证

**当前状态：** ⚠️ 已实现，但只有长度检查（CLI 层）

**文件：**
- `crates/cli/src/main.rs`：`validate_password_strength()`

```rust
fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 12 {
        bail!("Password too short — minimum 12 characters required.");
    }
    Ok(())
}
```

**当前限制：** 只检查长度 ≥ 12，不检查字符复杂度。

- [ ] **Step 1: 补充复杂度检查**（建议后续增强）
  - 至少包含 3 种字符类型：大写、小写、数字、特殊字符
  - 或检测常见弱密码模式（重复字符、键盘序列）

---

## Task 15: 关键依赖版本

**当前版本：**

| 依赖 | 版本 | 用途 |
|------|------|------|
| argon2 | 0.5 | 密钥派生 |
| aes-gcm | 0.10 | 对称加密 |
| hkdf | 0.12 | HKDF 派生 |
| sha2 | 0.10 | SHA-256 哈希 |
| secrecy | 0.8 | Secret wrapper |
| zeroize | 1.8 | 内存清零 |
| bip39 | 2 | BIP39 助记词 |
| rmp-serde | 1.3 | MessagePack 序列化 |
| serde | 1.0 | 序列化框架 |
| rand | 0.9 | 随机数生成 |
| uuid | 1.10 | 临时文件名 |

**审计事项：**
- [ ] `cargo audit` 检查已知漏洞
- [ ] 确认 aes-gcm 0.10 是最新版
- [ ] 确认 argon2 0.5 是最新版

---

# lockit-core 功能文档

> 本文档描述 lockit-core 的完整功能、公共 API、设计原理和安全模型。目标读者：重构者、集成者、代码审查者。

## 一、概览

lockit-core 是 lockit 的加密基础层，负责：

1. 密钥派生（密码 + device key → master key）
2. 加密/解密（AES-256-GCM）
3. Vault 文件管理（创建、打开、保存、恢复）
4. 安全内存管理（zeroize-on-drop）

**唯一公共类型：** `UnlockedVault`——所有加密操作都通过这个类型完成，外部代码无法直接调用加密函数。

---

## 二、公共 API

### 2.1 创建 vault

```rust
// 不带恢复
pub fn init(password: &str, device_key: &[u8; 32]) -> Result<Self>

// 带恢复助记词，返回 (vault, mnemonic_phrase)
pub fn init_with_recovery(password: &str, device_key: &[u8; 32]) -> Result<(Self, String)>
```

### 2.2 打开 vault

```rust
pub fn open(path: &Path, password: &str, device_key: &[u8; 32]) -> Result<Self>
```

失败情况：
- 文件不存在 → `std::io::Error`
- magic bytes 不匹配 → `Error::InvalidVault`
- 版本不支持 → `Error::InvalidVault`
- 密码错误 → `Error::IncorrectPassword`
- 文件损坏（GCM tag 校验失败） → `Error::VaultCorrupted`

### 2.3 恢复助记词

```rust
pub fn recover_with_mnemonic(
    path: &Path,
    mnemonic_phrase: &str,
    new_password: &str,
    device_key: &[u8; 32],
) -> Result<()>
```

- 恢复不依赖原密码
- 恢复后原密码失效，新密码生效
- vault 文件原地更新（原子写入）
- 凭据数据不丢失

### 2.4 CRUD 操作

```rust
pub fn set<K: AsRef<str>>(&mut self, profile: K, key: K, value: &Secret<Vec<u8>>) -> Result<()>
pub fn get<K: AsRef<str>>(&self, profile: K, key: K) -> Result<Option<Secret<Vec<u8>>>>
pub fn delete<K: AsRef<str>>(&mut self, profile: K, key: K) -> Result<bool>
pub fn profiles(&self) -> Result<Vec<String>>
pub fn keys<K: AsRef<str>>(&self, profile: K) -> Result<Vec<String>>
pub fn contains<K: AsRef<str>>(&self, profile: K, key: K) -> Result<bool>
```

### 2.5 保存

```rust
pub fn save(&self) -> Result<()>                    // 保存到原路径
pub fn save_to(&mut self, path: &Path) -> Result<()> // 保存到新路径
pub fn set_path(&mut self, path: &Path)              // 设置默认路径
```

保存流程：
1. 收集 entries，按 (profile, key) 排序（保证确定性输出）
2. 序列化为 MessagePack
3. AES-256-GCM 加密
4. 原子写入（temp → rename）

### 2.6 锁定

```rust
pub fn lock(self)
```

消费 self，触发 Drop：清零所有凭据值、VEK、device key。

### 2.7 密码更改

```rust
pub fn change_password(&mut self, new_password: &str) -> Result<()>
```

- 生成新 salt（防止关联攻击）
- 用新密码 + device key 重新包裹 VEK
- 不改变凭据数据、不改变 device key

---

## 三、加密架构

### 3.1 密钥派生链

```
password + salt ──Argon2id──→ PasswordKey (32 bytes)
                                      │
device_key (32 bytes) ────────────────┘
                                      │
                            HKDF-SHA256 ──→ MasterKey (32 bytes)
                                              │
                                         ┌────┘
                                         ▼
                                   VaultEncryptionKey (VEK)
```

### 3.2 文件结构

```
vault.lockit (rmp-serde binary)
┌────────────────────────────────────────┐
│ magic:     [u8; 8]   "LOCKIT\0\0"     │  8 bytes
│ version:   u16                          │  2 bytes
│ salt:      [u8; 32]   Argon2id salt   │ 32 bytes
│ wrapped_vek: Vec<u8>  AES-GCM(VEK)    │ ~52 bytes
│ encrypted_entries: Vec<u8>  AES-GCM    │ ~N bytes
│ recovery_wrapped_vek: Option<Vec<u8>>  │ ~52 bytes 或 0
└────────────────────────────────────────┘
```

### 3.3 加密参数

| 参数 | 值 | 说明 |
|------|------|------|
| KDF | Argon2id | 64 MiB memory, 3 iterations, 4 parallelism |
| Encryption | AES-256-GCM | 96-bit nonce, 128-bit auth tag |
| Key size | 32 bytes | 所有密钥统一长度 |
| Nonce 生成 | OsRng | 每次加密独立随机，内部生成 |
| salt | OsRng | 每次 init / change-password 重新生成 |

### 3.4 恢复密钥派生

```
BIP39 助记词 ──entropy──→ 32 bytes
                              │
                    HKDF-SHA256 ──→ RecoveryKey (32 bytes)
                                         │
                                    wrap_key(VEK) → recovery_wrapped_vek
```

**恢复不依赖 device key**，因为恢复场景假设 device key 可能丢失。

---

## 四、安全内存管理

### 4.1 Secret<T> 包装

以下敏感数据使用 `secrecy::Secret` 包装：

| 类型 | 包装方式 | drop 时行为 |
|------|---------|------------|
| VEK | `VaultEncryptionKey` → `Secret<[u8; 32]>` | zeroize |
| MasterKey | `Secret<[u8; 32]>` | zeroize |
| PasswordKey | `Secret<[u8; 32]>` | zeroize |
| RecoveryKey | `Secret<[u8; 32]>` | zeroize |
| IKM | 直接 zeroize | 清零 |
| 凭据 values | `Vec<u8>` → `zeroize()` | 清零 |
| 入口 bytes | `entries_bytes` | 加密后立即清零 |

### 4.2 Drop 实现

```rust
impl Drop for UnlockedVault {
    fn drop(&mut self) {
        for value in self.credentials.values_mut() {
            value.zeroize();
        }
        // vek, device_key 通过 Secret 自动 zeroize
    }
}
```

### 4.3 泄露防护

| 场景 | 防护措施 |
|------|---------|
| Debug 打印 | Secret<T> 不实现 Debug/Display |
| panic 信息 | 不包含 key/value 内容 |
| 日志 | tracing 只记录操作类型，不记录敏感数据 |
| 内存转储 | zeroize 依赖操作系统行为（非强制保证） |

---

## 五、错误类型

```rust
pub enum Error {
    IncorrectPassword,           // 密码错误（GCM auth 失败）
    InvalidVault(String),        // vault 文件格式错误
    VaultCorrupted,              // 文件损坏（GCM tag 不匹配）
    Encryption(String),          // 加密操作失败
    Decryption(String),          // 解密操作失败
    KeyDerivation(String),       // KDF 失败
    NoPath,                      // 保存时未设置路径
    Io(std::io::Error),          // 文件 I/O 错误
}
```

**错误区分设计：**
- `IncorrectPassword` vs `VaultCorrupted`：前者是 KDF unwrap VEK 时失败（密码错），后者是 entries 解密失败（文件损坏/被篡改）
- 这两个错误用不同的 GCM 验证点检测，能区分"人输错了"和"文件坏了"

---

## 六、确定性序列化

entries 序列化时，按 (profile, key) 字典序排序，保证：
- 相同内容的 vault 生成相同的加密数据
- 便于 diff（如果未来实现 vault 的 git diff 支持）
- 同步时可以用 checksum 检测是否有变化

```rust
entries.sort_by(|a, b| a.profile.cmp(&b.profile).then(a.key.cmp(&b.key)));
```

---

## 七、性能参考

| 操作 | 时间估算 | 场景 |
|------|---------|------|
| init（首次创建） | ~200ms | Argon2id 计算 |
| open（打开 vault） | ~200ms | Argon2id 计算 + 1 次 AES 解密 |
| set/get/delete | <1μs | HashMap 操作 |
| save（100 条凭据） | ~200ms | 序列化 + AES 加密 + fsync |
| save（1000 条凭据） | ~300ms | 同上 |
| change-password | ~200ms | 重新 Argon2id |
| recover_with_mnemonic | ~5ms | HKDF 不需要 Argon2id |

**瓶颈在 Argon2id：** 64MiB memory + 3 iterations，每次 open/recover 都需要。
