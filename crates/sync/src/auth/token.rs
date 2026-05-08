//! Google OAuth token validation and refresh.

use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::GoogleTokenStore;
use crate::error::{Error, Result};
use crate::http::{blocking_client, send_blocking_with_retry};

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const TOKEN_EXPIRY_BUFFER_SECS: u64 = 30;
const DEFAULT_EXPIRES_IN_SECS: u64 = 3600;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

/// Check if the stored access token is still usable.
pub fn is_token_valid(store: &GoogleTokenStore) -> bool {
    if store.access_token.expose_secret().is_empty() {
        return false;
    }
    let Some(expires_at) = store.expires_at else {
        return false;
    };
    now_secs() < expires_at.saturating_sub(TOKEN_EXPIRY_BUFFER_SECS)
}

/// Exchange an authorization code for access and refresh tokens.
pub(crate) fn exchange_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<GoogleTokenStore> {
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    let token = request_token("token exchange", &params)?;
    Ok(GoogleTokenStore {
        access_token: Secret::new(token.access_token),
        refresh_token: Secret::new(token.refresh_token.unwrap_or_default()),
        expires_at: Some(expires_at(token.expires_in)),
    })
}

/// Use a refresh token to obtain a fresh access token.
pub fn refresh_tokens(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<GoogleTokenStore> {
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let token = request_token("token refresh", &params)?;
    Ok(GoogleTokenStore {
        access_token: Secret::new(token.access_token),
        refresh_token: Secret::new(refresh_token.to_string()),
        expires_at: Some(expires_at(token.expires_in)),
    })
}

fn request_token(action: &'static str, params: &[(&str, &str)]) -> Result<TokenResponse> {
    let client = blocking_client()?;
    let resp = send_blocking_with_retry(client.post(TOKEN_URL).form(params))
        .map_err(|e| Error::Config(format!("{action} failed: {e}")))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| Error::Config(format!("{action} response read failed: {e}")))?;

    if !status.is_success() {
        return Err(Error::Config(format!(
            "{action} failed with HTTP {status}: {body}"
        )));
    }

    serde_json::from_str(&body)
        .map_err(|e| Error::Config(format!("{action} response parse failed: {e}")))
}

fn expires_at(expires_in: Option<u64>) -> u64 {
    now_secs() + expires_in.unwrap_or(DEFAULT_EXPIRES_IN_SECS)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(access_token: &str, expires_at: Option<u64>) -> GoogleTokenStore {
        GoogleTokenStore {
            access_token: Secret::new(access_token.to_string()),
            refresh_token: Secret::new("refresh".to_string()),
            expires_at,
        }
    }

    #[test]
    fn token_without_access_token_is_invalid() {
        assert!(!is_token_valid(&token("", Some(now_secs() + 3600))));
    }

    #[test]
    fn token_expiring_inside_buffer_is_invalid() {
        assert!(!is_token_valid(&token("access", Some(now_secs() + 10))));
    }

    #[test]
    fn token_with_future_expiry_is_valid() {
        assert!(is_token_valid(&token("access", Some(now_secs() + 3600))));
    }
}
