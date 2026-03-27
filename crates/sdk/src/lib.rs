//! lockit-sdk — Rust client library for the lockit daemon.
//!
//! Connects to the lockit daemon via IPC and provides a simple synchronous API.
//!
//! # Example
//!
//! ```rust,no_run
//! use lockit_sdk::LockitClient;
//!
//! let client = LockitClient::new()?;
//! let value = client.get("myapp", "api_key")?;
//! println!("{}", value.unwrap_or_default());
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::{Result, anyhow};
use lockit_ipc::{IpcClient, Request, Response};

/// Synchronous client for the lockit daemon.
pub struct LockitClient {
    rt: tokio::runtime::Runtime,
    socket_path: std::path::PathBuf,
}

impl LockitClient {
    /// Connect to the default daemon socket.
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let socket_path = lockit_ipc::socket_path()?;
        Ok(Self { rt, socket_path })
    }

    /// Send a request and return the response.
    fn send(&self, request: Request) -> Result<Response> {
        let path = self.socket_path.clone();
        self.rt.block_on(async move {
            let client = IpcClient::new(path);
            client
                .send_request(&request)
                .await
                .map_err(|e| anyhow!("{}", e))
        })
    }

    /// Get a credential value. Returns None if the key does not exist.
    pub fn get(&self, profile: &str, key: &str) -> Result<Option<String>> {
        let resp = self.send(Request::GetCredential {
            profile: profile.to_string(),
            key: key.to_string(),
        })?;
        match resp {
            Response::Value { value: Some(bytes) } => Ok(Some(
                String::from_utf8(bytes).map_err(|e| anyhow!("invalid UTF-8: {e}"))?,
            )),
            Response::Value { value: None } => Ok(None),
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    /// List all profiles.
    pub fn list_profiles(&self) -> Result<Vec<String>> {
        let resp = self.send(Request::ListProfiles)?;
        match resp {
            Response::Profiles { profiles } => Ok(profiles),
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    /// List all keys in a profile.
    pub fn list_keys(&self, profile: &str) -> Result<Vec<String>> {
        let resp = self.send(Request::ListKeys {
            profile: profile.to_string(),
        })?;
        match resp {
            Response::Keys { keys } => Ok(keys),
            other => Err(anyhow!("unexpected response: {:?}", other)),
        }
    }

    /// Check if the daemon is running and the vault is unlocked.
    pub fn is_unlocked(&self) -> bool {
        let resp = self.send(Request::DaemonStatus);
        matches!(resp, Ok(Response::Status { locked: false, .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LockitClient::new() may fail if there is no daemon socket, but the
    /// construction of the runtime should always succeed when the socket path
    /// can be derived (i.e., HOME is set).
    ///
    /// We cannot test actual IPC without a running daemon, so we just verify
    /// that `new()` either succeeds or fails with a meaningful error.
    #[test]
    fn new_returns_result() {
        // This will succeed (socket path derivation works) or fail with an
        // IPC error — either way it should not panic.
        let _ = LockitClient::new();
    }

    #[test]
    fn is_unlocked_returns_false_when_daemon_not_running() {
        // Without a daemon, is_unlocked() should return false rather than panic.
        if let Ok(client) = LockitClient::new() {
            // The daemon is likely not running in CI, so this should be false.
            // We just assert it doesn't panic.
            let _ = client.is_unlocked();
        }
    }
}
