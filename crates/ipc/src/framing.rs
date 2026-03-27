//! Length-prefixed MessagePack framing.
//!
//! Wire format per message:
//! ```text
//! [ u32 big-endian length (4 bytes) ][ msgpack payload (length bytes) ]
//! ```
//!
//! `rmp_serde::to_vec_named` is used so the msgpack map contains string keys,
//! which makes the format forward-compatible and debuggable.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result};

/// Maximum allowed frame payload size (4 MiB).
///
/// Frames claiming to be larger than this are rejected with
/// [`Error::FrameTooLarge`] to prevent memory exhaustion from rogue clients.
pub(crate) const MAX_FRAME_SIZE: u32 = 4 * 1024 * 1024;

/// Serialize `value` to named MessagePack and write it with a 4-byte
/// big-endian length prefix.
pub(crate) async fn write_message<W, T>(writer: &mut W, value: &T) -> Result<()>
where
    W: AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let payload = rmp_serde::to_vec_named(value).map_err(|e| Error::Serialize(e.to_string()))?;
    let len = payload.len() as u32;
    writer.write_u32(len).await?;
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a 4-byte big-endian length prefix, then read and deserialize that many
/// bytes as named MessagePack.
pub(crate) async fn read_message<R, T>(reader: &mut R) -> Result<T>
where
    R: AsyncRead + Unpin,
    T: serde::de::DeserializeOwned,
{
    let len = reader.read_u32().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::ConnectionClosed
        } else {
            Error::Socket(e)
        }
    })?;

    if len > MAX_FRAME_SIZE {
        return Err(Error::FrameTooLarge {
            size: len,
            max: MAX_FRAME_SIZE,
        });
    }

    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::ConnectionClosed
        } else {
            Error::Socket(e)
        }
    })?;

    rmp_serde::from_slice(&buf).map_err(|e| Error::Deserialize(e.to_string()))
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Request, Response};
    use tokio::io::{AsyncWriteExt, duplex};

    #[tokio::test]
    async fn request_roundtrip_over_duplex() {
        let (mut client, mut server) = duplex(4096);
        let req = Request::GetCredential {
            profile: "github".into(),
            key: "token".into(),
        };
        write_message(&mut client, &req).await.unwrap();
        let decoded: Request = read_message(&mut server).await.unwrap();
        // Request doesn't impl PartialEq (Secret fields); check by re-serializing.
        let orig_bytes = rmp_serde::to_vec_named(&req).unwrap();
        let decoded_bytes = rmp_serde::to_vec_named(&decoded).unwrap();
        assert_eq!(orig_bytes, decoded_bytes);
    }

    #[tokio::test]
    async fn response_roundtrip_over_duplex() {
        let (mut client, mut server) = duplex(4096);
        let resp = Response::Status {
            locked: false,
            version: "0.1.0".into(),
            uptime_secs: 99,
        };
        write_message(&mut client, &resp).await.unwrap();
        let decoded: Response = read_message(&mut server).await.unwrap();
        assert_eq!(decoded, resp);
    }

    #[tokio::test]
    async fn multiple_messages_in_sequence() {
        let (mut client, mut server) = duplex(4096);
        let msgs = vec![
            Request::ListProfiles,
            Request::LockVault,
            Request::DaemonStatus,
        ];
        for m in &msgs {
            write_message(&mut client, m).await.unwrap();
        }
        // Request doesn't impl PartialEq; compare by re-serializing.
        for expected in &msgs {
            let got: Request = read_message(&mut server).await.unwrap();
            let got_bytes = rmp_serde::to_vec_named(&got).unwrap();
            let exp_bytes = rmp_serde::to_vec_named(expected).unwrap();
            assert_eq!(got_bytes, exp_bytes);
        }
    }

    #[tokio::test]
    async fn frame_too_large_is_rejected() {
        let (mut client, mut server) = duplex(64);
        // Write a fake length prefix that exceeds MAX_FRAME_SIZE
        let bad_len = (MAX_FRAME_SIZE + 1).to_be_bytes();
        client.write_all(&bad_len).await.unwrap();
        // Drop client to avoid blocking the server read
        drop(client);
        let err = read_message::<_, Request>(&mut server).await.unwrap_err();
        assert!(
            matches!(err, Error::FrameTooLarge { .. }),
            "expected FrameTooLarge, got {err:?}"
        );
    }

    #[tokio::test]
    async fn connection_closed_on_eof() {
        let (client, mut server) = duplex(64);
        // Drop the client immediately — server should see ConnectionClosed
        drop(client);
        let err = read_message::<_, Request>(&mut server).await.unwrap_err();
        assert!(
            matches!(err, Error::ConnectionClosed),
            "expected ConnectionClosed, got {err:?}"
        );
    }
}
