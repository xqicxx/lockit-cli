# lockit

**统一收口所有 secret，统一注入到任何运行环境。**

---

## 一句话定位

lockit 是一个 **本地优先、加密存储、CLI-first 的通用凭据注入层**。

不是密码管理器，不是 1Password 替代品。
是给开发者和工具链用的 **credential primitive**。

---

## 核心问题

你的 API key、cookie、token、密码散落在各处——浏览器、.env 文件、bashrc、备忘录、微信聊天记录。lockit 把它们收口到一个加密的 vault 里，然后通过一行命令注入到任何运行环境。

```
secret（API key / cookie / token / password / env var）
    ↓
lockit vault（AES-256-GCM + device key 双因素）
    ↓
注入（CLI / Android App / CI / daemon / SDK）
```

---

## 使用方式

### 存

```bash
lk add github --key token --value ghp_xxx
lk add myapp --key cookie --value "session=abc"
lk add --from-env AWS_          # 批量导入环境变量前缀匹配的
```

### 注入到命令

```bash
lk run -- myscript.sh           # 所有凭据作为 env 注入
lk run --prefix GITHUB_ -- curl ...
```

### 程序调用

```rust
use lockit_sdk::Vault;
let vault = Vault::open("~/.lockit/vault.lockit")?;
let token = vault.get("github", "token")?;
```

### 导入导出

```bash
lk import dotenv .env           # 从 .env 导入
lk export --format dotenv       # 导出为 .env
```

### 同步

```bash
lk sync push / lk sync pull     # S3 / Local（Git 后端开发中）
```

---

## 安全模型

| 层级 | 机制 |
|------|------|
| Vault 文件加密 | AES-256-GCM（AEAD，文件级完整性校验） |
| 密钥派生 | Argon2id（64MiB, 3 iter, 4 parallel） |
| 双因素 | 密码 + device key（分离存储） |
| 恢复 | BIP39 24词助记词 |
| Daemon | Unix Domain Socket IPC，15分钟无操作自动锁定 |
| 文件权限 | vault/device.key 均为 0600 |

---

## 跨平台设备安全

| 平台 | Device Key 存储 |
|------|----------------|
| Linux | 文件 `~/.lockit/device.key`, mode 0600 |
| macOS | 文件 + Keychain 保护（规划） |
| Windows | 文件 + DPAPI 加密（规划） |
| Android | Android Keystore（硬件级，不可导出） |
| iOS | Keychain + Secure Enclave（规划） |

---

## 数据模型

### 当前

```
profile: String    // "github", "openai", "myapp"
key: String        // "token", "api_key", "cookie"
value: Vec<u8>     // raw bytes, 加密存储
```

### Cookie 扩展（规划）

```
name: String
value: Vec<u8>
domain: Option<String>    // ".example.com"
path: Option<String>      // "/"
expires: Option<u64>      // Unix timestamp
httpOnly: bool
secure: bool
```

支持按 domain 精确查询：

```bash
lk get-cookies --domain .example.com
lk get-cookies --domain .example.com --name session_id
```

---

## 全栈架构

```
          lockit vault
    (AES-256-GCM + device key)
            │
   ┌────────┼────────┐
   ▼        ▼        ▼
 桌面端   Android   服务端
 CLI      App       SDK
 lk run   JNI       Rust crate
 lk get   直接读    HTTP API（规划）
```

---

## 发展阶段

### Phase 1 — 稳固基础（当前）
- [x] 核心加密存储 + CLI + daemon + IPC + S3同步
- [ ] 修复安全问题（#48-#52）
- [ ] 完善 SDK（#65）
- [ ] 完善 import/export

### Phase 2 — Cookie 专用能力
- [ ] Cookie 结构化存储
- [ ] domain/name 精确查询
- [ ] 浏览器 cookie 导入

### Phase 3 — Android
- [ ] lockit-core 编译 aarch64-linux-android
- [ ] JNI binding
- [ ] Android Keystore 替代 device key

### Phase 4 — 生态扩展
- [ ] Git 同步后端
- [ ] Python / JS FFI
- [ ] HTTP API
- [ ] Tauri GUI

---

## 核心用户画像

> 在 20 个项目之间切换，每个项目有不同的 API key / token / cookie，
> 需要统一管理、自动注入、离线可用、不依赖任何云服务。

lockit 不是给人用的，是给工具用的。
不是云优先，是本地优先。
不是 GUI 优先，是 CLI/SDK 优先。
