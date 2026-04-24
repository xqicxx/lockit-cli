# Lockit 同步模块审计报告

> 对比 `lockit-cli/docs/sync-design.md`（设计方案）与 `lockit-android/app/src/main/java/com/lockit/data/sync/`（实际实现）

---

## 一、文件清单

| 文件 | 行数 | 状态 |
|------|------|------|
| `sync-design.md` (CLI) | ~680 | 设计文档 |
| `SyncBackend.kt` | 55 | ✅ 接口定义 |
| `SyncCrypto.kt` | 112 | ✅ 加密实现 |
| `SyncManifest.kt` | 73 | ✅ Manifest + 状态枚举 |
| `SyncManager.kt` | 260 | ✅ 核心同步逻辑 |
| `GoogleDriveBackend.kt` | 230 | ✅ Google Drive 后端 |
| `WebDavBackend.kt` | 380 | ✅ WebDAV 后端（额外） |
| `GoogleDriveSyncManager.kt` | 100 | ⚠️ 旧代码，未删除 |

---

## 二、设计方案 vs 实际实现对照

### 2.1 核心架构

| 设计项 | sync-design.md | Android 实现 | 符合度 |
|--------|---------------|-------------|--------|
| Sync Key 独立于 PIN | ✅ 明确设计 | ✅ `SyncCrypto` + SharedPreferences | ✅ |
| AES-256-GCM 加密 | ✅ | ✅ 完全一致 | ✅ |
| 加密格式: Version+Nonce+Ciphertext | ✅ | ✅ 完全一致 | ✅ |
| manifest.json 元数据 | ✅ | ✅ `SyncManifest` | ✅ |
| SHA-256 checksum 冲突检测 | ✅ | ✅ `computeLocalChecksum()` | ✅ |
| Device ID 标识 | ✅ | ✅ `Build.MODEL + UUID` | ✅ |
| Push/Pull/Both 三模式 | ✅ Push/Pull | ✅ push/pull/forcePush/forcePull | ✅ |
| 冲突解决策略 | ✅ 提示用户选择 | ✅ `SyncConflictException` | ⚠️ 缺 UI |

### 2.2 加密模块 `SyncCrypto`

| 设计 | 实现 | 状态 |
|------|------|------|
| KEY_SIZE = 32 | ✅ 32 bytes | ✅ |
| NONCE_SIZE = 12 | ✅ 12 bytes | ✅ |
| TAG_SIZE = 16 | ✅ 16 bytes | ✅ |
| Base64 编码/解码 | ✅ `encodeSyncKey`/`decodeSyncKey` | ✅ |
| 格式校验 | ✅ `isValidEncryptedBlob` | ✅ |

### 2.3 同步流程

| 流程 | 设计 | 实现 | 状态 |
|------|------|------|------|
| Push | 加密→对比→上传 | ✅ 完全实现 | ✅ |
| Pull | 下载→解密→替换 | ✅ 完全实现 | ✅ |
| 冲突检测 | checksum 对比 | ✅ 四种状态判断 | ✅ |
| Force Push | 忽略冲突 | ✅ `forcePush()` | ✅ |
| Force Pull | 忽略冲突 | ⚠️ 直接复用 pull | ⚠️ |
| 数据库关闭保护 | 未提及 | ✅ `LockitDatabase.closeAndReset()` | ✅ 超设计 |
| Pull 前备份 | 未提及 | ✅ `vault.db.backup` | ✅ 超设计 |

### 2.4 后端实现

| 后端 | 设计提及 | 实现 | 状态 |
|------|---------|------|------|
| Google Drive | ✅ 主要后端 | ✅ `GoogleDriveBackend` | ✅ |
| WebDAV | ✅ Phase 3 | ✅ `WebDavBackend` | ✅ 提前完成 |
| S3 | ✅ Phase 3 | ❌ 未实现 | ❌ |
| Git | ✅ Phase 3 | ❌ 未实现 | ❌ |

### 2.5 WebDAV 额外能力（设计中未提及）

| 功能 | 实现 | 说明 |
|------|------|------|
| HTTPS 强制 | ✅ | `SecurityException` 阻止 HTTP |
| ETag 乐观锁 | ✅ | `getVaultETag()` + `uploadVaultWithLocking()` |
| If-Match 冲突检测 | ✅ | HTTP 412 返回冲突 |
| OOM 保护 | ✅ | 50MB 上限检查 |
| 超时配置 | ✅ | 连接 30s / 读写 60s |
| EncryptedSharedPreferences | ✅ | 凭据加密存储 |

---

## 三、问题清单

### 🔴 严重问题

| # | 问题 | 影响 |
|---|------|------|
| 1 | **`GoogleDriveSyncManager.kt` 未删除** | 新旧两套 Google Drive 代码并存，容易混淆。旧版是破坏性覆盖上传（先删再建），无冲突检测 |
| 2 | **无同步锁机制** | 设计中提到 `lock` 文件防并发写入，实际未实现。多设备同时 push 可能互相覆盖 |
| 3 | **缺少双向同步（syncBoth）** | `SyncManager` 只有 push/pull/forcePush，无自动双向同步逻辑 |

### 🟡 中等问题

| # | 问题 | 影响 |
|---|------|------|
| 4 | **Sync Key 存储在 SharedPreferences（非加密）** | 设计中明确用 `EncryptedSharedPreferences`，但 `SyncManager` 用的是普通 SharedPreferences |
| 5 | **无 Sync Key 设置 UI** | 设计中包含二维码生成、扫码输入等 UI，实际缺失 |
| 6 | **无自动同步触发** | issue-cloud-sync-enhancement 已提出，当前仅手动触发 |
| 7 | **forcePull 直接复用 pull** | 设计意图是忽略冲突，但 pull 本身已检测冲突，forcePull 语义不完整 |
| 8 | **无 WorkManager 后台同步** | 设计中提到用 WorkManager 处理定时同步和重试 |

### 🟢 轻微问题

| # | 问题 | 影响 |
|---|------|------|
| 9 | **checksum 计算流式读取但未关闭流** | `computeLocalChecksum()` 用了 `use`，但如果中途异常可能泄漏 |
| 10 | **manifest 版本硬编码** | `SyncManifest.version = 1`，无版本升级机制 |
| 11 | **设备 ID 每次重启可能变化** | UUID 存储在 SharedPreferences，但 `getDeviceId()` 未做持久化检查 |
| 12 | **无 S3/MinIO 后端** | 设计中 Phase 3 的 S3 后端未实现 |

---

## 四、代码质量评估

| 维度 | 评分 | 说明 |
|------|------|------|
| 架构设计 | ⭐⭐⭐⭐ | 接口分离清晰，`SyncBackend` trait 设计好 |
| 加密安全 | ⭐⭐⭐⭐⭐ | AES-256-GCM，格式规范，Sync Key 独立 |
| 冲突处理 | ⭐⭐⭐ | 检测完善，但缺少锁和 UI 解决流程 |
| 错误处理 | ⭐⭐⭐⭐ | Result 链式处理，异常分类清晰 |
| 代码复用 | ⭐⭐⭐ | 旧 `GoogleDriveSyncManager` 未清理 |
| 安全性 | ⭐⭐⭐ | Sync Key 未加密存储，WebDAV 强制 HTTPS |

---

## 五、实施进度

| 阶段 | 设计内容 | 完成度 |
|------|---------|--------|
| **Phase 1: Android 改造** | | **80%** |
| ├── SyncCrypto | ✅ 完整实现 | 100% |
| ├── SyncManager 接口 | ✅ 完整实现 | 100% |
| ├── GoogleDrive 后端 | ✅ `GoogleDriveBackend` | 100% |
| ├── WebDAV 后端 | ✅ `WebDavBackend`（含 ETag 锁） | 120% |
| ├── Sync Key UI | ❌ 缺失 | 0% |
| ├── 同步状态 UI | ❌ 缺失 | 0% |
| ├── 清理旧代码 | ❌ `GoogleDriveSyncManager` 残留 | 0% |
| **Phase 2: CLI 同步** | | **0%** |
| ├── Rust SyncCrypto | ❌ | 0% |
| ├── SyncBackend trait | ❌ | 0% |
| ├── lk sync 命令 | ❌ | 0% |
| **Phase 3: 多后端** | | **33%** |
| ├── S3 | ❌ | 0% |
| ├── WebDAV | ✅ 已实现 | 100% |
| ├── Git | ❌ | 0% |

---

## 六、建议优先级

1. **🔴 删除 `GoogleDriveSyncManager.kt`** — 旧代码与新架构冲突，删除后代码库更清晰
2. **🔴 Sync Key 改用 EncryptedSharedPreferences** — 安全要求，30 分钟可修复
3. **🔴 实现同步锁机制** — 防止多设备并发写入损坏数据
4. **🟡 补全 Sync Key 设置 UI** — 用户无法首次配置
5. **🟡 实现 forcePull 独立逻辑** — 当前语义不完整
6. **🟡 添加 WorkManager 自动同步** — 用户体验提升
7. **🟢 补充 S3 后端** — Phase 3 规划

---

## 七、总结

**实现质量总体评价：⭐⭐⭐⭐（优秀）**

设计方案在 Android 端的落地非常好。核心加密、同步逻辑、冲突检测、多后端接口全部按设计实现，且 WebDAV 后端超出了设计预期（额外实现了 ETag 乐观锁）。

主要差距在于：
1. **旧代码未清理**（`GoogleDriveSyncManager`）
2. **UI 层缺失**（Sync Key 配置、冲突解决、状态展示）
3. **CLI 端未启动**（`lockit-cli` 目前只有设计文档，无代码）

下一步建议：先清理旧代码 + 修复 Sync Key 安全存储，然后推进 CLI 端实现。
