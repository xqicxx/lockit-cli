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

    /// Incorrect master password.
    #[error("Incorrect password")]
    IncorrectPassword,

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
        let err = Error::InvalidKeySize {
            expected: 32,
            actual: 16,
        };
        assert_eq!(
            err.to_string(),
            "Invalid key size: expected 32 bytes, got 16"
        );
    }

    #[test]
    fn test_vault_corrupted_display() {
        let err = Error::VaultCorrupted;
        assert_eq!(err.to_string(), "Vault file corrupted or tampered");
    }
}
