//! IPC server — bind a Unix Domain Socket and dispatch requests to a handler.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::framing;
use crate::proto::{Request, Response};

/// Trait implemented by the daemon to handle decoded IPC requests.
///
/// Separating request handling from transport allows integration tests to
/// inject a lightweight mock without needing a real vault.
///
/// # Example
///
/// ```rust,ignore
/// struct EchoHandler;
///
/// impl RequestHandler for EchoHandler {
///     async fn handle(&self, _req: Request) -> Response {
///         Response::Ok
///     }
/// }
/// ```
pub trait RequestHandler: Send + Sync + 'static {
    /// Process a single decoded request and return the response to send back.
    fn handle(&self, request: Request) -> impl std::future::Future<Output = Response> + Send;
}

/// IPC server bound to a Unix Domain Socket.
///
/// # Lifecycle
///
/// 1. Create with [`IpcServer::bind`] or [`IpcServer::bind_default`].
/// 2. Call [`IpcServer::serve`] with an `Arc<impl RequestHandler>` to start
///    accepting connections.  `serve` runs until the listener errors.
/// 3. On `Drop`, the socket file is removed automatically.
pub struct IpcServer {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl IpcServer {
    /// Bind to `path`, removing any stale socket file from a previous run.
    pub fn bind(path: PathBuf) -> Result<Self> {
        // Best-effort removal of a stale socket left by a crashed daemon.
        #[cfg(unix)]
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        // Ensure the parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&path)?;

        // Explicitly set socket file permissions to 0600 (owner only).
        // This prevents other users from connecting to the socket.
        // Without this, permissions are determined by umask which may be too permissive.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            debug!(path = %path.display(), "Set socket permissions to 0600");
        }

        info!(path = %path.display(), "IPC server listening");
        Ok(Self {
            listener,
            socket_path: path,
        })
    }

    /// Bind using the platform default socket path (see [`crate::socket_path`]).
    pub fn bind_default() -> Result<Self> {
        Self::bind(crate::socket::socket_path()?)
    }

    /// Returns the path this server is bound to.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    /// Accept connections in a loop, spawning a task per connection.
    ///
    /// `handler` is wrapped in `Arc` and cloned into each connection task.
    /// This method runs until the listener returns an unrecoverable error.
    pub async fn serve<H: RequestHandler>(self, handler: Arc<H>) -> Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let handler = Arc::clone(&handler);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, handler).await {
                            warn!("Connection error: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {e}");
                    return Err(Error::Socket(e));
                }
            }
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        // Best-effort cleanup: remove the socket file on shutdown.
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Handle a single client connection: read requests in a loop and write
/// responses until the client disconnects.
async fn handle_connection<H: RequestHandler>(stream: UnixStream, handler: Arc<H>) -> Result<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);

    loop {
        let request: Request = match framing::read_message(&mut reader).await {
            Ok(r) => r,
            Err(Error::ConnectionClosed) => {
                debug!("Client disconnected");
                break;
            }
            Err(e) => return Err(e),
        };

        debug!(?request, "received IPC request");
        let response = handler.handle(request).await;
        debug!(?response, "sending IPC response");
        framing::write_message(&mut writer, &response).await?;
    }
    Ok(())
}
