//! IPC protocol message types.
//!
//! All types are serialized/deserialized with `rmp-serde` (named MessagePack).
//! The `#[serde(tag = "type", rename_all = "snake_case")]` convention produces
//! a msgpack map containing a `"type"` key that identifies the variant — this
//! is forward-compatible and works cleanly with `rmp_serde::to_vec_named`.

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// A zeroize-on-drop password field for use in IPC messages.
///
/// - `Debug` output is redacted so passwords never appear in logs.
/// - The inner `String` is overwritten with zeros when dropped.
/// - Serializes/deserializes transparently as a plain string so the wire
///   format is unchanged.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Password(String);

impl Password {
    /// Wrap a `String` as a `Password`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Access the password value.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Zeroize for Password {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Current IPC protocol version.  Callers that see a different version from
/// the server should surface a clear error rather than silently misbehaving.
pub const PROTOCOL_VERSION: u16 = 1;

/// Requests sent from a client to the daemon.
///
/// Note: `PartialEq` / `Eq` are intentionally omitted — `Secret<String>` does
/// not implement those traits to prevent accidental plaintext comparisons.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Unlock the vault with the given master password and device key.
    UnlockVault {
        /// Master password (UTF-8), wrapped in [`Password`] so it is zeroized
        /// on drop and omitted from `Debug` output.
        password: Password,
        /// 32-byte device key (raw bytes).
        device_key: Vec<u8>,
    },

    /// Unlock the vault using a password retrieved from the OS biometric
    /// store (Touch ID / Face ID on macOS).  The `password` field contains
    /// the vault password that the CLI retrieved from Keychain after a
    /// successful biometric challenge; the daemon treats it identically to
    /// a regular `UnlockVault` request.
    ///
    /// The `biometric_source` field is informational (e.g. `"touchid"`) and
    /// used only for audit logging — it does not affect vault decryption.
    UnlockWithBiometric {
        /// Vault password obtained from the biometric-protected Keychain item.
        password: Password,
        /// 32-byte device key.
        device_key: Vec<u8>,
        /// Human-readable biometric source name for audit logging.
        biometric_source: String,
    },

    /// Lock the vault, clearing the in-memory VEK.
    LockVault,

    /// Retrieve a single credential value.
    GetCredential { profile: String, key: String },

    /// Create or update a credential value.
    SetCredential {
        profile: String,
        key: String,
        /// Raw credential bytes.
        value: Vec<u8>,
    },

    /// Delete a credential key.
    DeleteCredential { profile: String, key: String },

    /// List all profile names stored in the vault.
    ListProfiles,

    /// List all keys within a profile.
    ListKeys { profile: String },

    /// Query daemon status without modifying vault state.
    DaemonStatus,
}

/// Responses sent from the daemon to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// Generic success (no payload needed).
    Ok,

    /// A single credential value (`None` = key not found).
    Value { value: Option<Vec<u8>> },

    /// List of profile names.
    Profiles { profiles: Vec<String> },

    /// List of keys within a profile.
    Keys { keys: Vec<String> },

    /// Current daemon status.
    Status {
        locked: bool,
        version: String,
        uptime_secs: u64,
    },

    /// Application-level error.
    Error { kind: ErrorKind, message: String },
}

/// Structured error codes for application-level errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Vault is locked; caller must send `UnlockVault` first.
    VaultLocked,
    /// The requested profile or key was not found.
    NotFound,
    /// Wrong password or device key.
    IncorrectPassword,
    /// The caller does not have permission to perform this operation.
    PermissionDenied,
    /// Unrecoverable internal daemon error.
    Internal,
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip a value through `rmp_serde::to_vec_named` → `from_slice`,
    /// checking equality.  Only usable for types that implement `PartialEq`.
    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
        value: &T,
    ) {
        let bytes = rmp_serde::to_vec_named(value).expect("serialize");
        let decoded: T = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert_eq!(*value, decoded);
    }

    /// Serialize then deserialize without requiring `PartialEq`.  Used for
    /// `Request` which contains `Secret<String>` fields that intentionally
    /// do not implement `PartialEq`.
    fn serde_roundtrip<T: Serialize + for<'de> Deserialize<'de>>(value: &T) -> T {
        let bytes = rmp_serde::to_vec_named(value).expect("serialize");
        rmp_serde::from_slice(&bytes).expect("deserialize")
    }

    // ── Request variants ──────────────────────────────────────────────────

    #[test]
    fn request_unlock_vault_roundtrip() {
        let req = Request::UnlockVault {
            password: Password::new("hunter2"),
            device_key: vec![0u8; 32],
        };
        let decoded = serde_roundtrip(&req);
        if let Request::UnlockVault {
            password,
            device_key,
        } = decoded
        {
            assert_eq!(password.expose(), "hunter2");
            assert_eq!(device_key, vec![0u8; 32]);
        } else {
            panic!("wrong variant after round-trip");
        }
    }

    #[test]
    fn request_unlock_with_biometric_roundtrip() {
        let req = Request::UnlockWithBiometric {
            password: Password::new("from_keychain"),
            device_key: vec![0u8; 32],
            biometric_source: "touchid".into(),
        };
        let decoded = serde_roundtrip(&req);
        if let Request::UnlockWithBiometric {
            password,
            device_key,
            biometric_source,
        } = decoded
        {
            assert_eq!(password.expose(), "from_keychain");
            assert_eq!(device_key, vec![0u8; 32]);
            assert_eq!(biometric_source, "touchid");
        } else {
            panic!("wrong variant after round-trip");
        }
    }

    #[test]
    fn request_lock_vault_roundtrip() {
        serde_roundtrip(&Request::LockVault);
    }

    #[test]
    fn request_get_credential_roundtrip() {
        serde_roundtrip(&Request::GetCredential {
            profile: "github".into(),
            key: "token".into(),
        });
    }

    #[test]
    fn request_set_credential_roundtrip() {
        serde_roundtrip(&Request::SetCredential {
            profile: "github".into(),
            key: "token".into(),
            value: b"ghp_secret".to_vec(),
        });
    }

    #[test]
    fn request_delete_credential_roundtrip() {
        serde_roundtrip(&Request::DeleteCredential {
            profile: "p".into(),
            key: "k".into(),
        });
    }

    #[test]
    fn request_list_profiles_roundtrip() {
        serde_roundtrip(&Request::ListProfiles);
    }

    #[test]
    fn request_list_keys_roundtrip() {
        serde_roundtrip(&Request::ListKeys {
            profile: "myapp".into(),
        });
    }

    #[test]
    fn request_daemon_status_roundtrip() {
        serde_roundtrip(&Request::DaemonStatus);
    }

    // ── Response variants ─────────────────────────────────────────────────

    #[test]
    fn response_ok_roundtrip() {
        roundtrip(&Response::Ok);
    }

    #[test]
    fn response_value_some_roundtrip() {
        roundtrip(&Response::Value {
            value: Some(b"secret".to_vec()),
        });
    }

    #[test]
    fn response_value_none_roundtrip() {
        roundtrip(&Response::Value { value: None });
    }

    #[test]
    fn response_profiles_roundtrip() {
        roundtrip(&Response::Profiles {
            profiles: vec!["github".into(), "aws".into()],
        });
    }

    #[test]
    fn response_keys_roundtrip() {
        roundtrip(&Response::Keys {
            keys: vec!["token".into(), "secret".into()],
        });
    }

    #[test]
    fn response_status_roundtrip() {
        roundtrip(&Response::Status {
            locked: false,
            version: "0.1.0".into(),
            uptime_secs: 42,
        });
    }

    #[test]
    fn response_error_roundtrip() {
        roundtrip(&Response::Error {
            kind: ErrorKind::VaultLocked,
            message: "vault is locked".into(),
        });
    }

    // ── ErrorKind variants ────────────────────────────────────────────────

    #[test]
    fn error_kind_all_variants_roundtrip() {
        for kind in [
            ErrorKind::VaultLocked,
            ErrorKind::NotFound,
            ErrorKind::IncorrectPassword,
            ErrorKind::PermissionDenied,
            ErrorKind::Internal,
        ] {
            roundtrip(&kind);
        }
    }

    #[test]
    fn serialized_bytes_are_non_empty() {
        let bytes = rmp_serde::to_vec_named(&Request::DaemonStatus).unwrap();
        assert!(!bytes.is_empty());
    }
}
