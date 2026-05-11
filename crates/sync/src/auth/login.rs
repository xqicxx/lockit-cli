//! Google OAuth 2.0 authentication for CLI.
//!
//! Uses the Authorization Code flow with PKCE:
//! 1. Generate PKCE challenge
//! 2. Open browser to Google's consent page
//! 3. Wait for the callback with the authorization code
//! 4. Exchange the code for access + refresh tokens

use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use crate::config::GoogleTokenStore;
use crate::error::{Error, Result};
use crate::util::{url_decode, url_encode};

use super::token;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);
const CALLBACK_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Generate a random code verifier for PKCE (43-128 bytes).
fn generate_pkce_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate PKCE code challenge from verifier.
fn pkce_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// Generate a random CSRF state token.
fn generate_csrf_state() -> String {
    let mut bytes = [0u8; 16];
    rand::fill(&mut bytes);
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Perform interactive Google OAuth login.
///
/// Opens a browser, waits for callback, returns tokens.
pub fn login(client_id: &str, _client_secret: &str) -> Result<GoogleTokenStore> {
    // Google PKCE flow for installed apps (client_secret not used)
    let verifier = generate_pkce_verifier();
    let challenge = pkce_code_challenge(&verifier);
    let state = generate_csrf_state();

    // Bind to a random port on localhost
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Build authorization URL
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}%20{}&state={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        AUTH_URL,
        url_encode(client_id),
        url_encode(&redirect_uri),
        url_encode("https://www.googleapis.com/auth/drive"),
        url_encode("https://www.googleapis.com/auth/drive.appdata"),
        state,
        challenge,
    );

    // Open browser
    eprintln!("Opening browser for Google authentication...");
    eprintln!("If it doesn't open automatically, visit:");
    eprintln!("  {auth_url}");
    eprintln!();

    open_browser(&auth_url);

    // Wait for callback
    let code = wait_for_callback(listener, &state)?;

    // Exchange code for tokens
    token::exchange_code(client_id, _client_secret, &code, &verifier, &redirect_uri)
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", url])
        .spawn();
}

fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(Error::Config(
                        "Timed out waiting for Google OAuth callback".into(),
                    ));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }
    };
    stream.set_read_timeout(Some(CALLBACK_READ_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split('?').nth(1).unwrap_or("");
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");
        match key {
            "code" => code = Some(url_decode(value)),
            "state" => state = Some(value.to_string()),
            _ => {}
        }
    }

    let code = code.ok_or_else(|| Error::Config("No authorization code in callback".into()))?;
    let state = state.ok_or_else(|| Error::Config("No state in callback".into()))?;

    if state != expected_state {
        return Err(Error::Config("CSRF state mismatch".into()));
    }

    // Send success response
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authentication successful!</h1><p>You can close this tab and return to the terminal.</p></body></html>";
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    Ok(code)
}
