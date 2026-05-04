use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_REDIRECT_PORT: u16 = 0; // 0 = OS picks a random port

pub const GOOGLE_CLIENT_ID_DEFAULT: &str =
    "1067183645292-2dshqcfq5jfraokjn4p8davhgkponfua.apps.googleusercontent.com";

pub fn google_client_id() -> String {
    std::env::var("LOCKIT_GOOGLE_CLIENT_ID")
        .unwrap_or_else(|_| GOOGLE_CLIENT_ID_DEFAULT.to_string())
}
const GOOGLE_DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub fn start_oauth_flow() -> Result<OAuthTokens, String> {
    // Generate PKCE code verifier and challenge
    let code_verifier = generate_code_verifier();
    let code_challenge = base64url_sha256(&code_verifier);

    // Bind to a random port
    let listener = TcpListener::bind(("127.0.0.1", GOOGLE_REDIRECT_PORT))
        .map_err(|e| format!("Failed to bind localhost: {e}"))?;
    let port = listener.local_addr().map_err(|e| format!("{e}"))?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Build the authorization URL
    let client_id = google_client_id();
    let auth_url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline",
        GOOGLE_AUTH_URL,
        url_encode(client_id.as_str()),
        url_encode(&redirect_uri),
        url_encode(GOOGLE_DRIVE_SCOPE),
        url_encode(&code_challenge),
    );

    // Open browser
    if let Err(e) = open::that(&auth_url) {
        eprintln!("Could not open browser automatically: {e}");
        eprintln!("Open this URL:\n\n{auth_url}\n");
    } else {
        eprintln!("Browser opened. Authorize the app and return here.");
    }

    // Set nonblocking so we can poll with a timeout
    listener.set_nonblocking(true).map_err(|e| format!("{e}"))?;

    // Accept ONE connection, extract the auth code (120s timeout)
    let code = match wait_for_callback(&listener) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "OAuth login timed out or failed.\n\
                 If the client ID is invalid, set LOCKIT_GOOGLE_CLIENT_ID env var.\n\
                 Or configure manually: lockit sync config\n\
                 Error: {e}"
            );
            return Err(e);
        }
    };
    drop(listener);

    // Exchange code for tokens
    exchange_code(&code, &code_verifier, &redirect_uri)
}

fn wait_for_callback(listener: &TcpListener) -> Result<String, String> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .map_err(|e| format!("Failed to set stream blocking: {e}"))?;
                stream
                    .set_read_timeout(Some(timeout))
                    .map_err(|e| format!("Failed to set read timeout: {e}"))?;
                break stream;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() > timeout {
                    return Err(
                        "OAuth login timed out after 120s. Browser did not complete authorization."
                            .to_string(),
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(format!("Connection error: {e}")),
        }
    };

    let mut reader = BufReader::new(stream.try_clone().map_err(|e| format!("{e}"))?);
    let mut stream = stream;
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("{e}"))?;

    // Parse GET /callback?code=...&scope=... HTTP/1.1
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let code = if let Some(query) = path.split('?').nth(1) {
        query
            .split('&')
            .find_map(|p| {
                let (k, v) = p.split_once('=')?;
                if k == "code" {
                    Some(v.to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| "No authorization code in callback".to_string())?
    } else {
        return Err("Invalid callback path".to_string());
    };

    // Send success response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>✓ Authorized</h1><p>You can close this tab and return to the terminal.</p></body></html>";
    let _ = stream.write_all(response.as_bytes());

    Ok(code)
}

fn exchange_code(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<OAuthTokens, String> {
    let cid = google_client_id();
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("client_id", cid.as_str()),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .map_err(|e| format!("Token request failed: {e}"))?;

    let json: serde_json::Value = resp.json().map_err(|e| format!("Parse error: {e}"))?;

    if let Some(err) = json.get("error") {
        return Err(format!("OAuth error: {err}"));
    }

    Ok(OAuthTokens {
        access_token: json["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: json["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_in: json["expires_in"].as_i64().unwrap_or(3600),
    })
}

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn base64url_sha256(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
