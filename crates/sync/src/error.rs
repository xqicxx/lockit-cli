//! Error types for lockit-sync.

use thiserror::Error;

/// Result type alias for lockit-sync operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for lockit-sync.
#[derive(Debug, Error)]
pub enum Error {
    /// Object not found in the backend.
    #[error("Object not found: {key}")]
    NotFound { key: String },

    /// Upload operation failed.
    #[error("Upload failed for '{key}': {reason}")]
    Upload { key: String, reason: String },

    /// Download operation failed.
    #[error("Download failed for '{key}': {reason}")]
    Download { key: String, reason: String },

    /// List operation failed.
    #[error("List failed for prefix '{prefix}': {reason}")]
    List { prefix: String, reason: String },

    /// Delete operation failed.
    #[error("Delete failed for '{key}': {reason}")]
    Delete { key: String, reason: String },

    /// Metadata retrieval failed.
    #[error("Metadata failed for '{key}': {reason}")]
    Metadata { key: String, reason: String },

    /// Backend configuration is invalid.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Key contains path components that could escape the base directory.
    #[error("Invalid key '{key}': {reason}")]
    InvalidKey { key: String, reason: String },

    /// Content checksum does not match expected value.
    #[error("Checksum mismatch for '{key}': expected {expected}, got {actual}")]
    ChecksumMismatch {
        key: String,
        expected: String,
        actual: String,
    },

    /// The requested backend type is not implemented yet.
    #[error("Backend '{0}' is not implemented")]
    NotImplemented(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
