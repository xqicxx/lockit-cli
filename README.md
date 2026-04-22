# Lockit CLI

> Lockit 命令行工具 — Agent 级别的认证代理基础设施

## 目标

Lockit CLI 让 Agent（Claude Code、Codex 等）和开发者能通过命令行安全地读取、注入和管理认证凭据。

## 设计参考

详细调研报告见 [docs/cli-agent-research.md](docs/cli-agent-research.md)

### 竞品对比

| 特性 | 1Password (`op`) | Bitwarden (`bw`) | BW Secrets (`bws`) | KeePass |
|------|:-:|:-:|:-:|:-:|
| JSON 输出 | ✅ | ✅ | ✅ | ❌ |
| URI 引用 | ✅ `op://` | ❌ | ❌ | ❌ |
| 环境变量注入 | ✅ `op run` | ❌ | ✅ `bws run` | ❌ |
| 配置文件注入 | ✅ `op inject` | ❌ | ❌ | ❌ |
| 机器账户 | ✅ | ✅ | ✅ | ❌ |
| Agent 友好度 | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐ |

### Lockit CLI 设计方向

- `lk://vault/item/field` Secret Reference URI
- `lk run` 环境变量注入
- Agent Token（限定 scope、可撤销、有过期时间）
- 全命令 JSON 输出
