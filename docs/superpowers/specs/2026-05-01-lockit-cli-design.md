# Lockit CLI 设计文档

## 概述

基于 lockit-android 已有能力，将 lockit-cli 打造成功能对等的命令行凭据管理器。增强现有 Rust workspace 中的 `lockit-core` 和 `lockit-cli`。

## Android 兼容边界

Android 和 CLI 的存储介质不同，兼容性定义在**字段加密层**和**导入导出层**：

| 层面 | Android | CLI |
|------|---------|-----|
| 存储介质 | Room/SQLite，每个 credential value 独立 nonce+ciphertext（行级加密） | 单文件 vault.enc，整个 VaultPayload 序列化后 JSON VaultEnvelope 整体加密（文件级加密） |
| 加密算法 | AES-256-GCM，`[12-byte nonce][ciphertext + 16-byte GCM tag]` | **算法相同**，但加密粒度不同（文件级 vs 行级） |
| KDF 参数 | Argon2id (64MB, 3 iter, 4 parallel) | **相同** |
| 凭据类型 | 18 种，CredentialField 定义 | 从 Android 移植 |
| 互操作 | — | `import` 可解析 Android 加密备份文件，解密后重加密写入 vault.enc；`export` 可输出 Android 可导入的格式 |
| 同步 manifest | Android `SyncManifest.toJson()` 使用 **camelCase**（vaultChecksum, updatedAt, updatedBy） | CLI manifest 使用独立 schema，字段映射层处理 camelCase/snake_case 转换 |

**不是**文件级兼容（vault.enc ≠ Room DB），**是**导入导出桥接 + 同步协议对齐。两端各自管理自己的本地存储，通过 import/export 和 sync manifest 互通。

## 命令结构

```
lockit
├── init                          # 初始化 vault
├── add                           # 添加凭据（交互式、类型感知）
├── list [query]                  # 列表 / 搜索
├── show <name>                   # 查看详情
├── edit <name>                   # 编辑凭据（交互式）
├── delete <name>                 # 删除
├── reveal <name> [field]         # 解密查看单个字段值
│
├── coding-plan                   # Coding Plan 板块
│   ├── list                      #   列出所有 provider 配额
│   └── refresh [provider]        #   刷新配额
│
├── sync                          # 云同步
│   ├── status                    #   查看同步状态
│   ├── push                      #   推送到远程
│   ├── pull                      #   从远程拉取
│   └── config                    #   配置同步后端
│
├── env <name>                    # 注入凭据到环境变量（输出 export 语句）
├── run <name> -- <cmd>           # 在注入环境中运行命令
│
├── export [name]                 # 导出 vault / 单个凭据
└── import <file>                 # 导入（支持 Android 备份格式）
```

## 模块架构

```
lockit/
├── crates/
│   ├── lockit-core/           # 核心库（扩展）
│   │   ├── credential.rs      #   已有：18 种类型 + Credential + Draft
│   │   ├── credential_field.rs #   ★新增：字段定义、预设值、校验规则
│   │   ├── crypto.rs          #   已有：AES-256-GCM + Argon2id
│   │   ├── vault.rs           #   已有：VaultSession CRUD + 审计
│   │   ├── coding_plan.rs     #   ★新增：Provider 模型、配额数据结构
│   │   ├── coding_plan/       #   ★新增：各 provider API 客户端
│   │   ├── sync.rs            #   增强：同步 trait + Google Drive 实现
│   │   ├── sync/              #   ★新增：SyncManifest、冲突检测
│   │   ├── migration.rs       #   已有：旧格式导入
│   │   └── lib.rs
│   │
│   └── lockit-cli/            # CLI 层（重写）
│       ├── main.rs            #   Clap 命令解析 + 路由
│       ├── commands/          #   每个子命令一个模块
│       ├── interactive.rs     #   ★交互式提示（inquire crate）
│       └── output.rs          #   ★统一输出格式（表格/JSON/纯文本）
```

## 凭据类型感知系统

从 Android `CredentialType.kt` 移植字段定义到 `lockit-core/src/credential_field.rs`：

```rust
struct CredentialFieldDef {
    label: String,         // "PROVIDER"
    placeholder: String,   // "Select provider"
    required: bool,        // 必填校验
    is_dropdown: bool,     // 下拉 or 自由输入
    presets: Vec<String>,  // 预设值列表
}
```

每个 `CredentialType`（18 种）关联其字段列表和必填索引。

### 交互式添加（默认）

```
$ lockit add

? Credential type:  ← 列表选择
  api_key
  github
  account
  ...

? NAME:  OPENAI_API_KEY
? SERVICE:  [下拉] openai / anthropic / google / ... 或自定义
? KEY_IDENTIFIER:  [默认: default]
? SECRET_VALUE:  ********    ← 隐藏输入

✓ Credential added: 0xA3F2B
```

### 非交互模式

**从 stdin 读取（推荐，不泄露到 shell history）：**
```bash
lockit add --stdin <<'EOF'
{"type":"api_key","name":"OPENAI_API_KEY","fields":{"service":"openai","secret_value":"sk-xxx"}}
EOF
```

**从文件读取：**
```bash
lockit add --file credential.json
```

**`--json` 仅用于非敏感测试（会进 shell history）：**
```bash
lockit add --json '{"type":"api_key","name":"OPENAI_API_KEY","fields":{"service":"openai"}}'
# 注意：敏感字段仍需后续 edit 或交互提示输入
```

## Coding Plan

```
$ lockit coding-plan list

PROVIDER      PLAN       QUOTA USED    REMAINING    REFRESHED
qwen_bailian  plus       847 / 1000    153          2H_AGO
chatgpt       team       324 / 500     176          1H_AGO
claude        pro        —             —            5H_AGO

$ lockit coding-plan refresh
  qwen_bailian  ✓  (847/1000)
  chatgpt       ✓  (324/500)
  claude        ✗  auth expired
```

Provider 客户端从 Android 对应类移植：
- `QwenCodingPlan` / `BailianAuthClient`
- `ChatGPTCodingPlan` / `ChatGptAuthClient`
- `ClaudeCodingPlan` / `ClaudeAuthClient`
- `DeepSeekCodingPlan`
- `MimoCodingPlan`

## 同步

**Google Drive 优先**，WebDAV 作为第二后端后续实现。

```
$ lockit sync config
? Backend:  googledrive
✓ Google Drive configured

$ lockit sync status
Remote:  2026-05-01 14:22 (3 entries)
Local:   2026-05-01 13:10 (3 entries)
Status:  up-to-date

$ lockit sync push
Pushing... ✓ (3 credentials)

$ lockit sync pull
Remote has 2 new, 1 conflict.
? Resolve conflict "GITHUB_TOKEN": keep_local / keep_remote / keep_both
```

同步模块结构：
- `SyncBackend` trait — 抽象同步后端
- `GoogleDriveBackend` — Google Drive API 实现（首发）
- `WebDavBackend` — WebDAV 实现（后续）
- `SyncManifest` — 远程清单（版本号、校验和、条目列表）
- `ConflictDetector` — 三方对比（local vs base vs remote）

## Agent 注入

```bash
# lockit env 输出 export 语句，通过 eval 注入当前 shell
eval "$(lockit env OPENAI_API_KEY)"

# lockit run 在子进程中设置环境变量后执行命令
# 注意：使用 sh -c + 单引号，避免父 shell 提前展开变量
lockit run OPENAI_API_KEY -- sh -c 'curl -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com'

# 注入多个凭据
lockit run OPENAI_API_KEY,GITHUB_TOKEN -- ./deploy.sh
```

`lockit run` 行为：
1. 解锁 vault（提示输入主密码或从 `LOCKIT_MASTER_PASSWORD` 环境变量读取）
2. 读取指定凭据的字段值，解密
3. 将字段映射为环境变量：`{NAME}_{FIELD}` → 大写、下划线（如 `OPENAI_API_KEY_SECRET_VALUE`）
4. `fork` + `exec` 子进程，在子进程环境中设置这些变量
5. 子进程退出后环境变量随进程销毁

`lockit env` 行为：
1. 同理解锁 + 解密
2. 输出 `export VAR="value"` 到 stdout，供 `eval` 消费
3. 不直接修改父 shell 环境（不可能），而是让 shell 自己 eval

## 错误处理

- `lockit-core`：`thiserror` 枚举 — `CryptoError`, `VaultError`, `SyncError`, `CodingPlanError`
- `lockit-cli`：`anyhow::Result` 包装，用户友好输出

## 加密格式

与 Android 端字段级加密一致：
- 格式：`[12-byte nonce][ciphertext + 16-byte GCM tag]`
- KDF：Argon2id (memory=64MB, iterations=3, parallelism=4)
- Vault 文件：JSON VaultEnvelope（base64 编码各字段）
- 文件路径：`~/.lockit/vault.enc`（通过 `directories` crate 获取平台路径）

## 测试基线

- `lockit-core`：单元测试 — crypto 加解密、credential 字段校验、vault CRUD、sync 冲突检测、coding plan 解析
- `lockit-cli`：集成测试 — 各命令 JSON/stdin 非交互路径
