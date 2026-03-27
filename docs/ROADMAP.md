# lockit 执行路线图

> 工作流：**小步实现 → 外部模型验证 → 继续下一步**。
> 每个「可执行单元」独立可实现、有明确验收标准、不依赖未完成模块。

---

## 阶段 0 — 同步引擎基础（可与 Daemon 并行）

### 单元 0-A：SyncBackend trait 定义（Issue #16 部分）✅
- `SyncBackend` async trait（upload/download/list/delete/metadata）
- `BackendConfig` 枚举
- `MockBackend`（内存 HashMap）

**验收：** `cargo test --package lockit-sync` 全绿

---

### 单元 0-B：SyncBackend Factory + 元数据结构（Issue #16 剩余）✅
- `SyncBackendFactory::from_config()`
- `SyncMetadata`（版本号、时间戳、校验和）

**验收：** factory 可从 config 构造 mock backend，元数据可序列化

---

### 单元 0-C：S3 兼容后端（Issue #17）✅
- `S3Backend`（aws-sdk-s3 v1，支持 AWS/Aliyun/Tencent/MinIO）
- 集成测试（`#[ignore]`，需本地 MinIO）

**验收：** 单元测试全绿；集成测试需 MinIO

---

## 阶段 1 — Daemon 核心（p0）

### 单元 1-A：IPC 协议定义（Issue #8 部分）
- 所有请求/响应类型（UnlockVault、GetCredential、SetCredential 等）
- MessagePack 序列化/反序列化
- 协议版本号

**验收：** 所有消息类型序列化往返测试通过，无实际 socket 依赖

---

### 单元 1-B：IPC 通信层实现（Issue #8 剩余）
- Unix Domain Socket（macOS/Linux）/ Named Pipe（Windows）
- 连接超时与重连机制
- 错误响应规范化

**验收：** client 连接本地 socket，发送请求，收到响应 <10ms

---

### 单元 1-C：Daemon 生命周期管理（Issue #7 子任务）
- 启动 / 停止 / 状态查询
- PID 文件管理
- 超时自动锁定（15 分钟无活动清除 VEK）

**验收：** `lk daemon start/stop/status` 可用，超时锁定触发测试通过

---

## 阶段 2 — CLI 命令层（依赖阶段 1）

### 单元 2-A：核心读写命令（Issue #10 部分）
- `lk init`、`lk add`、`lk get`、`lk list`

**验收：** `lk get <profile> <key>` 在 Daemon 解锁状态下 <100ms 返回

---

### 单元 2-B：运行时注入与管理命令（Issue #10 剩余）
- `lk run`（环境变量注入）
- `lk delete`、`lk edit`、`lk lock`、`lk unlock`、`lk export`、`lk import`

**验收：** `lk run --profile foo -- env` 输出包含 profile 所有 key=value

---

### 单元 2-C：Shell 补全（Issue #10 子任务）
- bash / zsh / fish 补全脚本生成

**验收：** `lk --generate-completion zsh` 输出可直接 source

---

## 阶段 3 — SDK + 生物识别（依赖阶段 1）

### 单元 3-A：生物识别接入（Issue #9）
- macOS Touch ID via `security-framework`，失败 fallback 到主密码

**验收：** Touch ID 认证通过后可获取 MK，不可用时自动降级

---

### 单元 3-B：credentials 文件格式（Issue #18 子任务）
- `~/.lockit/credentials` INI 风格格式规范 + 读写实现

**验收：** 第三方工具不改源码即可读取

---

## 阶段 4 — 基础设施（随时可并行）

### 单元 4-A：文档（Issue #21）
- README、CONTRIBUTING、SECURITY、docs/

**验收：** 新用户按 README 5 分钟内完成安装和首次使用

---

### 单元 4-B：发布流程（Issue #22）
- cargo-dist + GitHub Actions release workflow，跨平台二进制

**验收：** 推送 tag 后三平台二进制出现在 GitHub Releases

---

## 执行顺序

```
阶段 0（Sync，已完成）
    0-A ✅ → 0-B ✅ → 0-C ✅

阶段 1（Daemon，可与阶段 0 并行）
    1-A → 1-B → 1-C

阶段 2（CLI，依赖阶段 1 完成）
    2-A → 2-B → 2-C

阶段 3（SDK+Bio，依赖阶段 1，可与阶段 2 并行）
    3-A / 3-B（互相独立）

阶段 4（随时可做）
    4-A / 4-B（互相独立）
```
