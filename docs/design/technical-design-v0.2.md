# lockit

统一凭证管理系统

技术设计文档 v0.2

2026 年 3 月

> **状态：** 草稿 · 内部审阅
> **版本：** 0.2.0
> **适用平台：** macOS · Windows · Linux · CLI

---

## 目录

1. [项目概述](#1-项目概述)
2. [竞品对标与差异化定位](#2-竞品对标与差异化定位)
3. [安全架构](#3-安全架构)
4. [系统架构](#4-系统架构)
5. [同步引擎设计](#5-同步引擎设计)
6. [核心数据流](#6-核心数据流)
7. [开发路线图](#7-开发路线图)
8. [合规与隐私声明](#8-合规与隐私声明)
9. [附录：关键术语](#9-附录关键术语)

---

## 1. 项目概述

### 1.1 背景与动机

现代开发者、工程师、产品团队日常面对的凭证数量已超出人脑记忆上限：API Key、OAuth Token、Session Cookie、数据库密码、SSH 私钥……每换一台设备或进入新的工作环境，就必须重新收集、重新粘贴这些凭证，不仅低效，还极易因明文传递或弱存储方式引发安全事故。

**lockit** 的目标是成为开发者的「凭证基础设施」：一次录入，全平台读取，加密存储，Zero-Knowledge 同步，彻底消灭凭证碎片化。

### 1.2 核心价值主张

- **一行命令替代 .env：** `lk get KEY`、`lk run -- cmd`、`lk inject template.env`，零配置启动。
- **Zero-Knowledge 同步：** 本地 AES-256-GCM 加密后上传，同步后端（S3 兼容 / Git / WebDAV / P2P / iCloud）永远只看到密文。
- **双密钥安全架构：** Master Password + Device Key（设备密钥），对标 1Password 但更轻量。
- **国内云原生支持：** 阿里云 OSS / 腾讯云 COS / 华为云 OBS 等 S3 兼容存储开箱即用。
- **极致轻量：** Rust 单二进制，零依赖，5 秒从安装到首次使用。

### 1.3 目标用户

| 用户类型 | 典型使用场景 |
|---------|-------------|
| 独立开发者 | 管理多项目的 API Key、数据库密码、第三方 Token |
| DevOps / SRE | CI/CD 凭证注入、多环境切换、团队 Secret 共享 |
| 安全研究员 | 隔离不同客户的凭证，防止越界访问 |
| AI 工具重度用户 | 管理多个 AI 平台 API Key，如 OpenClaw、Claude、Gemini |

---

## 2. 竞品对标与差异化定位

### 2.1 竞品全景图

| 产品 | CLI 一行读取 | Env Inject | Zero-Knowledge | 开源 | 自托管 | 国内云支持 | 定价 |
|------|------------|------------|---------------|------|--------|----------|------|
| 1Password `op` | ✅ `op read` | ✅ `op run` | ✅ 2SKD+SRP | ❌ | ❌ | ❌ | $2.99/月 |
| Doppler | ✅ `doppler get` | ✅ `doppler run` | ❌ | ❌ | ❌ | ❌ | 免费（3用户） |
| Infisical | ✅ `infisical get` | ✅ `infisical run` | ❌ 服务端解密 | ✅ MIT | ✅ 3容器 | ❌ | 免费（5 identity） |
| Bitwarden SM | ✅ `bws get` | ✅ `bws run` | ✅ E2E | ✅ | ✅ | ❌ | 免费（2用户） |
| pass | ✅ `pass show` | ❌ | ✅ 本地GPG | ✅ | ✅ 纯本地 | ❌ Git | 免费 |
| dotenvx | ✅ `dotenvx get` | ✅ `dotenvx run` | ✅ 本地加密 | ✅ | ✅ 纯本地 | ❌ | 免费 |
| **lockit** | ✅ `lk get` | ✅ `lk run` | ✅ MP+DeviceKey | ✅ MIT | ✅ 零依赖 | ✅ **原生支持** | 免费（个人） |

### 2.2 市场空白

1. **轻量级 CLI-first 工具缺失** — dotenvx 周安装量 300 万次证明需求真实存在，但现有工具要么太重（Vault）、要么太贵（1Password）、要么锁定 SaaS（Doppler）。
2. **中国市场完全空白** — 没有任何主流工具原生支持国内云存储（阿里云 OSS / 腾讯云 COS / 华为云 OBS）。
3. **可插拔同步 + Zero-Knowledge 组合不存在** — 1Password/Bitwarden 锁定自有云，pass 仅 Git，KeePass 缺现代 CLI。

### 2.3 lockit 差异化

| 维度 | 1Password | Doppler | pass | **lockit** |
|------|-----------|---------|------|-----------|
| 轻量 | ❌ 重型客户端 | ❌ SaaS 依赖 | ✅ bash 脚本 | ✅ **Rust 单二进制** |
| 速度 | ❌ `op read` 需认证 | ✅ 快 | ✅ 快 | ✅ **本地优先，毫秒响应** |
| 开源 | ❌ | ❌ | ✅ | ✅ **MIT** |
| 自托管 | ❌ | ❌ | ✅ | ✅ **零依赖** |
| Zero-Knowledge | ✅ | ❌ | ✅ | ✅ **双密钥架构** |
| 国内云 | ❌ | ❌ | ❌ | ✅ **原生 S3 兼容** |

**核心价值主张：**「一行命令替代 .env 文件，零信任同步到任何存储后端」。

---

## 3. 安全架构

### 3.1 设计原则

- **Zero-Knowledge：** 同步后端永远无法解密用户数据，密钥派生和解密只发生在用户设备本地。
- **双密钥架构：** Master Password + Device Key（设备密钥）混合派生，对标 1Password Secret Key 但更轻量——设备密钥自动生成存储于系统 Keychain，无需用户备份。
- **密钥分层（Key Hierarchy）：** 用户密码不直接加密数据，而是加密中间密钥 VEK；变更密码只需重新包裹 VEK，数据体不受影响。
- **内存安全：** VEK 明文只存在于进程内存（mlock 锁定），超时自动清除，绝不写入磁盘。
- **可恢复性：** 通过 BIP39 24 词助记恢复密钥提供兜底恢复路径，忘记主密码不丢数据。

### 3.2 密钥层级结构

整个加密系统分为四层：

#### 第一层：解锁层（User Unlock）

用户通过生物识别（Touch ID / Face ID）或主密码解锁 Vault。生物识别本身不参与加密运算，其作用是授权操作系统安全飞地（Secure Enclave / Keychain / Android StrongBox）释放已缓存的密钥材料。

#### 第二层：密钥派生层（Key Derivation）

**双密钥混合派生**（借鉴 1Password Secret Key 思路，但更轻量）：

```
Master Password + Salt
        │
        ▼
   Argon2id (64 MiB, 3 iter, 4 threads)
        │
        ▼
   Password Key (256 bit)

   Device Key (随机 256 bit，设备初始化时生成，存于系统 Keychain)
        │
        ▼
   HKDF-SHA256(Password Key ∥ Device Key)
        │
        ▼
   Master Key (MK, 256 bit)
```

**Argon2id 参数：**

| 参数 | 值 |
|------|-----|
| 算法 | Argon2id（抗侧信道 + 抗 GPU 暴力） |
| 内存用量 | 64 MB（memory-hard，提升 GPU 攻击成本） |
| 迭代次数 | 3 |
| 并行度 | 4 |
| 输出长度 | 32 bytes（256 bit） |
| Salt 长度 | 16 bytes（128 bit），随机生成，明文存储 |

**Device Key 与 1Password Secret Key 的区别：**

| 维度 | 1Password Secret Key | lockit Device Key |
|------|---------------------|-------------------|
| 生成时机 | 注册时随机生成 | 设备初始化时随机生成 |
| 存储位置 | 用户需手动备份 Emergency Kit | 系统 Keychain / Secure Enclave，自动管理 |
| 恢复方式 | 必须有备份才能新设备登录 | 已授权设备可签发新 Device Key（QR 码配对） |
| 用户负担 | 高（必须保管 34 字符） | 低（无感管理） |

#### 第三层：数据加密层（Vault Encryption Key）

Vault Encryption Key（VEK）是随机生成的 256 bit 密钥，是真正用于加密凭证数据的密钥。VEK 本身以 AES-256-GCM 被 MK 加密后存储（称为「加密态 VEK」）；解锁后，VEK 明文仅驻留内存（mlock 锁定），从不落盘。

改密码时只需用新 MK 重新包裹 VEK，全部凭证数据保持不变——变更密码开销从 O(n) 降至 O(1)。

恢复密钥（24 词 BIP39 助记词）是对 VEK 的独立加密备份，与主密码完全解耦。

#### 第四层：存储层（Encrypted Storage）

每条凭证条目独立使用 AES-256-GCM 加密，具有独立的 96 bit 随机 Nonce 和 128 bit Auth Tag，防止不同条目间的密文重用与篡改检测旁路。**Secret 名称、标签等元数据也加密**（避免 pass 的文件名泄露问题）。

### 3.3 威胁模型

| 威胁 | 缓解措施 | 状态 |
|------|---------|------|
| 攻击者拖库（云端） | 数据全程加密，服务器无密钥 | ✅ 已缓解 |
| 暴力破解主密码 | Argon2id 64MB 内存硬度 + Device Key 双因素 | ✅ 已缓解 |
| 设备丢失 | 本地文件 AES-256-GCM 加密，无主密码无法读取 | ✅ 已缓解 |
| 忘记主密码 | BIP39 24 词恢复密钥 | ✅ 已缓解 |
| 内存转储攻击 | VEK mlock 锁定 + 超时清除 | ⚠️ 部分缓解 |
| 供应链攻击 | Rust 编写，依赖最小化，发布签名验证 | 🔄 进行中 |
| USENIX '26 攻击 | 密文认证（AES-GCM Auth Tag）+ 密钥绑定 | ✅ 已缓解 |

> **USENIX Security '26 参考：** ETH Zurich 研究者发现 1Password、Bitwarden 等产品的 27 种攻击，包括 RSA-OAEP 密文缺乏认证、字段独立加密导致 cut-and-paste 攻击。lockit 的 AES-256-GCM AEAD 加密 + 双密钥绑定设计从架构层面规避这些问题。

### 3.4 零知识证明

| 云端存储内容（服务器可见） | 云端不存储（服务器不可见） |
|-------------------------|------------------------|
| 加密密文、Salt（明文） | 主密码、MK、VEK 明文 |
| 加密 VEK、版本号、时间戳 | Device Key、任何凭证内容 |

---

## 4. 系统架构

### 4.1 整体架构

lockit 采用「本地优先（Local-First）」架构：所有读写操作均在本地完成，云端同步是可选的异步副作用。

```
┌─────────────────────────────────────────────────────┐
│                   客户端层                           │
│  ┌──────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ CLI (lk) │  │ Desktop(Tauri)│  │ Mobile(Flutter)│  │
│  └────┬─────┘  └──────┬───────┘  └───────┬───────┘  │
│       │               │                  │          │
│       └───────────────┼──────────────────┘          │
│                       │ IPC (Unix Socket / Named Pipe)│
│                       ▼                              │
│              ┌─────────────────┐                     │
│              │ lockit-daemon   │  ← VEK 驻留内存     │
│              │ (守护进程)      │                     │
│              └────────┬────────┘                     │
│                       │                              │
│       ┌───────────────┼───────────────┐              │
│       ▼               ▼               ▼              │
│ ┌──────────┐   ┌───────────┐   ┌──────────┐         │
│ │lockit-core│   │lockit-sync│   │lockit-sdk│         │
│ │(加密引擎) │   │(同步引擎) │   │(第三方接入)│         │
│ └──────────┘   └───────────┘   └──────────┘         │
└─────────────────────────────────────────────────────┘
```

- **lockit-core：** Rust 编写的加密存储引擎，提供跨平台静态链接库和 FFI 接口。
- **lockit-daemon：** 后台常驻进程，持有解锁后的 VEK 明文，响应各客户端查询请求。
- **lockit-sync：** 可插拔同步引擎，支持 S3 兼容 / Git / WebDAV / P2P 后端。
- **lockit-sdk：** 第三方工具接入 SDK（Rust / Python / Node.js）。
- **客户端层：** CLI（lk）、桌面 GUI（Tauri）、移动端（Flutter）。

### 4.2 凭证文件格式

磁盘上的凭证文件采用类 AWS CLI 的 INI 风格，但以加密形式存储。明文结构示例：

```ini
[openclaw]
api_key = sk-xxxxxxxxxxxxxxxxxxxxxxxx
model = claude-sonnet-4-6

[github]
token = ghp_xxxxxxxxxxxxxxxxxxxxxxxx
username = yourname

[aws]
access_key_id = AKIAXXXXXXXXXXXXXXXX
secret_access_key = xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
region = us-west-2
```

实际存储在磁盘的是以上内容经 VEK 加密后的密文包，读取时由 Daemon 在内存中解密后返回。

### 4.3 CLI 工具（lk）

lk 是面向开发者的核心接入点：零配置启动，一行命令读取任意凭证。

**常用命令：**

```bash
# 首次初始化
lk init

# 添加凭证
lk add openclaw --key sk-xxxxx
lk add github --token ghp_xxxxx --username yourname

# 读取凭证（返回值）
lk get openclaw api_key

# 以环境变量形式注入并运行命令
lk run --profile openclaw -- openclaw chat

# 列出所有 profile
lk list

# 同步
lk sync
```

**第三方工具接入方式（无需修改源码）：**

| 方式 | 说明 | 适用场景 |
|------|------|---------|
| 配置文件读取 | 工具启动时自动查找 `~/.lockit/credentials` | 静态凭证（推荐） |
| 环境变量注入 | `lk run --profile <name>` 自动导出 | 运行时注入 |
| SDK 直接调用 | 引入 lockit-sdk，通过 IPC 查询 | 动态刷新 Token |

### 4.4 跨端技术选型

| 组件 | 技术 | 选型理由 |
|------|------|---------|
| 加密核心 | Rust + RustCrypto | 内存安全，无 GC 停顿 |
| CLI 工具 | Rust + clap | 单二进制，零依赖，跨平台 |
| 守护进程 | Rust | 常驻内存安全 |
| 同步引擎 | Rust + rusoto（S3）/ git2（Git） | 可插拔，高性能 |
| 桌面端 | Tauri (Rust + Web) | 轻量（~10MB），跨平台 |
| 移动端 | Flutter | iOS + Android 共享代码 |
| IPC 通信 | Unix Socket / Named Pipe | 本地通信，无网络暴露面 |

---

## 5. 同步引擎设计

### 5.1 架构原则：加密层与传输层分离

同步引擎遵循**「加密层与传输层严格分离」**：加密层确保 Zero-Knowledge（所有加密解密在客户端完成），传输层仅负责搬运不透明的密文 blob。同步后端可自由替换而不影响安全性。

### 5.2 SyncBackend Trait 接口

```rust
#[async_trait]
pub trait SyncBackend {
    async fn push(&self, blob: &[u8], version: u64) -> Result<()>;
    async fn pull(&self) -> Result<(Vec<u8>, u64)>;
    async fn list(&self) -> Result<Vec<SyncEntry>>;
    fn name(&self) -> &str;
}
```

所有同步后端实现此 trait，核心逻辑与传输解耦。

### 5.3 同步后端优先级

#### 第一优先级（MVP）

| 后端 | 说明 | 关键实现 |
|------|------|---------|
| **本地文件系统** | `~/.lockit/vault.enc` | 可被 Syncthing / iCloud Drive / Dropbox 自动同步 |
| **S3 兼容存储** | 标准 S3 API | 支持 AWS S3、阿里云 OSS、腾讯云 COS、华为云 OBS、MinIO |

**国内云 S3 兼容性处理：**

| 云服务 | 兼容度 | 需处理差异 |
|--------|--------|-----------|
| 阿里云 OSS | ~90% | 需 V2 签名，ETag 大小写不同 |
| 腾讯云 COS | 高 | 需配置 APPID/Endpoint |
| 华为云 OBS | 中高 | 仅 Path-style |
| 七牛云 Kodo | 中高 | 推荐原生接口 |

#### 第二优先级

| 后端 | 说明 |
|------|------|
| **Git 后端** | 加密文件存 Git 仓库，`lk sync` ≈ `git push/pull`，兼容 GitHub / Gitee |
| **WebDAV** | 对接坚果云、Nextcloud |

#### 第三优先级

| 后端 | 说明 |
|------|------|
| **P2P 直连** | QUIC + Noise Protocol，设备间直连，无需中心服务器 |
| **iCloud CloudKit** | macOS/iOS 原生集成 |

### 5.4 冲突解决

采用 **Record-level Last-Write-Wins + 软删除 + 版本历史**：

- 每条 Secret 独立加密存储，包含 UUID + 修改时间戳 + 版本号
- 同步时按 UUID 合并，时间戳较新者覆盖
- 删除操作标记为 tombstone（保留 30 天），避免删除/修改冲突
- 保留每条 Secret 最近 10 个版本，支持回滚

### 5.5 Zero-Knowledge 保障约束

- **所有加密/解密仅在客户端完成**，同步后端仅存储和传输密文
- **Master Password 和 Device Key 永不离开设备**
- **使用 AEAD 加密**（AES-256-GCM），防止密文被篡改
- **元数据也加密**（Secret 名称、标签）
- **新设备加入**：通过已信任设备扫描 QR 码授权，签发新 Device Key

---

## 6. 核心数据流

### 6.1 首次初始化

```
lk init
    │
    ├─ 生成随机 Salt（128 bit）
    ├─ 生成随机 Device Key（256 bit）→ 存入系统 Keychain
    ├─ Argon2id(主密码, Salt) → Password Key
    ├─ HKDF(Password Key ∥ Device Key) → Master Key (MK)
    ├─ 随机生成 VEK（256 bit）
    ├─ AES-GCM(MK, VEK) → 加密态 VEK，写入本地文件
    ├─ 生成 BIP39 24 词助记恢复密钥 → 展示给用户（仅一次）
    └─ 注册生物识别密钥引用至系统安全飞地
```

### 6.2 日常解锁

```
生物识别 / 输入主密码
    │
    ├─ 生物识别：安全飞地返回授权 → Daemon 从 Keychain 取出缓存的 MK/VEK
    └─ 主密码：Argon2id 派生 Password Key → HKDF(PW ∥ Device Key) → MK → 解包 VEK
    │
    ├─ VEK 存入内存（mlock），15 分钟无活动自动清除
    └─ 后续 CLI/GUI 查询走 IPC，VEK 不离内存
```

### 6.3 跨设备同步

```
新设备 lk init → 从云端下载加密密文包
    │
    ├─ 输入主密码 → 派生 MK → 解包 VEK → 解密本地文件
    └─ 首次同步完成
    │
后续修改 → 增量同步，冲突按「时间戳最新优先」合并
    │
云端始终只有密文，即使云账号被攻陷，凭证数据依然安全
```

---

## 7. 开发路线图

> **优先原则：** 先把 CLI + 本地加密做到极致，再向外扩展。一个稳定可靠的 MVP 比五个平台同时烂要好得多。

| 阶段 | 交付内容 | 对应 Issue |
|------|---------|-----------|
| **MVP** | lockit-core（KDF + Cipher + Vault 格式 + mlock）+ lockit-cli（init/add/get/list） | #2-6, #11-13 |
| **第二阶段** | lockit-daemon（IPC + 生物识别接入） | #7-9 |
| **第三阶段** | lockit-sync（SyncBackend trait + S3 兼容后端 + 增量同步） | #15-17 |
| **第四阶段** | lockit-sdk（credentials 格式规范 + Rust/Python/Node SDK）+ `lk run` | #14, #18-19 |
| **第五阶段** | 桌面 GUI（Tauri）+ 恢复密钥流程（BIP39） | #20 |
| **第六阶段** | README + 贡献指南 + 项目文档 | #21 |

---

## 8. 合规与隐私声明

- **数据最小化：** lockit 不收集任何遥测数据、不上报使用情况、不发送匿名统计。
- **Zero-Knowledge：** 同步后端（若启用）无法访问用户凭证内容，技术上无法做到，而非仅靠承诺。
- **开源可审计：** 核心加密库和协议实现以 MIT 协议开源，接受社区安全审计。
- **本地优先：** 云同步是可选功能；不使用云同步的用户，数据永远不离开本地设备。

---

## 9. 附录：关键术语

| 术语 | 含义 |
|------|------|
| MK（Master Key） | 由 Master Password + Device Key 经 HKDF 混合派生的 256 bit 密钥 |
| VEK（Vault Encryption Key） | 随机生成的 256 bit 密钥，真正用于加密凭证数据，仅驻留内存 |
| Device Key | 设备初始化时随机生成的 256 bit 密钥，存于系统 Keychain，无感管理 |
| Salt | 128 bit 随机值，与主密码共同输入 Argon2id，防止彩虹表攻击 |
| Argon2id | 2015 PHC 冠军，兼具时间硬度与内存硬度的密钥派生函数 |
| AES-256-GCM | 对称加密算法，提供认证加密（AEAD） |
| Secure Enclave | Apple/ARM 芯片中的独立安全处理器，存储敏感密钥材料 |
| BIP39 | 24 词助记词标准，对应 256 bit 熵，用于恢复密钥备份 |
| Zero-Knowledge | 同步后端设计上无法获知用户数据内容的架构模式 |
| SyncBackend | lockit 同步引擎的可插拔后端 trait 接口 |

---

*文档结束*
