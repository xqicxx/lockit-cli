# Lockit 双向同步方案设计

> Android ↔ Cloud ↔ CLI 双向同步

---

## 一、现状分析

### 当前 Android 同步实现

`GoogleDriveSyncManager.kt` 只做了两件事：

```kotlin
// 1. 上传整个 vault.db 到 Google Drive AppData
suspend fun uploadVault(account, dbFilePath): Result<Unit>
//    → 先删除旧文件，再创建新文件（破坏性覆盖）

// 2. 查询最后备份时间
suspend fun getLastBackupTime(account): Result<String?>

// ❌ 没有 download / restore
// ❌ 没有冲突检测
// ❌ 没有拉取逻辑
```

### 核心问题

| # | 问题 | 严重性 |
|---|------|--------|
| 1 | **只有上传，没有下载/恢复** | 🔴 致命 |
| 2 | **破坏性覆盖**：先删再创建 = 无回滚 | 🔴 致命 |
| 3 | **无冲突检测**：多设备同时修改 = 丢数据 | 🔴 致命 |
| 4 | **全文件上传**：无法增量同步 | 🟡 严重 |
| 5 | **只支持 Google Drive** | 🟡 严重 |

### 根本矛盾

```
Android: PIN=123456 → masterKey=abc → vault.db 用 abc 加密
CLI:     PIN=654321 → masterKey=xyz → vault.db 用 xyz 加密

同一个 .db 文件，不同设备用不同密钥加密
→ 无法互相解密
→ 不能直接同步 .db 文件
```

---

## 二、方案对比

### 方案 A：统一密钥同步整个 .db

```
所有设备用相同 masterKey → 直接同步 .db 文件
```

| 优点 | 缺点 |
|------|------|
| 实现最简单 | 所有设备必须相同 PIN |
| 无需改造现有加密 | 违背"每设备独立 PIN"安全模型 |
| 天然无冲突 | 用户换 PIN = 全设备重新配置 |

### 方案 B：按凭据级别同步（推荐）

```
每设备独立 PIN → 云端存加密凭据列表 → 各自解密合并
```

| 优点 | 缺点 |
|------|------|
| 每设备独立 PIN | 实现复杂 |
| 支持增量同步 | 需要全新同步架构 |
| 支持冲突合并 | 工作量大 |

### 方案 C：双层加密

```
凭据值用 deviceKey 加密 → 再用 syncKey 加密 → 云端存双层加密数据
各设备用 syncKey 解密外层 → 用各自 deviceKey 解密内层
```

**问题：** 每设备的 deviceKey 不同，即使解密了外层，内层也只能在自己的设备上解密。

---

## 三、推荐方案：统一 Sync Key + .db 文件同步

### 核心思路

```
保留 Android 现有的 .db 结构不变
引入独立 Sync Key（与 PIN 无关）
云端同步的是用 Sync Key 加密的 .db 文件
各设备下载后用 Sync Key 解密 → 得到 .db → 用自己的 PIN 解锁 .db
```

### 架构

```
┌─────────────────────────────────────────────────────┐
│                    Cloud Storage                     │
│                                                      │
│  lockit-sync/                                        │
│  ├── vault.enc          ← 用 Sync Key 加密的 .db      │
│  ├── manifest.json      ← 同步元数据                  │
│  └── lock               ← 同步锁（防并发写入）         │
└─────────────────────────────────────────────────────┘
       ↑ sync push/pull                      ↑ sync push/pull
       │ 用 Sync Key 加密/解密               │ 用 Sync Key 加密/解密
┌──────────────┐                    ┌──────────────┐
│  Android App  │                    │   CLI        │
│              │                    │              │
│ PIN=123456   │                    │ PIN=654321   │
│ masterKey=A  │                    │ masterKey=B  │
│              │                    │              │
│ local vault  │                    │ local vault  │
│ .db 用 A 加密│                    │ .db 用 B 加密│
└──────────────┘                    └──────────────┘
```

### 工作流程

#### Push（本地 → 云端）

```
1. 解锁本地 vault.db（用 PIN）
2. 将 vault.db 整体用 Sync Key 加密 → vault.enc
3. 下载云端 manifest.json
4. 对比 checksum：
   - 相同 → 无需同步
   - 不同 → 进入冲突检测
5. 检查云端 lock：
   - 有锁 → 等待或跳过
   - 无锁 → 创建锁
6. 上传 vault.enc
7. 更新 manifest.json（checksum + timestamp）
8. 释放锁
```

#### Pull（云端 → 本地）

```
1. 下载云端 manifest.json
2. 对比 checksum：
   - 相同 → 无需同步
   - 不同 → 云端有更新
3. 下载 vault.enc
4. 用 Sync Key 解密 → vault.db
5. 替换本地 vault.db
6. 更新本地 manifest.json
```

#### 双向同步（sync both）

```
1. 比较本地和云端的 updatedAt
2. 云端更新 → Pull
3. 本地更新 → Push
4. 都更新 → 冲突 → 提示用户选择：
   a. 保留云端（Pull 覆盖本地）
   b. 保留本地（Push 覆盖云端）
   c. 都保留（导出本地变更，Pull 后再导入）
```

### Sync Key 管理

```
同步密钥 = 随机生成 256-bit

首次同步时生成 → 各设备通过安全渠道共享（扫码/手动输入）
```

**推荐方案：**
- 生成 256-bit 随机同步密钥
- 首次设置时，一个设备生成，另一个设备扫码/输入获取
- 同步密钥存储在 Keychain（各设备独立存储）

---

## 四、云端数据格式

### manifest.json（同步元数据）

```json
{
  "version": 1,
  "syncId": "sync-uuid-here",
  "updatedAt": "2026-04-22T12:00:00Z",
  "vaultChecksum": "sha256:abc123...",
  "updatedBy": "android-device-1",
  "encryptedSize": 524288
}
```

### vault.enc 加密格式

```
+------------------+
| 版本 (1 byte)     |  v1
+------------------+
| Nonce (12 bytes)  |  AES-GCM nonce
+------------------+
| 加密数据           |  AES-256-GCM 加密后的 vault.db
+------------------+
| Tag (16 bytes)    |  AES-GCM auth tag
+------------------+
```

---

## 五、Android 端改造

### 5.1 现有代码问题

`GoogleDriveSyncManager` 需要完全重写：

**删除：**
```kotlin
// ❌ 整个 .db 上传，破坏性
suspend fun uploadVault(account, dbFilePath)
```

**新增：**
```kotlin
interface SyncManager {
    suspend fun syncPush(): SyncResult
    suspend fun syncPull(): SyncResult
    suspend fun syncBoth(): SyncResult
    suspend fun getSyncStatus(): SyncStatus
    suspend fun configureBackend(config: SyncBackendConfig): Result<Unit>
}

data class SyncResult(
    val uploaded: Int,
    val downloaded: Int,
    val conflicts: Int,
    val errors: List<String>
)
```

### 5.2 改造方案

**新增文件：**
```
app/src/main/java/com/lockit/data/sync/
├── SyncManager.kt           // 同步接口
├── SyncCrypto.kt            // 同步密钥加密/解密
├── SyncManifest.kt          // manifest 解析
├── GoogleDriveSync.kt       // Google Drive 后端
├── S3Sync.kt               // S3 后端（未来）
├── WebDAVSync.kt           // WebDAV 后端（未来）
└── ConflictResolver.kt     // 冲突解决
```

### 5.3 SyncCrypto

```kotlin
object SyncCrypto {
    private const val ALGORITHM = "AES/GCM/NoPadding"
    private const val KEY_SIZE = 32  // 256-bit
    private const val NONCE_SIZE = 12
    private const val TAG_SIZE = 16
    
    fun generateSyncKey(): ByteArray {
        val key = ByteArray(KEY_SIZE)
        SecureRandom().nextBytes(key)
        return key
    }
    
    fun encrypt(data: ByteArray, key: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(ALGORITHM)
        val nonce = ByteArray(NONCE_SIZE)
        SecureRandom().nextBytes(nonce)
        
        val secretKey = SecretKeySpec(key, "AES")
        val gcmSpec = GCMParameterSpec(TAG_SIZE * 8, nonce)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey, gcmSpec)
        
        val encrypted = cipher.doFinal(data)
        
        // 版本(1) + nonce(12) + encrypted + tag(16)
        return ByteBuffer.allocate(1 + NONCE_SIZE + encrypted.size)
            .put(0x01.toByte())
            .put(nonce)
            .put(encrypted)
            .array()
    }
    
    fun decrypt(data: ByteArray, key: ByteArray): ByteArray {
        val buffer = ByteBuffer.wrap(data)
        val version = buffer.get()
        if (version != 0x01.toByte()) throw SyncError.InvalidVersion(version)
        
        val nonce = ByteArray(NONCE_SIZE)
        buffer.get(nonce)
        
        val encrypted = ByteArray(buffer.remaining())
        buffer.get(encrypted)
        
        val cipher = Cipher.getInstance(ALGORITHM)
        val secretKey = SecretKeySpec(key, "AES")
        val gcmSpec = GCMParameterSpec(TAG_SIZE * 8, nonce)
        cipher.init(Cipher.DECRYPT_MODE, secretKey, gcmSpec)
        
        return cipher.doFinal(encrypted)
    }
}
```

### 5.4 Sync Key 设置 UI

```
Android 端：
  设置 → 同步 → 设置同步密钥
    ├── 生成新密钥 → 显示为二维码 + 6 组字符
    └── 输入已有密钥 → 粘贴 / 扫码

  生成密钥示例:
  ┌─────────────────────┐
  │   [QR CODE]         │
  │                     │
  │ Sync Key:           │
  │ A7B2C9 - D4E8F1     │
  │ 3G6H0J - K2L5M8     │
  │ N1P4Q7 - R9S3T6     │
  │                     │
  │ [复制] [分享] [确认] │
  └─────────────────────┘
```

---

## 六、CLI 端实现

### 6.1 同步流程

```bash
# 首次设置
lk sync config
# → 选择后端（Google Drive / S3 / WebDAV）
# → 生成同步密钥（或输入已有的）
# → 保存到 ~/.config/lockit/config.toml

# 推送
lk sync push

# 拉取
lk sync pull

# 双向同步
lk sync

# 状态
lk sync status
```

### 6.2 CLI 同步配置

```toml
# ~/.config/lockit/config.toml

[sync]
enabled = true
backend = "google_drive"
sync_key = "env:LOCKIT_SYNC_KEY"  # 256-bit 密钥
conflict_resolution = "prompt"    # prompt | remote | local | duplicate

[sync.google_drive]
# OAuth2 token stored in system keychain
# No additional config needed

[sync.s3]
# region = "cn-beijing"
# endpoint = "https://oss-cn-beijing.aliyuncs.com"
# bucket = "lockit-vault"
# prefix = "sync/"
# access_key_id = "env:AWS_ACCESS_KEY_ID"
# secret_access_key = "env:AWS_SECRET_ACCESS_KEY"

[sync.webdav]
# url = "https://dav.jianguoyun.com/dav/lockit/"
# username = "env:WEBDAV_USER"
# password = "env:WEBDAV_PASS"
```

---

## 七、冲突解决

### 检测方式

```
Push 时：
  1. 下载云端 manifest
  2. 云端 checksum ≠ 本地 checksum
  3. → 冲突：云端在此期间被其他设备修改了

Pull 时：
  1. 下载云端 manifest
  2. 云端 updatedAt > 本地 updatedAt
  3. 但本地也有未同步的修改
  4. → 冲突
```

### 解决策略

| 策略 | 行为 | 适用场景 |
|------|------|----------|
| 保留云端 | Pull 覆盖本地 | 云端数据更新 |
| 保留本地 | Push 覆盖云端 | 本地数据更新 |
| 导出本地 | 导出本地 .db 备份，Pull 云端 | 不确定时 |

### CLI 冲突处理

```bash
$ lk sync push
⚠ Conflict detected:
   Local updated:  2026-04-22 12:00
   Remote updated: 2026-04-22 11:30 (by android-pixel8)
   
   [1] Keep remote (pull and overwrite local)
   [2] Keep local (push and overwrite remote)
   [3] Export local backup, then pull remote
   [4] Cancel
> 3

✓ Local vault backed up to: vault.db.20260422-120000.backup
✓ Remote vault pulled successfully
```

---

## 八、Sync Key 安全

### 存储

| 平台 | 存储位置 |
|------|----------|
| Android | EncryptedSharedPreferences / AndroidKeyStore |
| macOS | Keychain |
| Linux | Secret Service (GNOME Keyring / KWallet) |
| Windows | Credential Manager |
| CLI 默认 | `~/.config/lockit/sync.key` (文件权限 600) |

### 传输

```
Android → CLI:
  1. Android 生成 Sync Key
  2. 显示为二维码 + 文本
  3. CLI 扫码或手动输入

CLI → Android:
  1. CLI 生成 Sync Key
  2. 显示为文本
  3. Android 手动输入
```

---

## 九、数据迁移

### 从旧格式迁移到新格式

```
旧格式（当前）:
  Google Drive AppData/
  └── lockit_vault_backup.db  ← 整个 .db

迁移流程:
  1. 下载旧的 .db
  2. 用 Android PIN 解密
  3. 生成同步密钥
  4. 按条加密上传到新目录 lockit-sync/
  5. 生成 manifest.json
  6. 删除旧文件
  7. CLI 拉取 → 解密 → 写入本地 vault.db
```

---

## 十、实施计划

### Phase 1：Android 改造（核心）

| 步骤 | 内容 | 预估 |
|------|------|------|
| 1 | 新增 SyncCrypto 加密/解密 | 0.5 天 |
| 2 | 新增 SyncManager 接口 | 0.5 天 |
| 3 | 改造 GoogleDriveSyncManager | 1 天 |
| 4 | 新增 Sync Key 设置 UI | 1 天 |
| 5 | 新增同步状态显示 | 0.5 天 |
| 6 | 迁移测试 | 0.5 天 |

### Phase 2：CLI 同步（2 周）

| 步骤 | 内容 | 预估 |
|------|------|------|
| 1 | Rust SyncCrypto 实现 | 0.5 天 |
| 2 | SyncBackend trait + Google Drive | 1 天 |
| 3 | lk sync push/pull/status 命令 | 1 天 |
| 4 | 冲突解决逻辑 | 1 天 |

### Phase 3：多后端（2 周）

| 步骤 | 内容 | 预估 |
|------|------|------|
| 1 | S3 后端 | 1 天 |
| 2 | WebDAV 后端 | 1 天 |
| 3 | Git 后端 | 1 天 |

---

## 十一、关键设计决策

### Q: 为什么同步整个 .db 而不是按条目？

**A:** 
- vault.db 通常很小（几十到几百 KB）
- 按条目同步需要定义通用 JSON schema，改造量巨大
- 冲突概率低（用户不会频繁在两个设备同时修改）
- 未来可以优化为按条目增量同步

### Q: Sync Key 和 PIN 的关系？

**A: 完全独立。**
- PIN → 解锁本地 vault.db
- Sync Key → 加密云端 vault.enc
- 不同设备可以不同 PIN，但必须相同 Sync Key

### Q: 如果 Sync Key 丢失了怎么办？

**A:**
- 无法从云端恢复数据（数据用 Sync Key 加密）
- 但本地 vault.db 仍可用（用 PIN 解锁）
- 解决：生成新 Sync Key → 从本地重新 Push

### Q: 云端 .db 被泄露了怎么办？

**A:**
- 云端是 AES-256-GCM 加密的
- 没有 Sync Key = 无法解密
- Sync Key 永不上传
