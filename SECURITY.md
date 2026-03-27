# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

Please do **not** file a public GitHub issue for security vulnerabilities.

Report vulnerabilities privately via **GitHub Security Advisories**:

1. Go to https://github.com/xqicxx/lockit/security/advisories
2. Click "New draft security advisory"
3. Fill in the details (description, affected versions, suggested fix if known)

We aim to acknowledge reports within 48 hours and provide a fix within 14 days for critical issues.

---

## Threat Model

### Assets

| Asset | Sensitivity | Location |
|-------|-------------|----------|
| Credential values (API keys, passwords, tokens) | Critical | vault file (encrypted) + daemon memory |
| Master Password | Critical | user's memory only; never persisted |
| Vault Encryption Key (VEK) | Critical | daemon memory only; zeroized on lock |
| Master Key (MK) | Critical | ephemeral; derived and zeroized per operation |
| Device Key | High | `~/.lockit/device.key` (0600) |
| Vault file | Medium | `~/.lockit/vault.lk` (ciphertext) |
| Profile / key names | Low | vault file (encrypted) |

### Attacker Capabilities and Mitigations

| Threat | Attacker capability | Mitigation |
|--------|---------------------|------------|
| Offline vault attack | Attacker steals `vault.lk` and `device.key` | Argon2id (64 MiB, 3 iter, p=4) makes brute-force expensive; 12+ char multi-class password required |
| Vault file only (no device key) | Attacker has `vault.lk` but not `device.key` | Dual-key architecture: both are needed; stealing one is not sufficient |
| Device key only | Attacker has `device.key` but not password | Same dual-key protection; password + device key needed for MK derivation |
| Malicious local process (same user) | Process reads daemon socket | IPC peer-UID check via `SO_PEERCRED` rejects connections from different UIDs |
| Malicious local process (different user) | Process connects to `~/.lockit/daemon.sock` | Socket created with `0600`; only owner can connect |
| Password brute-force via IPC | Repeated `UnlockVault` requests | Exponential backoff after 3 failures (10 s → 300 s cap) |
| Compromised sync backend | S3/WebDAV/Git backend is read or controlled by attacker | Zero-Knowledge: backend receives only ciphertext; MK never leaves the device |
| Memory scraping (unprivileged) | Process reads `/proc/self/mem` | All key material in `secrecy::Secret<T>`, zeroized on drop; VEK cleared on lock |
| Core dump leaking keys | SIGSEGV/OOM writes process memory | `secrecy::Secret` zeroizes on drop; AES cipher memory not yet zeroed (see Known Limitations) |
| Swap / hibernation | OS writes daemon memory to disk | Key material lives in heap; no mlocked pages yet (see Known Limitations) |
| Physical access (locked screen) | Attacker has brief physical access | Daemon auto-locks after 15 min idle; locked vault requires password to re-open |
| Physical access (powered off) | Full disk access | Same as offline vault attack above |
| Recovery phrase interception | Recovery phrase visible during `lk init` | Phrase printed to **stderr** with visual warning box; stdout only for scripts |
| Weak master password | User chooses `aaaaaaaaaaaa` | Minimum 12 chars + 2 character classes enforced at init/change-password |
| Plaintext credential leak | `lk add` writes to `~/.lockit/credentials` | Removed as of 0.1.0; plaintext export requires explicit `lk export` |

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────┐
│  TRUSTED (same UID, same process or daemon)                 │
│  - lockit daemon process                                     │
│  - lk CLI (communicates via IPC, same-UID check enforced)   │
└────────────────────────────┬────────────────────────────────┘
                             │ IPC (Unix socket, 0600, SO_PEERCRED)
┌────────────────────────────▼────────────────────────────────┐
│  SEMI-TRUSTED                                               │
│  - Sync backends (S3, WebDAV, Git): receive ciphertext only  │
│  - Filesystem: vault.lk + device.key protected by 0600      │
└────────────────────────────┬────────────────────────────────┘
                             │ encrypted blob
┌────────────────────────────▼────────────────────────────────┐
│  UNTRUSTED                                                  │
│  - Other local users (UID ≠ owner)                          │
│  - Remote attackers                                         │
│  - Compromised sync infrastructure                          │
└─────────────────────────────────────────────────────────────┘
```

### Out of Scope

- **Kernel exploits** — an attacker with kernel-level access can read any process memory.
- **Hardware attacks** — cold-boot attacks, JTAG, DMA attacks are not in scope.
- **Compromised OS** — a rootkitted system that intercepts keystrokes or system calls is out of scope.
- **Side-channel attacks** — timing attacks on AES-256-GCM are mitigated by hardware AES acceleration; cache-timing attacks are not explicitly mitigated.

---

## Security Model Overview

### Key Hierarchy

```
Master Password  +  Device Key (32-byte random, stored at ~/.lockit/device.key, 0600)
        |                   |
        +------- HKDF-SHA256 -------+
                    |
              Master Key (MK)  ← ephemeral, never persisted
                    |
           AES-256-GCM key wrap
                    |
         Vault Encryption Key (VEK)  ← in daemon memory only
                    |
         AES-256-GCM (per-entry, unique nonce)
                    |
              Credential bytes  ← in vault file as ciphertext
```

### Key Points

- **Argon2id** (64 MiB, 3 iterations, parallelism 4) makes offline brute-force expensive
- **Device Key** binds the vault to a specific device; syncing requires re-wrapping
- **VEK** never leaves daemon memory in plaintext; zeroized on `lock` or `drop`
- **Nonces** are generated internally (96-bit random, never reused, never exposed to callers)
- All key material is wrapped in `secrecy::Secret<T>` and zeroized on drop
- **IPC socket** is `0600` with `SO_PEERCRED` UID check — other-user connections rejected

### KDF Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Algorithm | Argon2id | Resistant to GPU/ASIC and side-channel attacks |
| Memory | 64 MiB | ~200 ms per attempt on modern hardware |
| Iterations | 3 | Combined with memory makes parallelism expensive |
| Parallelism | 4 | Matches typical 4-core CPU |
| Output | 32 bytes | 256-bit key |

---

## Known Limitations

| Limitation | Tracking | Notes |
|------------|----------|-------|
| AES-256-GCM cipher memory not zeroized | RustCrypto upstream (#96) | `Aes256Gcm` key schedule (176 bytes) may linger in heap until overwritten; input key is zeroized but expanded round keys are not. Disable core dumps (`ulimit -c 0`) and use encrypted swap for high-security deployments. |
| Heap memory not mlock'd | Future work | VEK/MK could be swapped to disk on memory pressure |
| Password strength check is heuristic | #57 | Character-class check added; dictionary attacks not checked |
| Windows device key file uses no ACL | #62 | NTFS default allows other local users to read; Windows DPAPI not yet used |
| No HSM / TPM integration | Future work | Device key stored as plaintext file; TPM binding would strengthen this |
