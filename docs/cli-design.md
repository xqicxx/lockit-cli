# Lockit CLI 实现设计

> 基于现有 Android 架构 + 竞品调研

---

## 一、现有架构分析

### Android 端现状

```
┌─────────────────────────────────────────────┐
│              Lockit Android                  │
├─────────────────────────────────────────────┤
│                                              │
│  VaultManager (Kotlin)                      │
│  ├── initVault(masterPassword)              │
│  ├── unlockVault(masterPassword)            │
│  ├── lockVault()                            │
│  ├── addCredential()                        │
│  ├── getCredentialById()                    │
│  ├── searchCredentials(query)               │
│  ├── changePassword(old, new)               │
│  └── upgradeArgon2Params()                  │
│                                              │
│  KeyManager                                 │
│  ├── 主密钥 = Argon2id(PIN, salt, 64MB, 3)  │
│  ├── SHA-256 hash 验证                      │
│  └── 内存中保留 masterKey（解锁状态）         │
│                                              │
│  LockitCrypto                               │
│  └── AES-256-GCM 加密/解密                   │
│                                              │
│  CredentialDao (Room)                       │
│  └── SQLite: credentials 表                  │
│      id, name, type, service, key,          │
│      value(加密), metadata, createdAt,       │
│      updatedAt                               │
│                                              │
│  GoogleDriveSyncManager                     │
│  ├── uploadVault(db)  → 整个 .db 上传       │
│  └── getLastBackupTime()                    │
│                                              │
│  CodingPlanFetchers                         │
│  ├── QwenCodingPlan (百炼)                   │
│  ├── ChatGPTCodingPlan                      │
│  └── ClaudeCodingPlan                       │
│                                              │
│  BiometricPinStorage                        │
│  └── AndroidKeyStore + BiometricPrompt      │
│                                              │
│  AuditLogger                                │
│  └── 审计日志（创建/查看/修改/删除）            │
│                                              │
│  SearchMatcher                              │
│  ├── 拼音搜索（TinyPinyin）                   │
│  ├── Levenshtein 容错                       │
│  └── 匹配评分排序                            │
└─────────────────────────────────────────────┘
```

### 核心原则

1. **云端只存密文** — Google Drive 存的是加密后的 .db 文件
2. **加解密全在本地** — 主密钥只在内存，不上云
3. **PIN 派生主密钥** — Argon2id(64MB, 3 iterations, 4 parallelism)
4. **AES-256-GCM** — 每个字段独立加密

---

## 二、CLI 架构设计

### 核心思路

CLI 与 App **共享同一 vault.db 文件格式**，但用 Rust 实现（跨平台、性能好、内存安全）。

```
┌─────────────────────────────────────────────────┐
│                  Lockit CLI                      │
├─────────────────────────────────────────────────┤
│                                                  │
│  CLI (clap)                                     │
│  ├── lk signin          # 解锁 vault              │
│  ├── lk whoami          # 查看状态               │
│  ├── lk get <item>      # 读取凭据               │
│  ├── lk list            # 列出凭据               │
│  ├── lk read "lk://..." # Secret Reference       │
│  ├── lk run -- 'cmd'    # Agent 注入             │
│  └── lk sync            # 云盘同步               │
│                                                  │
│  Vault Core (Rust)                              │
│  ├── vault.db 解析（SQLite）                     │
│  ├── Argon2id 密钥派生                           │
│  ├── AES-256-GCM 解密                           │
│  ├── Credential CRUD                            │
│  └── Search (拼音 + 模糊)                        │
│                                                  │
│  Sync Backends                                  │
│  ├── Google Drive (OAuth2)                      │
│  ├── S3 兼容 (AWS SDK)                          │
│  ├── WebDAV (坚果云/Nextcloud)                   │
│  └── 本地文件                                    │
│                                                  │
│  Auth                                           │
│  ├── PIN 解锁（终端输入）                         │
│  ├── Agent Token（JWT，限定 scope）              │
│  └── 生物识别（macOS Touch ID，可选）             │
│                                                  │
│  Agent Protocol                                 │
│  ├── lk:// URI 协议                             │
│  ├── JSON 输出（--format json）                  │
│  └── MCP Server 模式                             │
│                                                  │
│  Config (~/.config/lockit/)                     │
│  ├── config.toml         # 配置                 │
│  ├── vault.db            # 本地 vault            │
│  ├── tokens/             # Agent tokens          │
│  └── sync/               # 同步状态              │
└─────────────────────────────────────────────────┘
```

---

## 三、命令设计

### 3.1 认证命令

```bash
# 初始化 vault（首次使用）
lk init
# 提示：输入主 PIN（至少 6 位）
# 创建 ~/.config/lockit/vault.db

# 解锁 vault
lk signin
# 提示：输入 PIN
# 成功后 masterKey 保留在内存中（session 有效期内）

# 检查状态
lk whoami
# 输出:
#   Vault: ~/.config/lockit/vault.db
#   Status: unlocked
#   Credentials: 12
#   Providers: qwen_bailian, chatgpt, claude
#   Last sync: 2026-04-22 10:30 (Google Drive)

# 锁定 vault（清除内存中的 masterKey）
lk lock

# 修改 PIN
lk change-pin
```

### 3.2 读取凭据

```bash
# 列出所有凭据
lk list

# 按类型过滤
lk list --type CodingPlan
lk list --type GitHub
lk list --type Email

# 搜索
lk list --search "bailian"
lk list --search "github"

# 读取单个凭据
lk get "百炼"
# 输出（默认 human-readable）:
# Name: 百炼
# Type: CodingPlan
# Provider: qwen_bailian
# ─── Fields ───
# provider:    qwen_bailian
# apiKey:      •••••••••••• (lk read to reveal)
# baseUrl:     https://coding.dashscope.aliyuncs.com/v1
# cookie:      •••••••••••• (lk read to reveal)
# rawCurl:     curl 'https://...'

# JSON 输出
lk get "百炼" --format json

# 读取单个字段
lk get "百炼" apiKey        # 只输出 apiKey 值
lk get "百炼" cookie        # 只输出 cookie 值
lk get "GitHub" username    # 只输出 username
```

### 3.3 Secret Reference URI

```bash
# 语法: lk://[vault/]item/field
lk read "lk://百炼/apiKey"
# 输出: sk-xxx...（明文）

lk read "lk://百炼/cookie"
lk read "lk://GitHub/token"
lk read "lk://百炼"         # 读取整个 item（JSON）

# 带查询参数
lk read "lk://百炼/apiKey?mask=false"   # 不脱敏（默认脱敏）
lk read "lk://百炼?field=apiKey"        # 指定字段
lk read "lk://百炼?otp=true"            # 获取 OTP（如果有）
```

### 3.4 Agent 注入

```bash
# 环境变量注入
lk run -- claude "fix the bug in main.py"
# 等价于:
#   export LK_ITEM_百炼_apiKey="sk-xxx"
#   export LK_ITEM_百炼_baseUrl="https://..."
#   claude "fix the bug in main.py"

# 指定注入哪些 item
lk run --items "百炼,GitHub" -- codex "add tests"

# 指定注入哪些字段
lk run --fields apiKey,token -- claude "deploy"

# 自定义前缀
lk run --prefix CODING_ -- claude "fix"
# → CODING_apiKey=sk-xxx

# 配置文件注入
lk inject -i config.yml.tpl -o config.yml
# 模板中写 lk:// 引用:
#   openai_api_key: "lk://ChatGPT/accessToken"
```

### 3.5 编码用量查询

```bash
# 查询所有 CodingPlan 用量
lk quota

# 指定 provider
lk quota --provider qwen_bailian
lk quota --provider chatgpt
lk quota --provider claude

# 所有 provider
lk quota --all

# JSON 输出
lk quota --all --format json
```

### 3.6 同步命令

```bash
# 同步配置
lk sync config
# 交互式选择同步方式:
#   ○ Google Drive
#   ○ S3 兼容存储
#   ○ WebDAV (坚果云/Nextcloud)
#   ○ Git 仓库
#   ○ 不同步（仅本地）

# 手动同步
lk sync push       # 上传到云端
lk sync pull       # 从云端下载
lk sync status     # 查看同步状态

# 查看同步历史
lk sync history
```

### 3.7 Agent Token

```bash
# 创建 Agent token
lk token create --name "Claude Code" --scope "read:CodingPlan" --expires 24h
# 输出: lk_tok_xxxxx...

# 列出所有 token
lk token list

# 撤销 token
lk token revoke <token-id>

# 使用 token（无需 PIN）
export LK_TOKEN="lk_tok_xxx"
lk get "百炼" apiKey
```

### 3.8 管理命令

```bash
# 创建凭据
lk create --type CodingPlan --name "百炼" \
  --field provider=qwen_bailian \
  --field apiKey=sk-xxx \
  --field baseUrl=https://coding.dashscope.aliyuncs.com/v1

# 编辑凭据
lk edit "百炼" --field apiKey=sk-new-value

# 删除凭据
lk delete "百炼"

# 导入/导出
lk export --format json > vault-export.json
lk import vault-export.json

# 审计日志
lk audit
lk audit --since 24h
```

---

## 四、JSON 输出 Schema

### lk get --format json

```json
{
  "schemaVersion": "lockit.v1",
  "item": {
    "id": "uuid",
    "name": "百炼",
    "type": "CodingPlan",
    "service": "qwen_bailian",
    "fields": [
      { "name": "provider", "value": "qwen_bailian", "type": "text" },
      { "name": "apiKey", "value": "sk-xxx", "type": "secret" },
      { "name": "baseUrl", "value": "https://coding.dashscope.aliyuncs.com/v1", "type": "url" },
      { "name": "cookie", "value": "...", "type": "secret" },
      { "name": "rawCurl", "value": "curl ...", "type": "text" }
    ],
    "metadata": {
      "provider": "qwen_bailian",
      "createdAt": "2026-04-22T00:00:00Z",
      "updatedAt": "2026-04-22T00:00:00Z"
    }
  }
}
```

### lk list --format json

```json
{
  "schemaVersion": "lockit.v1",
  "items": [
    {
      "id": "uuid",
      "name": "百炼",
      "type": "CodingPlan",
      "service": "qwen_bailian",
      "updatedAt": "2026-04-22T00:00:00Z"
    },
    {
      "id": "uuid",
      "name": "GitHub",
      "type": "GitHub",
      "service": "github.com",
      "updatedAt": "2026-04-21T00:00:00Z"
    }
  ],
  "total": 12
}
```

### lk quota --all --format json

```json
{
  "schemaVersion": "lockit.v1",
  "quotas": [
    {
      "provider": "qwen_bailian",
      "instanceName": "sfm-coding-public-cn",
      "status": "VALID",
      "remainingDays": 18,
      "session": { "used": 120, "total": 200, "percent": 60 },
      "weekly": { "used": 800, "total": 2000, "percent": 40 },
      "monthly": { "used": 3000, "total": 10000, "percent": 30 }
    },
    {
      "provider": "chatgpt",
      "status": "VALID",
      "session": { "used": 80, "total": 160, "percent": 50 },
      "weekly": { "used": 500, "total": 3000, "percent": 17 }
    }
  ]
}
```

---

## 五、同步设计（基于现有架构）

### 现状问题

当前 Android 端同步：**全量上传整个 .db 文件到 Google Drive AppData**
- 无冲突检测
- 无增量同步
- 只支持 Google Drive
- 覆盖式（last-write-wins）

### CLI 同步架构

```
┌──────────────────────────────────────────────┐
│               Sync Flow                       │
├──────────────────────────────────────────────┤
│                                               │
│  1. 配置同步后端                               │
│  ┌──────────────────────────────────────┐    │
│  │ lk sync config                       │    │
│  │                                      │    │
│  │ ○ Google Drive (OAuth2)              │    │
│  │ ○ S3 (Access Key + Secret Key)       │    │
│  │   - AWS S3 / 阿里云 OSS / 腾讯 COS    │    │
│  │   - Cloudflare R2 / MinIO            │    │
│  │ ○ WebDAV (URL + username + password) │    │
│  │   - 坚果云 / Nextcloud / Seafile      │    │
│  │ ○ Git (remote URL + SSH key)         │    │
│  │   - GitHub / Gitee / GitLab          │    │
│  │ ○ 本地目录                            │    │
│  └──────────────────────────────────────┘    │
│                                               │
│  2. 同步机制                                   │
│  ┌──────────────────────────────────────┐    │
│  │ 文件: lockit_vault.db                │    │
│  │ 元数据: lockit_sync.json             │    │
│  │                                      │    │
│  │ lockit_sync.json:                    │    │
│  │ {                                    │    │
│  │   "version": 1,                      │    │
│  │   "lastModified": "...",             │    │
│  │   "checksum": "sha256:...",          │    │
│  │   "device": "cli-laptop",            │    │
│  │   "conflictResolution": "prompt"     │    │
│  │ }                                    │    │
│  └──────────────────────────────────────┘    │
│                                               │
│  3. 冲突检测                                   │
│  ┌──────────────────────────────────────┐    │
│  │ push:                                │    │
│  │   1. 下载云端 sync metadata           │    │
│  │   2. 比较 checksum                   │    │
│  │   3. 相同 → 直接上传                 │    │
│  │   4. 不同 → 提示冲突                 │    │
│  │                                      │    │
│  │ pull:                                │    │
│  │   1. 下载云端 vault.db               │    │
│  │   2. 比较本地 checksum               │    │
│  │   3. 云端更新 → 合并到本地            │    │
│  └──────────────────────────────────────┘    │
│                                               │
│  4. 自动同步（可选）                            │
│  ┌──────────────────────────────────────┐    │
│  │ lk sync config --auto-push true      │    │
│  │ lk sync config --auto-pull true      │    │
│  │ lk sync config --interval 30m        │    │
│  └──────────────────────────────────────┘    │
└──────────────────────────────────────────────┘
```

### 同步配置文件 (~/.config/lockit/config.toml)

```toml
[vault]
path = "~/.config/lockit/vault.db"
argon2_memory = 65536       # 64MB
argon2_iterations = 3
argon2_parallelism = 4

[sync]
enabled = true
backend = "google_drive"
auto_push = false
auto_pull = false
interval_minutes = 30
conflict_resolution = "prompt"  # prompt | local_wins | remote_wins

[sync.google_drive]
# OAuth2 token stored in keychain
# No config needed

[sync.s3]
# region = "cn-beijing"
# endpoint = "https://oss-cn-beijing.aliyuncs.com"
# bucket = "lockit-vault"
# key_prefix = "vaults/"
# access_key_id = "env:ALIYUN_ACCESS_KEY_ID"
# secret_access_key = "env:ALIYUN_ACCESS_KEY_SECRET"

[sync.webdav]
# url = "https://dav.jianguoyun.com/dav/lockit/"
# username = "env:WEBDAV_USER"
# password = "env:WEBDAV_PASS"

[sync.git]
# remote = "git@github.com:xqicxx/lockit-vault.git"
# branch = "main"
# ssh_key = "~/.ssh/id_ed25519"
```

---

## 六、技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| 语言 | Rust | 跨平台、内存安全、性能好、CLI 生态成熟 |
| CLI 框架 | clap | Rust 最成熟的 CLI 解析库 |
| SQLite | rusqlite | 原生 Rust SQLite 绑定 |
| 加密 | ring / aes-gcm | 审计过的加密库 |
| Argon2 | argon2 (Rust crate) | OWASP 推荐 |
| HTTP | reqwest | 异步 HTTP 客户端 |
| Google Drive | yup-oauth2 + google-drive3 | 官方 SDK |
| S3 | aws-sdk-s3 | 官方 SDK，兼容所有 S3 |
| WebDAV | dav1d / webdav-client | WebDAV 客户端 |
| 配置 | toml + config | 配置文件解析 |
| 密钥存储 | keyring (跨平台) | macOS Keychain / Linux Secret Service / Windows Credential Manager |

---

## 七、项目结构

```
lockit-cli/
├── Cargo.toml
├── README.md
├── docs/
│   ├── cli-agent-research.md    # 竞品调研
│   └── cli-design.md            # 本文件
├── src/
│   ├── main.rs                  # CLI 入口
│   ├── cli/
│   │   ├── mod.rs               # clap 命令定义
│   │   ├── signin.rs            # lk signin
│   │   ├── whoami.rs            # lk whoami
│   │   ├── get.rs               # lk get
│   │   ├── list.rs              # lk list
│   │   ├── read.rs              # lk read
│   │   ├── run.rs               # lk run
│   │   ├── quota.rs             # lk quota
│   │   ├── sync/
│   │   │   ├── mod.rs           # lk sync
│   │   │   ├── config.rs
│   │   │   ├── push.rs
│   │   │   ├── pull.rs
│   │   │   └── conflict.rs
│   │   ├── token/
│   │   │   ├── mod.rs           # lk token
│   │   │   ├── create.rs
│   │   │   ├── list.rs
│   │   │   └── revoke.rs
│   │   └── audit.rs             # lk audit
│   ├── vault/
│   │   ├── mod.rs               # Vault 核心
│   │   ├── crypto.rs            # AES-256-GCM
│   │   ├── key.rs               # Argon2id 密钥派生
│   │   ├── credential.rs        # Credential CRUD
│   │   ├── search.rs            # 搜索（拼音+模糊）
│   │   └── schema.rs            # SQLite schema
│   ├── sync/
│   │   ├── mod.rs               # Sync trait
│   │   ├── google_drive.rs      # Google Drive 后端
│   │   ├── s3.rs               # S3 后端
│   │   ├── webdav.rs           # WebDAV 后端
│   │   └── git.rs              # Git 后端
│   ├── auth/
│   │   ├── mod.rs               # Auth trait
│   │   ├── pin.rs               # PIN 认证
│   │   ├── token.rs             # Agent Token (JWT)
│   │   └── biometric.rs         # Touch ID (macOS)
│   ├── agent/
│   │   ├── mod.rs               # Agent 协议
│   │   ├── uri.rs               # lk:// URI 解析
│   │   ├── inject.rs            # 环境变量注入
│   │   └── mcp.rs               # MCP Server
│   └── config/
│       ├── mod.rs               # 配置管理
│       └── schema.rs            # config.toml schema
└── tests/
    ├── vault_test.rs
    ├── crypto_test.rs
    └── sync_test.rs
```

---

## 八、MVP 实现顺序

### Phase 1：核心（1-2 周）
1. 项目搭建 + Cargo.toml
2. SQLite vault.db 解析（兼容 Android 格式）
3. Argon2id 密钥派生 + AES-256-GCM 解密
4. `lk signin` / `lk whoami` / `lk lock`
5. `lk list` / `lk get`（text + JSON）

### Phase 2：Agent 能力（1 周）
6. `lk read "lk://..."` Secret Reference URI
7. `lk run` 环境变量注入
8. Agent Token（JWT，限定 scope）
9. `lk quota` 编码用量查询

### Phase 3：同步（1-2 周）
10. Google Drive 同步
11. S3 同步
12. `lk sync push/pull/status`
13. 冲突检测

### Phase 4：增强（后续）
14. WebDAV 同步
15. 拼音搜索 + 模糊匹配
16. MCP Server 模式
17. macOS Touch ID 集成
18. 审计日志

---

## 九、与 Android 端的兼容

### vault.db 格式兼容

```
Android (Kotlin/Room)    ↔    CLI (Rust/rusqlite)
─────────────────────────────────────────────────
CredentialEntity          ↔    CredentialRow (same schema)
AES-256-GCM encrypted     ↔    AES-256-GCM decrypt
Argon2id params           ↔    Same params (configurable)
SHA-256 key hash          ↔    SHA-256 key hash verification
```

### 共享配置

```
Android:                    CLI:
/data/data/com.lockit/      ~/.config/lockit/
├── databases/              ├── vault.db           # 同一格式
│   └── lockit.db           ├── config.toml        # CLI 配置
├── shared_prefs/           ├── tokens/            # Agent tokens
│   └── lockit_vault.xml    ├── audit.log          # 审计日志
└── files/                  └── sync/              # 同步状态
    └── audit.log
```

### 不同步的内容

- PIN/主密钥（每设备独立）
- Biometric 绑定（AndroidKeyStore / macOS Keychain 各自独立）
- 审计日志（本地记录，不同步）

---

## 十、lk run 环境变量注入示例

```bash
# 场景：让 Claude Code 使用百炼 API

# 方式 1：自动注入所有 CodingPlan 凭据
lk run -- claude "用百炼 API 写一个 Python 脚本"

# 注入的环境变量:
#   LK_CODINGPLAN_BAILIAN_PROVIDER=qwen_bailian
#   LK_CODINGPLAN_BAILIAN_APIKEY=sk-xxx
#   LK_CODINGPLAN_BAILIAN_BASEURL=https://coding.dashscope.aliyuncs.com/v1

# 方式 2：指定注入哪些
lk run --items "百炼" --fields apiKey,baseUrl \
  --prefix OPENAI_ \
  -- claude "写脚本"

# 注入:
#   OPENAI_APIKEY=sk-xxx
#   OPENAI_BASEURL=https://coding.dashscope.aliyuncs.com/v1

# 方式 3：在 Claude Code 的 hooks 中使用
# .claude/settings.json:
#   "hooks": {
#     "PreToolUse": {
#       "matcher": "Bash",
#       "hooks": [{ "type": "command", "command": "lk run -- $CLAUDE_COMMAND" }]
#     }
#   }
```
