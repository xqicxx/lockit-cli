# Architecture

Detailed cryptographic architecture for lockit.

## Key Hierarchy

```
User input:   Master Password (UTF-8 string)
Device:       Device Key (32 random bytes, ~/.lockit/device.key)

Step 1 — Argon2id KDF
  Input:  Master Password + Salt (16 random bytes, stored in vault header)
  Params: memory=64 MiB, iterations=3, parallelism=4, output=32 bytes
  Output: Argon2 Output Key (AOK)

Step 2 — HKDF-SHA256 key derivation
  IKM:    AOK + Device Key (concatenated)
  Salt:   (none / zero)
  Info:   "lockit-master-key"
  Output: Master Key (MK, 32 bytes)

Step 3 — VEK wrapping
  The Vault Encryption Key (VEK) is a 32-byte random key generated at vault init.
  It is wrapped (encrypted) with MK using AES-256-GCM + a random 12-byte nonce.
  The wrapped VEK is stored in the vault header.

Step 4 — Credential encryption
  Each credential value is encrypted individually:
    ciphertext = AES-256-GCM(key=VEK, nonce=random_12_bytes, plaintext=value)
  The nonce is prepended to the ciphertext and stored in the vault.
```

## Vault File Format

The vault file is MessagePack-serialized with the following layout:

```
Bytes 0-7:   Magic header  b"LOCKIT01"
Bytes 8-9:   Version       u16 little-endian  (currently 1)
Bytes 10+:   MessagePack-encoded VaultData struct:
  - salt:        [u8; 16]   Argon2id salt
  - wrapped_vek: Vec<u8>    nonce (12 bytes) + AES-GCM ciphertext of VEK
  - entries:     HashMap<profile, HashMap<key, EncryptedEntry>>
    - EncryptedEntry:
        - nonce:      [u8; 12]
        - ciphertext: Vec<u8>
```

## KDF Parameters

| Parameter   | Value        | Rationale                              |
|-------------|--------------|----------------------------------------|
| Algorithm   | Argon2id     | Resistant to both CPU and GPU attacks  |
| Memory      | 64 MiB       | Makes parallel attacks expensive       |
| Iterations  | 3            | Increases time cost                    |
| Parallelism | 4            | Matches typical CPU core count         |
| Output      | 32 bytes     | 256-bit key for AES-256-GCM            |
| Salt size   | 16 bytes     | 128 bits of randomness                 |

## Crate Boundaries

```
lockit-core   (private)
  cipher.rs   AES-256-GCM key wrapping and credential encryption
  kdf.rs      Argon2id + HKDF-SHA256 key derivation
  memory.rs   VaultEncryptionKey wrapper with Secret<[u8;32]>
  vault.rs    UnlockedVault — the single public type

lockit-ipc    (internal)
  proto.rs    Request / Response message types (rmp-serde)
  framing.rs  Length-prefixed message framing
  client.rs   Async IPC client
  server.rs   Async IPC server + RequestHandler trait

lockit-sync   (optional)
  backend.rs  SyncBackend trait
  backends/   local, mock, S3
  config.rs   BackendConfig (serde, TOML-deserializable)
  factory.rs  SyncBackendFactory

lockit-sdk    (public library)
  lib.rs      LockitClient — synchronous wrapper around IpcClient
```

## Daemon Architecture

The daemon (started by `lk daemon start`) holds the unlocked VEK in memory.
Clients communicate over a Unix Domain Socket (`~/.lockit/lockit.sock`).
Each request opens a new connection, sends one message, receives one response,
and closes. The daemon auto-locks after a configurable idle timeout.

```
lk get myapp api_key
  |
  +-> IpcClient::send_request(GetCredential { profile: "myapp", key: "api_key" })
        |
        +-> daemon: RequestHandler::handle(req)
              |
              +-> vault.get("myapp", "api_key")  (VEK in memory, no disk I/O)
              |
              +-> Response::Value { value: Some(b"...") }
```
