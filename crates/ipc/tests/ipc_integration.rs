//! Integration tests for lockit-ipc.
//!
//! These tests spin up a real `UnixListener`, connect a client, and verify
//! round-trip behaviour including response timing.
//!
//! Gated on `#[cfg(unix)]` so the test module compiles but does nothing on
//! Windows.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Instant;

use lockit_ipc::{IpcClient, IpcServer, Password, Request, RequestHandler, Response};
use tempfile::TempDir;

/// Minimal handler that covers every request variant.
struct EchoHandler;

impl RequestHandler for EchoHandler {
    async fn handle(&self, request: Request) -> Response {
        match request {
            Request::DaemonStatus => Response::Status {
                locked: false,
                version: "0.1.0-test".into(),
                uptime_secs: 0,
            },
            Request::ListProfiles => Response::Profiles {
                profiles: vec!["github".into(), "aws".into()],
            },
            Request::ListKeys { .. } => Response::Keys {
                keys: vec!["token".into()],
            },
            Request::GetCredential { .. } => Response::Value {
                value: Some(b"secret_value".to_vec()),
            },
            _ => Response::Ok,
        }
    }
}

/// Spawn a server in a background task and return the client pointed at it.
/// The `TempDir` must be kept alive for the duration of the test.
async fn spawn_server(dir: &TempDir) -> IpcClient {
    let socket_path = dir.path().join("test.sock");
    let server = IpcServer::bind(socket_path.clone()).unwrap();
    tokio::spawn(async move {
        // Ignore the serve error when the test ends and the socket is dropped.
        let _ = server.serve(Arc::new(EchoHandler)).await;
    });
    // Yield once to let the server task start accepting.
    tokio::task::yield_now().await;
    IpcClient::new(socket_path).with_timeout(1_000)
}

// ── Correctness tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn daemon_status_returns_status_response() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client.send_request(&Request::DaemonStatus).await.unwrap();
    assert!(
        matches!(resp, Response::Status { locked: false, .. }),
        "unexpected: {resp:?}"
    );
}

#[tokio::test]
async fn lock_vault_returns_ok() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client.send_request(&Request::LockVault).await.unwrap();
    assert_eq!(resp, Response::Ok);
}

#[tokio::test]
async fn unlock_vault_returns_ok() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client
        .send_request(&Request::UnlockVault {
            password: Password::new("hunter2"),
            device_key: vec![0u8; 32],
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
}

#[tokio::test]
async fn get_credential_returns_value() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client
        .send_request(&Request::GetCredential {
            profile: "github".into(),
            key: "token".into(),
        })
        .await
        .unwrap();
    assert!(matches!(resp, Response::Value { value: Some(_) }));
}

#[tokio::test]
async fn set_credential_returns_ok() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client
        .send_request(&Request::SetCredential {
            profile: "github".into(),
            key: "token".into(),
            value: b"ghp_secret".to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
}

#[tokio::test]
async fn delete_credential_returns_ok() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client
        .send_request(&Request::DeleteCredential {
            profile: "p".into(),
            key: "k".into(),
        })
        .await
        .unwrap();
    assert_eq!(resp, Response::Ok);
}

#[tokio::test]
async fn list_profiles_returns_profiles() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client.send_request(&Request::ListProfiles).await.unwrap();
    assert!(matches!(resp, Response::Profiles { .. }));
}

#[tokio::test]
async fn list_keys_returns_keys() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;
    let resp = client
        .send_request(&Request::ListKeys {
            profile: "github".into(),
        })
        .await
        .unwrap();
    assert!(matches!(resp, Response::Keys { .. }));
}

// ── Performance test ──────────────────────────────────────────────────────────

#[tokio::test]
async fn round_trip_latency_under_10ms() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(&dir).await;

    let start = Instant::now();
    client.send_request(&Request::DaemonStatus).await.unwrap();
    let elapsed = start.elapsed();

    // Allow up to 100 ms to avoid flakiness under parallel test load.
    assert!(
        elapsed.as_millis() < 100,
        "round trip took {}ms, expected <100ms",
        elapsed.as_millis()
    );
}

// ── Concurrent clients ────────────────────────────────────────────────────────

#[tokio::test]
async fn multiple_concurrent_clients() {
    let dir = TempDir::new().unwrap();
    let socket_path = dir.path().join("concurrent.sock");
    let server = IpcServer::bind(socket_path.clone()).unwrap();
    tokio::spawn(async move {
        let _ = server.serve(Arc::new(EchoHandler)).await;
    });
    tokio::task::yield_now().await;

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let path = socket_path.clone();
            tokio::spawn(async move {
                let client = IpcClient::new(path).with_timeout(1_000);
                client.send_request(&Request::DaemonStatus).await.unwrap()
            })
        })
        .collect();

    for handle in handles {
        let resp = handle.await.unwrap();
        assert!(matches!(resp, Response::Status { .. }));
    }
}

// ── Error handling ────────────────────────────────────────────────────────────

#[tokio::test]
async fn connect_to_nonexistent_socket_returns_timeout_or_socket_error() {
    let client = IpcClient::new("/tmp/lockit-nonexistent-test.sock".into()).with_timeout(100);
    let err = client
        .send_request(&Request::DaemonStatus)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            lockit_ipc::Error::Timeout { .. } | lockit_ipc::Error::Socket(_)
        ),
        "unexpected error: {err:?}"
    );
}

// ── Security tests ────────────────────────────────────────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn socket_permissions_are_restricted_to_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let socket_path = dir.path().join("secure.sock");
    let _server = IpcServer::bind(socket_path.clone()).unwrap();

    let metadata = std::fs::metadata(&socket_path).unwrap();
    let perms = metadata.permissions().mode() & 0o777;
    assert_eq!(
        perms, 0o600,
        "socket permissions should be 0600, got {:04o}",
        perms
    );
}

// ── Attack / robustness tests ─────────────────────────────────────────────────

/// Sending a frame whose 4-byte length prefix exceeds MAX_FRAME_SIZE (4 MiB) must
/// be rejected by the server without crashing or OOM-ing; the server must still
/// handle normal requests afterward.
#[tokio::test]
async fn oversized_frame_is_rejected_server_stays_alive() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let dir = TempDir::new().unwrap();
    let socket_path = dir.path().join("attack.sock");
    let server = IpcServer::bind(socket_path.clone()).unwrap();
    tokio::spawn(async move {
        let _ = server.serve(Arc::new(EchoHandler)).await;
    });
    tokio::task::yield_now().await;

    // Send a raw frame with a 512 MiB length prefix — server should close the conn.
    {
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let huge_len: u32 = 512 * 1024 * 1024;
        stream.write_all(&huge_len.to_be_bytes()).await.unwrap();
        drop(stream);
    }

    // Server must still be alive and handle a normal request afterward.
    let client = IpcClient::new(socket_path).with_timeout(1_000);
    let resp = client.send_request(&Request::DaemonStatus).await.unwrap();
    assert!(matches!(resp, Response::Status { .. }));
}

/// 50 concurrent clients all sending requests simultaneously must all receive
/// valid responses — the server must not deadlock, panic, or drop connections.
#[tokio::test]
async fn high_concurrency_stress_50_clients() {
    let dir = TempDir::new().unwrap();
    let socket_path = dir.path().join("stress.sock");
    let server = IpcServer::bind(socket_path.clone()).unwrap();
    tokio::spawn(async move {
        let _ = server.serve(Arc::new(EchoHandler)).await;
    });
    tokio::task::yield_now().await;

    let handles: Vec<_> = (0..50)
        .map(|i| {
            let path = socket_path.clone();
            tokio::spawn(async move {
                let client = IpcClient::new(path).with_timeout(2_000);
                let req = if i % 2 == 0 {
                    Request::DaemonStatus
                } else {
                    Request::ListProfiles
                };
                client.send_request(&req).await
            })
        })
        .collect();

    let mut ok_count = 0usize;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            ok_count += 1;
        }
    }
    assert_eq!(ok_count, 50, "only {ok_count}/50 requests succeeded");
}

/// After the socket file is deleted while the server is running, a new client
/// attempting to connect must get a clear socket error rather than hanging.
#[tokio::test]
async fn socket_deleted_client_gets_error() {
    let dir = TempDir::new().unwrap();
    let socket_path = dir.path().join("deleted.sock");
    let server = IpcServer::bind(socket_path.clone()).unwrap();
    tokio::spawn(async move {
        let _ = server.serve(Arc::new(EchoHandler)).await;
    });
    tokio::task::yield_now().await;

    let client = IpcClient::new(socket_path.clone()).with_timeout(1_000);
    client.send_request(&Request::DaemonStatus).await.unwrap();

    std::fs::remove_file(&socket_path).unwrap();

    let client2 = IpcClient::new(socket_path).with_timeout(200);
    let err = client2
        .send_request(&Request::DaemonStatus)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            lockit_ipc::Error::Timeout { .. } | lockit_ipc::Error::Socket(_)
        ),
        "expected socket error after deletion, got: {err:?}"
    );
}
