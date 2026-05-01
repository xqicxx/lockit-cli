# Findings

## Current Project Findings

### CLI (`lockit`)
- 当前 CLI 已实现基础命令：`init`、`add`、`get`、`list`、`delete`、`import`、`export`。
- 加密已就位：AES-256-GCM + Argon2id，vault.enc JSON VaultEnvelope，VaultSession CRUD + 审计日志。
- 当前 `CredentialType` 已有 18 种类型完整枚举，与 Android 端对齐。
- 当前 CLI 待补：类型感知的交互式 add/edit、Coding Plan、云同步命令、Agent 注入（env/run）、导入导出与 Android 桥接。

### Android (`lockit-android`)
- Android 端已经具备更成熟的凭据模型、更多 credential types，以及同步后端抽象。
- 已存在 `SyncBackend`、`GoogleDriveBackend`、`WebDavBackend`、`SyncManifest` 等同步相关设计。
- Android 端 README 中已经明确了 AES-256-GCM 与 Argon2id 参数，可作为协议对齐参考。

### Cross-platform Implications
- CLI 与 Android 现在还没有共享协议契约，继续并行开发会放大后续迁移成本。
- 最应该先冻结的是：
  - 凭据统一 schema
  - 加密 vault 文件格式
  - 云端 manifest 格式
  - agent 导入接口约定

## Recommended Direction
- 以 `lockit` CLI 作为协议和能力的”参考实现”。
- Android 与未来桌面端围绕同一个 vault format / sync manifest / import contract 对齐。
- 同步后端：**Google Drive 优先**，WebDAV 作为第二优先级。
- 把”查看 secret”和”把 secret 注入 agent/命令”分成两类能力，避免 CLI 直接把敏感值打印到日志。

## New Documentation Structure
- `lockit/docs/overview/`：产品目标、边界、用户价值
- `lockit/docs/architecture/`：模块拆分、同步与注入方案
- `lockit/docs/roadmap/`：阶段计划、TODO、优先级
- `lockit/docs/standards/`：代码质量标准、验收标准、测试门槛

## Architecture Notes
- 推荐后续 Rust 代码按以下逻辑层拆分，而不是继续集中在 `storage.rs`：
  - `domain`：凭据模型、策略、校验
  - `application`：用例层，如 add/get/use/sync
  - `infrastructure`：vault、crypto、filesystem、cloud backends
  - `interfaces`：CLI、桌面端桥接、导入器

## Open Questions
- 桌面端技术栈是否也会沿用 Rust 作为核心，UI 单独壳化？
- CLI 的“一键导入”第一版是优先支持 `.env`、shell export，还是直接 `lockit run -- <command>`？
- Android 端既有数据格式，是否要做一次兼容迁移，还是统一升级到新格式？
