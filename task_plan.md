---
name: lockit task plan
description: lockit 跨端凭据管理项目计划，覆盖 CLI、桌面端、Android 与云同步
type: project
---

# Lockit Task Plan

## Goal
把 `lockit` 做成一个可跨端使用的敏感信息保险库，安全存储并按需导出 API Key、账号密码、邮箱、Cookie、Token、SSH Key 等数据，最终支持 agent 通过 CLI 一键注入运行环境或项目配置。

## Current State
- `lockit` Rust CLI 已有 `init/add/get/list/delete/import/export` 命令。
- CLI 已实现 AES-256-GCM + Argon2id 加密，存储为 `vault.enc`（JSON VaultEnvelope），明文 Markdown 阶段已结束。
- `lockit-android` 已经具备较完整的数据模型、加密能力与 `Google Drive/WebDAV` 同步抽象，可作为能力参考。
- 当前待补：类型感知的凭据表单、Coding Plan、云同步命令、Agent 注入、与 Android 的导入导出桥接。

## Target Scope
1. 本地端到端加密保险库。
2. CLI 一键导入凭据到环境变量、`.env`、stdin、临时文件或 shell session。
3. 云同步后端：优先 `Google Drive`，`WebDAV` 后续。
4. 客户端形态：CLI 优先，桌面端跟进，Android 与协议对齐。
5. 统一的凭据模型、同步格式、验收标准和测试基线。

## Proposed Workstreams

| Workstream | Status | Description |
|-----------|--------|-------------|
| 1. 规划与规范 | completed | 明确产品边界、目录、路线图、验收标准 |
| 2. 统一数据模型 | in_progress | 统一 credential schema、secret bundle、manifest |
| 3. 安全核心 | completed | 主密码、Argon2id、AES-256-GCM、密钥轮换 |
| 4. 本地存储层 | completed | 加密 vault.enc（JSON VaultEnvelope），明文 Markdown 已废弃 |
| 5. CLI 体验 | in_progress | `init/add/get/list/import/export/use/env/run` |
| 6. Agent 导入能力 | pending | 环境注入、模板导出、按服务自动映射 |
| 7. 云同步 | pending | Google Drive / WebDAV / 冲突解决 / manifest |
| 8. 桌面端规划 | pending | 共享协议、配置页、同步状态、导入助手 |
| 9. 测试与发布 | pending | 单测、集成测试、兼容性测试、发布检查表 |

## Milestones

| Milestone | Status | Exit Criteria |
|-----------|--------|---------------|
| M1: Spec Freeze | in_progress | 文档确认、schema 草案、命令清单冻结 |
| M2: Secure Vault MVP | completed | CLI 可初始化加密 vault，并可增删查改 |
| M3: Agent Import MVP | pending | CLI 可按服务一键导出 env / shell 注入 |
| M4: Sync MVP | pending | Google Drive 可双向同步并完成冲突检测 |
| M5: Multi-client Alignment | pending | Android / CLI / Desktop 共享同一协议 |

## Deliverables Created In This Session
- `lockit/docs/README.md`
- `lockit/docs/overview/product-plan.md`
- `lockit/docs/architecture/system-architecture.md`
- `lockit/docs/roadmap/roadmap-todo.md`
- `lockit/docs/standards/code-acceptance.md`

## Risks
- 现有 Android 数据模型远比 CLI 丰富，若没有统一 schema，后续会产生双轨演进。
- 加密存储已就位（vault.enc），后续需确保 import/export 与 Android 备份格式兼容。
- “一键导入”能力涉及 shell、子进程、临时文件与日志泄露边界，安全设计必须前置。
- 云同步一旦上线，就必须定义版本、校验和、冲突解决策略，不能靠客户端隐式约定。

## Decision Log
- 先做规范和目录沉淀，再推进实现，避免 CLI/Android/桌面端各自长出不同协议。
- 文档目录放在 `lockit/docs/`，按 `overview / architecture / roadmap / standards` 分类。
- 开发优先级采用 `CLI 安全 MVP -> Agent 导入 -> Google Drive 同步 -> 其他客户端对齐`。

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| 仓库根目录不是 Git 仓库 | 1 | 改为把 `lockit` 视为独立子项目进行规划 |
