//! lockit-ipc — IPC protocol and transport for the lockit daemon.
//!
//! # Architecture
//!
//! ```text
//! Client process                       Daemon process
//! ─────────────                        ──────────────
//! IpcClient::send_request(req)
//!   → framing::write_message           IpcServer::serve(handler)
//!   ← framing::read_message              → framing::read_message
//!                                         → handler.handle(req)
//!                                         → framing::write_message
//! ```
//!
//! # Wire format
//!
//! Each message is a 4-byte big-endian `u32` length prefix followed by that
//! many bytes of named MessagePack (`rmp_serde::to_vec_named`).  The named
//! encoding stores string keys in every map, which makes the format
//! forward-compatible and human-debuggable with msgpack tools.
//!
//! # Platform support
//!
//! | Platform | Transport | Status |
//! |----------|-----------|--------|
//! | Linux    | Unix Domain Socket | ✅ Fully implemented |
//! | macOS    | Unix Domain Socket | ✅ Fully implemented |
//! | Windows  | Named Pipe (stub)  | ⚠️ Returns [`Error::NotImplemented`] |
//!
//! # Example
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use lockit_ipc::{IpcClient, IpcServer, Request, RequestHandler, Response};
//!
//! // Daemon side
//! struct MyHandler;
//! impl RequestHandler for MyHandler {
//!     async fn handle(&self, _req: Request) -> Response { Response::Ok }
//! }
//! let server = IpcServer::bind_default()?;
//! tokio::spawn(async move { server.serve(Arc::new(MyHandler)).await });
//!
//! // Client side
//! let client = IpcClient::new_default()?;
//! let resp = client.send_request(&Request::DaemonStatus).await?;
//! ```

pub mod client;
pub mod error;
pub mod proto;
pub mod server;

pub(crate) mod framing;
pub(crate) mod socket;

// Convenience re-exports
pub use client::{DEFAULT_TIMEOUT_MS, IpcClient};
pub use error::{Error, Result};
pub use proto::{ErrorKind, PROTOCOL_VERSION, Password, Request, Response};
pub use server::{IpcServer, RequestHandler};
pub use socket::socket_path;
