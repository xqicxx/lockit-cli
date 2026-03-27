//! Error types for lockit-ipc.

use thiserror::Error;

use crate::proto::ErrorKind;

/// Result type alias for lockit-ipc operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for lockit-ipc.
#[derive(Debug, Error)]
pub enum Error {
    /// Underlying socket or I/O error.
    #[error("Socket error: {0}")]
    Socket(#[from] std::io::Error),

    /// Failed to serialize a message to MessagePack.
    #[error("Serialization error: {0}")]
    Serialize(String),

    /// Failed to deserialize a message from MessagePack.
    #[error("Deserialization error: {0}")]
    Deserialize(String),

    /// Incoming frame length exceeded the configured maximum.
    #[error("Frame too large: {size} bytes (max {max})")]
    FrameTooLarge { size: u32, max: u32 },

    /// Operation timed out.
    #[error("Timeout after {millis}ms")]
    Timeout { millis: u64 },

    /// The daemon returned an application-level error.
    #[error("IPC error ({kind:?}): {message}")]
    IpcError { kind: ErrorKind, message: String },

    /// The connection was closed before a full response was received.
    #[error("Connection closed unexpectedly")]
    ConnectionClosed,

    /// Feature not supported on the current platform.
    #[error("Not implemented on this platform")]
    NotImplemented,
}
