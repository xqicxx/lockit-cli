//! WebDAV sync backend (Nextcloud, 坚果云, etc.)

use async_trait::async_trait;
use quick_xml::Reader;
use quick_xml::events::Event;
use secrecy::{ExposeSecret, Secret};
use sha2::{Digest, Sha256};

use crate::backend::{SyncBackend, SyncMetadata};
use crate::config::WebDavConfig;
use crate::error::{Error, Result};

pub struct WebDavBackend {
    client: reqwest::Client,
    base_url: String,
    username: String,
    /// Zeroized on drop via `secrecy::Secret`.
    password: Secret<String>,
}

impl WebDavBackend {
    pub fn new(cfg: &WebDavConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| Error::Config(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            client,
            base_url: cfg.url.trim_end_matches('/').to_string(),
            username: cfg.username.clone(),
            password: Secret::new(cfg.password.expose_secret().clone()),
        })
    }

    fn url(&self, key: &str) -> String {
        format!("{}/{}", self.base_url, key)
    }

    fn auth(&self) -> (&str, &str) {
        (&self.username, self.password.expose_secret().as_str())
    }
}

#[async_trait]
impl SyncBackend for WebDavBackend {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        let (u, p) = self.auth();
        let res = self
            .client
            .put(self.url(key))
            .basic_auth(u, Some(p))
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| Error::Upload {
                key: key.to_string(),
                reason: e.to_string(),
            })?;
        if !res.status().is_success() {
            return Err(Error::Upload {
                key: key.to_string(),
                reason: format!("PUT failed: {}", res.status()),
            });
        }
        Ok(())
    }

    async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let (u, p) = self.auth();
        let res = self
            .client
            .get(self.url(key))
            .basic_auth(u, Some(p))
            .send()
            .await
            .map_err(|e| Error::Download {
                key: key.to_string(),
                reason: e.to_string(),
            })?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound {
                key: key.to_string(),
            });
        }
        if !res.status().is_success() {
            return Err(Error::Download {
                key: key.to_string(),
                reason: format!("GET failed: {}", res.status()),
            });
        }
        res.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| Error::Download {
                key: key.to_string(),
                reason: e.to_string(),
            })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let (u, p) = self.auth();
        let res = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                format!("{}/", self.base_url),
            )
            .basic_auth(u, Some(p))
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:href/></d:prop></d:propfind>"#)
            .send()
            .await
            .map_err(|e| Error::List {
                prefix: prefix.to_string(),
                reason: e.to_string(),
            })?;

        let body = res.text().await.map_err(|e| Error::List {
            prefix: prefix.to_string(),
            reason: e.to_string(),
        })?;

        parse_propfind_hrefs(&body, &self.base_url, prefix)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let (u, p) = self.auth();
        let res = self
            .client
            .delete(self.url(key))
            .basic_auth(u, Some(p))
            .send()
            .await
            .map_err(|e| Error::Delete {
                key: key.to_string(),
                reason: e.to_string(),
            })?;
        if !res.status().is_success() && res.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(Error::Delete {
                key: key.to_string(),
                reason: format!("DELETE failed: {}", res.status()),
            });
        }
        Ok(())
    }

    async fn metadata(&self, key: &str) -> Result<SyncMetadata> {
        let (u, p) = self.auth();
        let res = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                self.url(key),
            )
            .basic_auth(u, Some(p))
            .header("Depth", "0")
            .header("Content-Type", "application/xml")
            .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getlastmodified/><d:getcontentlength/><d:getetag/></d:prop></d:propfind>"#)
            .send()
            .await
            .map_err(|e| Error::Metadata {
                key: key.to_string(),
                reason: e.to_string(),
            })?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound {
                key: key.to_string(),
            });
        }

        let body = res.text().await.map_err(|e| Error::Metadata {
            key: key.to_string(),
            reason: e.to_string(),
        })?;

        let (last_modified, size, etag) = parse_propfind_metadata(&body);

        // Use the ETag as checksum when available — it reflects actual file content.
        // WebDAV ETags are typically the content hash (e.g. MD5 on Apache/Nginx)
        // or a version counter, and change whenever the file changes.
        // Fall back to sha256(last_modified || size || key) if the server omits
        // the ETag; this is still content-sensitive unlike a static URL hash.
        let checksum = if let Some(tag) = etag {
            let mut h = Sha256::new();
            h.update(tag.trim_matches('"').as_bytes());
            hex::encode(h.finalize())
        } else {
            let mut h = Sha256::new();
            h.update(last_modified.unwrap_or(0).to_le_bytes());
            h.update(size.unwrap_or(0).to_le_bytes());
            h.update(key.as_bytes());
            hex::encode(h.finalize())
        };

        Ok(SyncMetadata {
            version: 1,
            last_modified: last_modified.unwrap_or(0),
            checksum,
            size: size.unwrap_or(0),
        })
    }

    fn backend_name(&self) -> &str {
        "webdav"
    }
}

// ── XML helpers ──────────────────────────────────────────────────────────────

/// Parse `<D:href>` elements from a PROPFIND response, returning keys
/// (relative to `base_url`) filtered by `prefix`.
fn parse_propfind_hrefs(xml: &str, base_url: &str, prefix: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut keys = Vec::new();
    let mut in_href = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name();
                // Only Start events have text children; Empty (self-closing)
                // elements have no content so we never set the flag for them.
                if name.as_ref() == b"href" {
                    in_href = true;
                }
            }
            Ok(Event::Empty(_)) => {
                // Self-closing tag — no text content follows, nothing to do.
            }
            Ok(Event::Text(e)) if in_href => {
                let href = e.unescape().unwrap_or_default().to_string();
                // Strip the base_url prefix to get the key.
                // Servers may return hrefs with or without a trailing slash on
                // the base path, so try both forms before falling back.
                let base_no_slash = base_url.trim_end_matches('/');
                let key = href
                    .strip_prefix(&format!("{}/", base_no_slash))
                    .or_else(|| href.strip_prefix(base_no_slash))
                    .unwrap_or(&href)
                    .trim_start_matches('/')
                    .to_string();
                if !key.is_empty() && key.starts_with(prefix) {
                    keys.push(key);
                }
                in_href = false;
            }
            Ok(Event::End(_)) => {
                in_href = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::List {
                    prefix: prefix.to_string(),
                    reason: format!("XML parse error: {e}"),
                });
            }
            _ => {}
        }
    }

    keys.sort();
    Ok(keys)
}

/// Parse `<D:getlastmodified>`, `<D:getcontentlength>`, and `<D:getetag>` from
/// a PROPFIND response.
///
/// Returns `(last_modified_unix, size_bytes, etag)` as `Option` values — `None`
/// when the corresponding XML element is absent or cannot be parsed.
fn parse_propfind_metadata(xml: &str) -> (Option<u64>, Option<u64>, Option<String>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut last_modified: Option<u64> = None;
    let mut size: Option<u64> = None;
    let mut etag: Option<String> = None;
    let mut in_lm = false;
    let mut in_cl = false;
    let mut in_etag = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                // Only Start events (not self-closing Empty) have text children.
                let name = e.local_name();
                match name.as_ref() {
                    b"getlastmodified" => in_lm = true,
                    b"getcontentlength" => in_cl = true,
                    b"getetag" => in_etag = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(_)) => {
                // Self-closing tag — no text child; reset to avoid mis-attribution.
                in_lm = false;
                in_cl = false;
                in_etag = false;
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_lm {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(&text) {
                        last_modified = Some(dt.timestamp().max(0) as u64);
                    }
                    in_lm = false;
                } else if in_cl {
                    size = text.trim().parse().ok();
                    in_cl = false;
                } else if in_etag {
                    let tag = text.trim().to_string();
                    if !tag.is_empty() {
                        etag = Some(tag);
                    }
                    in_etag = false;
                }
            }
            Ok(Event::Eof) => break,
            _ => {
                in_lm = false;
                in_cl = false;
                in_etag = false;
            }
        }
    }

    (last_modified, size, etag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WebDavConfig;

    fn test_cfg() -> WebDavConfig {
        WebDavConfig {
            url: "https://dav.example.com/lockit".into(),
            username: "user".into(),
            password: Secret::new("pass".into()),
        }
    }

    #[test]
    fn constructor_stores_fields() {
        let b = WebDavBackend::new(&test_cfg()).unwrap();
        assert_eq!(b.base_url, "https://dav.example.com/lockit");
        assert_eq!(b.username, "user");
        assert_eq!(b.password.expose_secret(), "pass");
    }

    #[test]
    fn backend_name_is_webdav() {
        let b = WebDavBackend::new(&test_cfg()).unwrap();
        assert_eq!(b.backend_name(), "webdav");
    }

    #[test]
    fn parse_hrefs_filters_prefix() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:href>https://dav.example.com/lockit/vault.lockit</d:href></d:response>
  <d:response><d:href>https://dav.example.com/lockit/other.txt</d:href></d:response>
</d:multistatus>"#;
        let keys = parse_propfind_hrefs(xml, "https://dav.example.com/lockit", "vault").unwrap();
        assert_eq!(keys, vec!["vault.lockit"]);
    }

    #[test]
    fn parse_metadata_uses_etag_when_present() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</d:getlastmodified>
        <d:getcontentlength>1024</d:getcontentlength>
        <d:getetag>"abc123"</d:getetag>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;
        let (_, _, etag) = parse_propfind_metadata(xml);
        assert_eq!(etag.as_deref(), Some("\"abc123\""));
    }
}
