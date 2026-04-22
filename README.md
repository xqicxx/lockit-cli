# Lockit CLI

> Agent 级别的认证代理基础设施 — 命令行工具

Lockit CLI 让 Agent（Claude Code、Codex 等）和开发者能通过命令行安全地读取、注入和管理认证凭据。

## 设计文档

| 文档 | 说明 |
|------|------|
| [docs/cli-agent-research.md](docs/cli-agent-research.md) | 竞品调研（1Password / Bitwarden / KeePass） |
| [docs/cli-design.md](docs/cli-design.md) | CLI 实现设计（架构/命令/同步/MVP 计划） |

## 核心特性

- **Secret Reference URI** — `lk://vault/item/field` 协议
- **Agent Token** — 限定 scope、可撤销、有过期时间
- **环境变量注入** — `lk run -- claude "..."`
- **云盘同步** — Google Drive / S3 / WebDAV / Git
- **编码用量查询** — `lk quota` 查看百炼/ChatGPT/Claude 用量
- **JSON 输出** — 所有命令支持 `--format json`
- **兼容 Android** — 共享同一 vault.db 格式

## 命令预览

```bash
lk signin                    # 解锁 vault
lk whoami                    # 查看状态
lk get "百炼"                # 读取凭据
lk get "百炼" --format json  # JSON 输出
lk read "lk://百炼/apiKey"   # Secret Reference
lk run -- claude "fix bug"   # Agent 注入
lk quota                     # 编码用量
lk sync push                 # 同步到云端
lk token create              # 创建 Agent token
```

## 技术栈

- Rust + clap（跨平台 CLI）
- rusqlite（SQLite vault.db 解析）
- ring / aes-gcm（AES-256-GCM 解密）
- argon2（Argon2id 密钥派生）
