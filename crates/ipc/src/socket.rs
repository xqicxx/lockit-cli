//! Platform-aware socket path resolution.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// Returns the per-user socket path for the lockit daemon.
///
/// **Unix (Linux / macOS)**
/// 1. `$XDG_RUNTIME_DIR/lockit.sock` — preferred on Linux (tmpfs, per-user,
///    auto-cleaned by systemd).
/// 2. `~/.lockit/daemon.sock` — fallback used on macOS where
///    `XDG_RUNTIME_DIR` is typically unset.
///
/// **Windows** — returns [`Error::NotImplemented`].
/// Named Pipe support (`\\.\pipe\lockit-<username>`) will be added in a
/// future unit; the stub compiles cleanly and fails at runtime only when
/// actually called on Windows.
pub fn socket_path() -> Result<PathBuf> {
    #[cfg(unix)]
    {
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            return Ok(PathBuf::from(xdg).join("lockit.sock"));
        }
        let home = home::home_dir().ok_or_else(|| {
            Error::Socket(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "home directory not found",
            ))
        })?;
        Ok(home.join(".lockit").join("daemon.sock"))
    }

    #[cfg(not(unix))]
    {
        Err(Error::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn socket_path_returns_a_path() {
        // Just verify it doesn't error; the exact path depends on env.
        let path = socket_path().expect("socket_path should succeed on Unix");
        assert!(path.to_str().unwrap().contains("lockit"));
    }

    #[cfg(unix)]
    #[test]
    fn socket_path_uses_xdg_runtime_dir_when_set() {
        // SAFETY: single-threaded test; no other threads read this var.
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", "/tmp/test-xdg");
        }
        let path = socket_path().unwrap();
        // SAFETY: restore env after test.
        unsafe {
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
        assert_eq!(path, PathBuf::from("/tmp/test-xdg/lockit.sock"));
    }
}
