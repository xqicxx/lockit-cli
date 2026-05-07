//! Shared utilities for lockit-sync.

use sha2::{Digest, Sha256};

/// Compute the SHA-256 digest of `data` and return it as a lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

/// Prepend `prefix` to `key`, inserting a `/` separator when the prefix is
/// non-empty and does not already end with `/`.
#[allow(dead_code)]
pub fn full_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}/{}", prefix.trim_end_matches('/'), key)
    }
}

/// URL-encode a string for query parameters.
pub fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Decode a query parameter value.
pub fn url_decode(value: &str) -> String {
    let mut result = Vec::new();
    let mut bytes = value.bytes();

    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => {
                let Some(h1) = bytes.next() else {
                    result.push(b'%');
                    break;
                };
                let Some(h2) = bytes.next() else {
                    result.extend_from_slice(&[b'%', h1]);
                    break;
                };
                let hex = [h1, h2];
                match std::str::from_utf8(&hex)
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(decoded) => result.push(decoded),
                    None => {
                        result.extend_from_slice(&[b'%', h1, h2]);
                    }
                }
            }
            b'+' => result.push(b' '),
            _ => result.push(byte),
        }
    }

    String::from_utf8_lossy(&result).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_empty() {
        // SHA-256 of empty input is a well-known constant.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_known_value() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn full_key_no_prefix() {
        assert_eq!(full_key("", "vault.lockit"), "vault.lockit");
    }

    #[test]
    fn full_key_with_prefix_no_slash() {
        assert_eq!(full_key("lockit", "vault.lockit"), "lockit/vault.lockit");
    }

    #[test]
    fn full_key_with_prefix_trailing_slash() {
        assert_eq!(full_key("lockit/", "vault.lockit"), "lockit/vault.lockit");
    }

    #[test]
    fn url_encode_decodes_roundtrip_for_query_values() {
        let encoded = url_encode("a b/ç");
        assert_eq!(encoded, "a%20b%2F%C3%A7");
        assert_eq!(url_decode(&encoded), "a b/ç");
        assert_eq!(url_decode("hello+world%21"), "hello world!");
    }

    #[test]
    fn url_decode_keeps_invalid_percent_escapes() {
        assert_eq!(url_decode("bad%2"), "bad%2");
        assert_eq!(url_decode("bad%zz"), "bad%zz");
    }
}
