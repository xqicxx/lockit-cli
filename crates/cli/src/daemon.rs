//! Daemon management for lockit.
//!
//! Provides:
//! - [`VaultHandler`]: implements `RequestHandler` for the IPC server.
//! - [`run_daemon_foreground`]: blocking entry point for `lk daemon run`.
//! - [`start_daemon`]: spawn daemon in the background.
//! - [`stop_daemon`]: send SIGTERM to a running daemon.
//! - [`status_daemon`]: query the daemon via IPC.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use lockit_core::{Secret, UnlockedVault};
use lockit_ipc::{ErrorKind, IpcClient, IpcServer, Request, RequestHandler, Response};
use secrecy::ExposeSecret;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::vault;

// ─── PID file helpers ────────────────────────────────────────────────────────

fn pid_path() -> Result<PathBuf> {
    let home = home::home_dir().ok_or_else(|| anyhow!("home directory not found"))?;
    Ok(home.join(".lockit").join("daemon.pid"))
}

fn write_pid(pid: u32) -> Result<()> {
    let path = pid_path()?;
    std::fs::write(&path, pid.to_string())?;
    // Restrict to owner-only so other local users cannot observe the daemon PID.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn read_pid() -> Result<Option<u32>> {
    let path = pid_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let pid: u32 = content
        .trim()
        .parse()
        .map_err(|_| anyhow!("invalid PID file"))?;
    Ok(Some(pid))
}

fn remove_pid() {
    if let Ok(path) = pid_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn is_running(pid: u32) -> bool {
    // On Linux, check /proc/{pid}
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    // On other Unix, use kill(pid, 0)
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use nix::sys::signal;
        use nix::unistd::Pid;
        signal::kill(Pid::from_raw(pid as i32), None).is_ok()
    }
    // On non-Unix platforms, assume running if PID file exists (best effort)
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

// ─── DaemonState ─────────────────────────────────────────────────────────────

struct DaemonState {
    vault: Option<UnlockedVault>,
    vault_path: PathBuf,
    last_activity: Instant,
    start_time: Instant,
}

// ─── VaultHandler ─────────────────────────────────────────────────────────────

/// IPC request handler that manages an in-memory unlocked vault.
pub struct VaultHandler {
    state: Mutex<DaemonState>,
}

impl VaultHandler {
    fn new(vault_path: PathBuf) -> Self {
        let now = Instant::now();
        Self {
            state: Mutex::new(DaemonState {
                vault: None,
                vault_path,
                last_activity: now,
                start_time: now,
            }),
        }
    }
}

fn ipc_error(kind: ErrorKind, msg: impl Into<String>) -> Response {
    Response::Error {
        kind,
        message: msg.into(),
    }
}

/// Common unlock logic shared by `UnlockVault` and `UnlockWithBiometric`.
fn do_unlock(
    state: &mut DaemonState,
    password: &str,
    device_key: Vec<u8>,
    log_msg: &str,
) -> Response {
    let dk: [u8; 32] = match device_key.try_into() {
        Ok(k) => k,
        Err(_) => return ipc_error(ErrorKind::IncorrectPassword, "invalid device key length"),
    };

    match UnlockedVault::open(&state.vault_path, password, &dk) {
        Ok(v) => {
            state.vault = Some(v);
            info!("{}", log_msg);
            Response::Ok
        }
        Err(lockit_core::Error::IncorrectPassword) => ipc_error(
            ErrorKind::IncorrectPassword,
            "incorrect password or device key",
        ),
        Err(e) => ipc_error(ErrorKind::Internal, format!("failed to unlock vault: {e}")),
    }
}

impl RequestHandler for VaultHandler {
    async fn handle(&self, request: Request) -> Response {
        let mut state = self.state.lock().await;
        state.last_activity = Instant::now();

        match request {
            Request::UnlockVault {
                password,
                device_key,
            } => do_unlock(
                &mut state,
                password.expose(),
                device_key,
                "Vault unlocked via IPC",
            ),

            Request::UnlockWithBiometric {
                password,
                device_key,
                biometric_source,
            } => do_unlock(
                &mut state,
                password.expose(),
                device_key,
                &format!("Vault unlocked via biometric ({biometric_source})"),
            ),

            Request::LockVault => {
                drop(state.vault.take());
                info!("Vault locked via IPC");
                Response::Ok
            }

            Request::GetCredential { profile, key } => match &state.vault {
                None => ipc_error(ErrorKind::VaultLocked, "vault is locked"),
                Some(vault) => match vault.get(&profile, &key) {
                    Ok(Some(secret)) => Response::Value {
                        value: Some(secret.expose_secret().to_vec()),
                    },
                    Ok(None) => Response::Value { value: None },
                    Err(e) => ipc_error(ErrorKind::Internal, format!("get failed: {e}")),
                },
            },

            Request::SetCredential {
                profile,
                key,
                value,
            } => match &mut state.vault {
                None => ipc_error(ErrorKind::VaultLocked, "vault is locked"),
                Some(vault) => {
                    let secret = Secret::new(value);
                    match vault.set(&profile, &key, &secret) {
                        Ok(()) => match vault.save() {
                            Ok(()) => Response::Ok,
                            Err(e) => ipc_error(ErrorKind::Internal, format!("save failed: {e}")),
                        },
                        Err(e) => ipc_error(ErrorKind::Internal, format!("set failed: {e}")),
                    }
                }
            },

            Request::DeleteCredential { profile, key } => match &mut state.vault {
                None => ipc_error(ErrorKind::VaultLocked, "vault is locked"),
                Some(vault) => match vault.delete(&profile, &key) {
                    Ok(true) => match vault.save() {
                        Ok(()) => Response::Ok,
                        Err(e) => ipc_error(ErrorKind::Internal, format!("save failed: {e}")),
                    },
                    Ok(false) => ipc_error(ErrorKind::NotFound, "key not found"),
                    Err(e) => ipc_error(ErrorKind::Internal, format!("delete failed: {e}")),
                },
            },

            Request::ListProfiles => match &state.vault {
                None => ipc_error(ErrorKind::VaultLocked, "vault is locked"),
                Some(vault) => match vault.profiles() {
                    Ok(profiles) => Response::Profiles { profiles },
                    Err(e) => ipc_error(ErrorKind::Internal, format!("list profiles failed: {e}")),
                },
            },

            Request::ListKeys { profile } => match &state.vault {
                None => ipc_error(ErrorKind::VaultLocked, "vault is locked"),
                Some(vault) => match vault.keys(&profile) {
                    Ok(keys) => Response::Keys { keys },
                    Err(e) => ipc_error(ErrorKind::Internal, format!("list keys failed: {e}")),
                },
            },

            Request::DaemonStatus => {
                let locked = state.vault.is_none();
                let uptime_secs = state.start_time.elapsed().as_secs();
                Response::Status {
                    locked,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    uptime_secs,
                }
            }
        }
    }
}

// ─── Auto-lock background task ───────────────────────────────────────────────

async fn auto_lock_task(handler: Arc<VaultHandler>) {
    const CHECK_INTERVAL: Duration = Duration::from_secs(60);
    const IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

    loop {
        tokio::time::sleep(CHECK_INTERVAL).await;
        let mut state = handler.state.lock().await;
        if state.vault.is_some() && state.last_activity.elapsed() >= IDLE_TIMEOUT {
            // Explicitly drop the vault so that UnlockedVault's Drop impl zeroizes
            // the VEK and all in-memory credential bytes before setting to None.
            drop(state.vault.take());
            info!("Vault auto-locked due to inactivity");
        }
    }
}

// ─── run_daemon_foreground ───────────────────────────────────────────────────

/// Start the daemon in the foreground (called by `lk daemon run`).
/// This function sets up a tokio runtime and blocks until the daemon exits.
pub fn run_daemon_foreground() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run_daemon_async())
}

async fn run_daemon_async() -> Result<()> {
    // Write PID file
    write_pid(std::process::id())?;

    // Set up cleanup on exit
    let _guard = PidGuard;

    // Set up SIGTERM handler
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate())?;
        let pid = std::process::id();
        tokio::spawn(async move {
            sigterm.recv().await;
            info!("Received SIGTERM, shutting down daemon (PID {})", pid);
            remove_pid();
            std::process::exit(0);
        });
    }

    // Build the vault handler
    let vault_path = vault::vault_path()?;
    let handler = Arc::new(VaultHandler::new(vault_path));

    // Spawn auto-lock background task
    let handler_clone = Arc::clone(&handler);
    tokio::spawn(auto_lock_task(handler_clone));

    // Bind IPC server
    let server = IpcServer::bind_default()?;
    info!("Daemon listening on {}", server.socket_path().display());
    println!("Daemon listening on {}", server.socket_path().display());

    server.serve(handler).await?;

    Ok(())
}

/// RAII guard that removes the PID file when dropped.
struct PidGuard;

impl Drop for PidGuard {
    fn drop(&mut self) {
        remove_pid();
    }
}

// ─── start_daemon ────────────────────────────────────────────────────────────

/// Start the daemon in the background by spawning `lk daemon run`.
pub fn start_daemon() -> Result<()> {
    if let Some(pid) = read_pid()? {
        if is_running(pid) {
            println!("Daemon already running (PID {pid}).");
            return Ok(());
        }
        // Stale PID file — clean it up
        remove_pid();
    }

    let exe = std::env::current_exe()?;

    let child = std::process::Command::new(&exe)
        .arg("daemon")
        .arg("run")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let child_pid = child.id();
    // Detach: let the child run independently
    std::mem::forget(child);

    println!("Daemon started (PID {child_pid}).");
    Ok(())
}

// ─── stop_daemon ─────────────────────────────────────────────────────────────

/// Send SIGTERM to a running daemon.
#[cfg(unix)]
pub fn stop_daemon() -> Result<()> {
    let pid = read_pid()?.ok_or_else(|| anyhow!("Daemon is not running."))?;
    if !is_running(pid) {
        remove_pid();
        bail!("Daemon is not running (stale PID file removed).");
    }

    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM)?;
    println!("Sent SIGTERM to daemon (PID {pid}).");
    Ok(())
}

#[cfg(not(unix))]
pub fn stop_daemon() -> Result<()> {
    bail!("stop_daemon is not supported on this platform.");
}

// ─── status_daemon ───────────────────────────────────────────────────────────

/// Query the daemon status via IPC.
pub fn status_daemon() -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let response = rt.block_on(async {
        let client = IpcClient::new_default()?;
        client.send_request(&Request::DaemonStatus).await
    });

    match response {
        Ok(Response::Status {
            locked,
            version,
            uptime_secs,
        }) => {
            println!("Daemon is running.");
            println!("  Version:  {version}");
            println!("  Vault:    {}", if locked { "locked" } else { "unlocked" });
            println!("  Uptime:   {}s", uptime_secs);
        }
        Err(e) => {
            // Check if the daemon is even running
            match read_pid()? {
                Some(pid) if is_running(pid) => {
                    bail!("Daemon is running (PID {pid}) but IPC failed: {e}");
                }
                _ => {
                    println!("Daemon is not running.");
                }
            }
        }
        Ok(other) => {
            warn!("unexpected IPC response: {:?}", other);
            bail!("Unexpected response from daemon.");
        }
    }

    Ok(())
}
