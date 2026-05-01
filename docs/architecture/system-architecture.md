# Lockit System Architecture

## 1. 总体原则

- 先协议统一，再多端实现
- 安全默认开启，便利性通过受控接口提供
- 领域模型稳定，客户端只是壳
- 模块高内聚、边界清晰、依赖单向流动

## 2. 建议架构分层

```text
lockit/
├── src/
│   ├── main.rs
│   ├── cli/                # clap 命令定义、输出格式、参数解析
│   ├── application/        # 用例层：add/get/use/import/export/sync
│   ├── domain/             # Credential、Profile、Policy、Validation
│   ├── security/           # Argon2id、AES-GCM、key rotation、redaction
│   ├── vault/              # 本地 vault 仓储、schema migration、manifest
│   ├── sync/               # WebDAV / Google Drive / conflict resolution
│   ├── integrations/       # env exporter、cookie exporter、ssh temp file
│   └── support/            # error、time、fs、testing helpers
└── docs/
```

## 3. 核心模块职责

### `domain`
- 定义统一的 `Credential`、`CredentialType`、`SecretField`
- 定义 profile、service mapping、导出策略
- 只放业务规则，不碰 IO、网络和 CLI

### `application`
- 编排用例，例如：
  - `AddCredential`
  - `GetCredential`
  - `UseProfileForCommand`
  - `ExportEnvFile`
  - `SyncVault`
- 负责事务边界、权限检查、日志脱敏

### `security`
- 主密码校验
- Argon2id KDF
- AES-256-GCM 加解密
- secret redaction 与安全擦除策略

### `vault`
- vault 文件格式
- schema 版本管理
- 本地读写与迁移
- 可选 SQLite 或单文件加密容器，但对上层暴露统一仓储接口

### `sync`
- `SyncBackend` 抽象
- manifest 序列化
- checksum、版本号、冲突检测
- `WebDAV`、`Google Drive` 实现

### `integrations`
- 导出为环境变量
- 写入临时 `.env`
- 生成 cookie jar / netrc / ssh 临时文件
- 启动受控子进程并注入运行环境

## 4. 建议统一的数据模型

```json
{
  "id": "uuid",
  "type": "api_key",
  "service": "openai",
  "name": "OPENAI_API_KEY",
  "fields": {
    "value": "sk-***",
    "key_identifier": "default"
  },
  "tags": ["agent", "project-a"],
  "metadata": {
    "source": "manual"
  },
  "created_at": "2026-04-27T00:00:00Z",
  "updated_at": "2026-04-27T00:00:00Z"
}
```

说明：
- 不再把所有类型都强压成 `key/value` 两字段
- 用 `fields` 容纳不同类型的结构化内容
- `name` 用于用户认知和 env 映射
- `service` 用于导入模板和筛选

## 5. Agent 导入设计

建议分成三层能力：

### Level 1: 查看
- `lockit get <name>`
- 默认输出脱敏摘要，只有显式参数才返回敏感值

### Level 2: 导出
- `lockit export env --service openai`
- `lockit export dotenv --profile project-a`
- `lockit export cookie --name github-session`

### Level 3: 注入执行
- `lockit run --profile project-a -- claude code`
- `lockit run --service openai -- env | grep OPENAI`

其中第三层最关键，因为它最接近“agent 一键导入”，也最能减少 secret 暴露面。

## 6. 同步协议建议

### 云端对象
- `vault.enc`
- `manifest.json`

### `manifest.json` 至少包含
- `version`
- `vault_checksum`
- `updated_at`
- `updated_by`
- `encrypted_size`
- `schema_version`

### 冲突策略
- 第一版不要只做 `last-write-wins`
- 建议最少提供：
  - 无冲突自动同步
  - 检测到双端都修改时中止并提示
  - 用户可手动选择 `keep-local` / `keep-remote`

## 7. 与 Android / Desktop 的关系

- Android 端继续作为移动录入和查看端
- CLI 端作为协议参考实现与 agent 接入层
- 桌面端优先复用 Rust 核心能力，UI 只是调度壳
- 所有客户端共享同一 vault format、schema version 和 sync manifest
