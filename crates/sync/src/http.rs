//! Small HTTP helpers shared by OAuth and Google Drive.

use std::time::Duration;

use reqwest::{Client, Response, StatusCode, blocking};

use crate::error::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_ATTEMPTS: usize = 3;

pub(crate) fn blocking_client() -> Result<blocking::Client> {
    blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| Error::Config(format!("Failed to create HTTP client: {e}")))
}

pub(crate) fn async_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| Error::Config(format!("Failed to create HTTP client: {e}")))
}

pub(crate) fn send_blocking_with_retry(
    request: blocking::RequestBuilder,
) -> Result<blocking::Response> {
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let Some(req) = request.try_clone() else {
            return request
                .send()
                .map_err(|e| Error::Config(format!("HTTP request failed: {e}")));
        };

        match req.send() {
            Ok(resp) if should_retry_status(resp.status()) && attempt < MAX_ATTEMPTS => {
                last_error = Some(format!("HTTP {}", resp.status()));
                std::thread::sleep(RETRY_DELAY);
            }
            Ok(resp) => return Ok(resp),
            Err(err) if is_retryable_error(&err) && attempt < MAX_ATTEMPTS => {
                last_error = Some(err.to_string());
                std::thread::sleep(RETRY_DELAY);
            }
            Err(err) => return Err(Error::Config(format!("HTTP request failed: {err}"))),
        }
    }

    Err(Error::Config(format!(
        "HTTP request failed after {MAX_ATTEMPTS} attempts: {}",
        last_error.unwrap_or_else(|| "unknown error".to_string())
    )))
}

pub(crate) async fn send_async_with_retry(request: reqwest::RequestBuilder) -> Result<Response> {
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let Some(req) = request.try_clone() else {
            return request
                .send()
                .await
                .map_err(|e| Error::Config(format!("HTTP request failed: {e}")));
        };

        match req.send().await {
            Ok(resp) if should_retry_status(resp.status()) && attempt < MAX_ATTEMPTS => {
                last_error = Some(format!("HTTP {}", resp.status()));
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Ok(resp) => return Ok(resp),
            Err(err) if is_retryable_error(&err) && attempt < MAX_ATTEMPTS => {
                last_error = Some(err.to_string());
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(err) => return Err(Error::Config(format!("HTTP request failed: {err}"))),
        }
    }

    Err(Error::Config(format!(
        "HTTP request failed after {MAX_ATTEMPTS} attempts: {}",
        last_error.unwrap_or_else(|| "unknown error".to_string())
    )))
}

fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_statuses_are_limited_to_transient_failures() {
        assert!(should_retry_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(should_retry_status(StatusCode::BAD_GATEWAY));
        assert!(!should_retry_status(StatusCode::BAD_REQUEST));
        assert!(!should_retry_status(StatusCode::UNAUTHORIZED));
    }
}
