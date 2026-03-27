# lockit-sdk 功能文档

## 一、概览

lockit-sdk 是面向外部应用的集成接口。目标：让其他 Rust 程序直接调用 lockit 功能，无需解析 CLI 输出。

**当前状态：** ⚠️ 薄壳——只 re-export 了 lockit-core 的类型，没有独立功能。

---

## 二、当前公共 API

```rust
pub use lockit_core::{UnlockedVault, Error, Result, Secret};
```

SDK 目前等同于：

```toml
[dependencies]
lockit-core = { path = "crates/core" }
```

外部应用直接依赖 `lockit-core` 即可，不需要 `lockit-sdk`。

---

## 三、目标 API（规划中）

### 3.1 Vault 打开

```rust
use lockit_sdk::Vault;

let vault = Vault::open("~/.lockit/vault.lockit")?;
// 内部读取密码？当前未设计——需要从 CLI / environment / keyring 获取
```

**待解决：** SDK 如何获取密码？

- 方案 A：从环境变量 `LOCKIT_PASSWORD` 读取
- 方案 B：调用 `lk unlock` 通过 IPC 获取已解锁的 vault
- 方案 C：直接传入密码参数

### 3.2 凭据查询

```rust
let token: String = vault.get_string("github", "token")?;
let raw: Vec<u8>  = vault.get_bytes("openai", "api_key")?;
let exists: bool  = vault.contains("myapp", "password")?;
let profiles: Vec<String> = vault.profiles()?;
let keys: Vec<String>     = vault.keys("myapp")?;
```

### 3.3 配合 daemon

```rust
// 通过 IPC 连接 daemon，不需要本地 vault 文件
let client = LockitClient::connect_default().await?;
let token = client.get_credential("github", "token").await?;
```

---

## 四、与其他语言的 FFI 桥接

SDK 是 FFI 的基础层：

```
lockit-sdk (Rust)
    ↓
#[no_mangle] extern "C" 函数
    ↓
┌───────────────────────────────────────┐
│ C ABI 桥接                             │
│ Python ctypes / PyO3                   │
│ JNI (Java/Android)                    │
│ wasm-bindgen (Web/JavaScript)          │
│ UniFFI (Kotlin/Swift/Python)           │
└───────────────────────────────────────┘
```

### 4.1 Python FFI（规划中）

```python
import lockit

vault = lockit.Vault.open("~/.lockit/vault.lockit")
token = vault.get_string("github", "token")
```

### 4.2 JavaScript/WASM（规划中）

```javascript
import { Vault } from 'lockit-wasm';

const vault = await Vault.open("~/.lockit/vault.lockit");
const token = await vault.getString("github", "token");
```

### 4.3 JNI/Android（规划中）

```java
// Kotlin
val vault = LockitVault.open("~/.lockit/vault.lockit")
val token = vault.getString("github", "token")
```

---

## 五、安全约束

1. **不直接暴露加密细节**：外部代码不能调用 KDF、加密、解密函数
2. **Secret 返回值**：`get()` 返回 `Secret<Vec<u8>>`，drop 时自动清零
3. **不缓存明文**：SDK 不在内存中保存明文凭据（需要时从 vault 解密）
4. **device key 管理**：SDK 需要获取 device key——通过 IPC 或从默认路径读取

---

## 六、依赖

| 依赖 | 用途 |
|------|------|
| lockit-core | 加密核心（透传） |
| lockit-ipc | daemon 通信（规划中） |

**当前依赖：** 只有 lockit-core，lockit-ipc 还未集成。

---

## 七、目标用户

1. **Python 脚本**：`import lockit` 获取环境变量
2. **Node.js 服务**：`require('lockit')` 获取数据库凭据
3. **Android App**：JNI 调用 lockit 获取 cookie/token
4. **Tauri 桌面应用**：Rust 后端直接依赖 lockit-sdk
5. **CI/CD 插件**：GitHub Actions / GitLab CI 插件

---

## 八、集成示例

### 8.1 Rust 项目直接依赖 lockit-core

```toml
[dependencies]
lockit-core = { git = "https://github.com/xqicxx/lockit", package = "lockit-core" }
```

不需要 SDK，直接用 core。

### 8.2 通过 CLI（跨语言通用）

```python
import subprocess
token = subprocess.check_output(["lk", "get", "github", "token"]).decode().strip()
```

### 8.3 通过 daemon IPC（规划中）

```rust
// connect to running daemon, get credential
let client = IpcClient::new_default().await?;
let response = client.send_request(&Request::GetCredential {
    profile: "github".into(),
    key: "token".into(),
}).await?;
```
