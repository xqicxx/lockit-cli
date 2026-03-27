# P1 Security Fixes Design

> **Goal:** 实现全部 5 个 p1 级别的 GitHub issues，分批处理确保质量

**Created:** 2026-03-26
**Status:** approved

---

## Overview

分批实现 5 个 p1 issues，每个 issue 独立 PR：

| Phase | Issue | Description |
|-------|-------|-------------|
| 1 | #100 | shell_quote 模块化 + 注入测试 |
| 2 | #95 | Recovery phrase 输出到 stderr |
| 3 | #94 | IPC socket 权限 0600 |
| 4 | #96 | SECURITY.md 文档更新 |
| 5 | #9 | 生物识别模块（单独设计） |

---

## Phase 1: shell_quote 安全修复 (Issue #100)

### 架构

将 `shell_quote()` 从 `main.rs` 提取到独立模块 `shell.rs`：

```
crates/cli/src/
├── main.rs          # 移除 shell_quote，添加 mod shell
├── shell.rs         # 新模块：shell_quote() + tests
```

### 测试用例

```rust
#[test]
fn test_injection_cases() {
    // 命令注入防护
    assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
    assert_eq!(shell_quote("`cat /etc/passwd`"), "'`cat /etc/passwd`'");
    assert_eq!(shell_quote("; ls"), "'; ls'");
    assert_eq!(shell_quote("| cat"), "'| cat'");
    assert_eq!(shell_quote("& whoami"), "'& whoami'");

    // 单引号转义
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
    assert_eq!(shell_quote("foo'"), "'foo'\\''");
    assert_eq!(shell_quote("'''"), "''\\'''\\'''\\''");

    // 边界情况
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("simple"), "simple");
    assert_eq!(shell_quote("hello world"), "'hello world'");
}
```

---

## Phase 2: Recovery phrase stderr (Issue #95)

### 修改

将 `lk add` 自动创建 vault 时的助记词从 `println!` 改为 `eprintln!`：

```rust
// Before
println!("(New vault created. Recovery phrase: {})", mnemonic);

// After
eprintln!();
eprintln!("⚠️  WARNING: New vault created");
eprintln!("Recovery phrase (write it down and store safely):");
eprintln!("{}", mnemonic);
eprintln!("⚠️  This phrase will NOT be shown again.");
```

---

## Phase 3: IPC socket 权限 (Issue #94)

### 修改

在 `IpcServer::bind()` 后添加权限设置：

```rust
pub fn bind(path: PathBuf) -> Result<Self> {
    // ... existing code ...
    let listener = UnixListener::bind(&path)?;

    // 显式设置 socket 文件权限为 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    // ... rest of code ...
}
```

---

## Phase 4: AES cipher zeroize 文档 (Issue #96)

### 创建 SECURITY.md

```markdown
# Security Considerations

## AES-256-GCM Cipher Memory

### Limitation

`Aes256Gcm::new_from_slice(key)` creates a cipher object on the heap
that holds an internal copy of the key. The `Aes256Gcm` type does not
implement `Zeroize`, meaning the key material may remain in memory
after the cipher is dropped.

### Risk Assessment

- **Core dumps**: Key material may be recoverable from crash dumps
- **Memory swap**: Keys may persist on disk if memory is swapped
- **Expanded keys**: AES-256 uses 176 bytes for round keys (larger than original key)

### Current Mitigation

1. The input `key_bytes` is wrapped in `Secret<[u8; 32]>` and zeroized
2. The cipher lifetime is minimized (created, used, dropped immediately)
3. We are tracking [RustCrypto/AEADs issue #XXX](https://github.com/RustCrypto/AEADs/issues/...)

### Future Improvement

When RustCrypto adds `Zeroize` support to `Aes256Gcm`, we will upgrade
and remove this limitation.

---

## Next Steps

1. Track upstream progress on `Zeroize` for `Aes256Gcm`
2. Evaluate alternative AEAD implementations if needed
3. Consider using hardware security modules (HSM) for high-security deployments
```

---

## Phase 5: 生物识别模块 (Issue #9)

**单独设计** - 这是大型功能模块，需要独立的 spec。