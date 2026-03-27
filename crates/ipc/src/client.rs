//! IPC client — connect to the daemon socket and send requests.

use std::path::PathBuf;
use std::time::Duration;

use tokio::net::UnixStream;
use tokio::time::timeout;
use tracing::debug;

use crate::error::{Error, Result};
use crate::framing;
use crate::proto::{Request, Response};

/// Default per-operation timeout in milliseconds (5 seconds).
pub const DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// IPC client that connects to the lockit daemon socket.
///
/// Each call to [`IpcClient::send_request`] opens a new connection, sends one
/// request, reads one response, and closes the connection.  This is
/// intentionally simple — Unix Domain Socket connect/disconnect overhead is
/// well under 100 µs locally, and a credential manager performs at most a few
/// requests per second.
///
/// # Example
///
/// ```rust,ignore
/// let client = IpcClient::new_default()?;
/// let response = client.send_request(&Request::DaemonStatus).await?;
/// ```
pub struct IpcClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl IpcClient {
    /// Create a client targeting the given socket path.
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        }
    }

    /// Create a client using the platform default socket path.
    pub fn new_default() -> Result<Self> {
        Ok(Self::new(crate::socket::socket_path()?))
    }

    /// Override the per-operation timeout.
    pub fn with_timeout(mut self, millis: u64) -> Self {
        self.timeout = Duration::from_millis(millis);
        self
    }

    /// Send a single request and return the decoded response.
    ///
    /// Returns `Err(Error::IpcError { .. })` when the daemon replies with a
    /// `Response::Error`, so callers never observe `Ok(Response::Error{..})`.
    ///
    /// Returns `Err(Error::Timeout { .. })` if connect, write, or read exceeds
    /// the configured timeout.
    pub async fn send_request(&self, request: &Request) -> Result<Response> {
        // ── Connect ──────────────────────────────────────────────────────
        let mut stream = timeout(self.timeout, UnixStream::connect(&self.socket_path))
            .await
            .map_err(|_| Error::Timeout {
                millis: self.timeout.as_millis() as u64,
            })?
            .map_err(Error::Socket)?;

        let (mut reader, mut writer) = tokio::io::split(&mut stream);

        // ── Write request ────────────────────────────────────────────────
        timeout(self.timeout, framing::write_message(&mut writer, request))
            .await
            .map_err(|_| Error::Timeout {
                millis: self.timeout.as_millis() as u64,
            })??;

        // ── Read response ────────────────────────────────────────────────
        let response: Response = timeout(self.timeout, framing::read_message(&mut reader))
            .await
            .map_err(|_| Error::Timeout {
                millis: self.timeout.as_millis() as u64,
            })??;

        debug!(?response, "received IPC response");

        // Transparently surface application-level errors as `Err`.
        if let Response::Error { kind, message } = response {
            return Err(Error::IpcError { kind, message });
        }

        Ok(response)
    }
}
