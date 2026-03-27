# lockit-sync 功能文档

## 一、概览

lockit-sync 提供可插拔的 vault 同步引擎。通过 `SyncBackend` trait 抽象所有同步操作，支持多种后端。

**核心组件：**
- `SyncBackend` trait：统一接口（upload/download/list/delete/metadata）
- `SyncBackendFactory`：从配置构造后端
- `SyncMetadata`：同步元数据（版本号、时间戳、校验和）
- 后端实现：`LocalBackend`、`S3Backend`、`MockBackend`

---

## 二、SyncBackend Trait

```rust
#[async_trait]
pub trait SyncBackend: Send + Sync {
    async fn upload(&self, key: &str, data: &[u8]) -> Result<()>;
    async fn download(&self, key: &str) -> Result<Vec<u8>>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn metadata(&self, key: &str) -> Result<SyncMetadata>;
    fn backend_name(&self) -> &str;
}
```

### SyncMetadata

```rust
pub struct SyncMetadata {
    pub version: u64,           // 单调递增版本号
    pub last_modified: u64,     // Unix 时间戳（秒）
    pub checksum: String,       // Hex 编码的 SHA-256
    pub size: u64,              // 内容字节数
}
```

---

## 三、后端配置

### 3.1 Local（本地目录）

```toml
[sync]
backend = "local"
path = "/home/user/sync-vault"
```

适用于：NFS 共享、USB 挂载、测试。

### 3.2 S3（兼容所有 S3 协议）

```toml
[sync]
backend = "s3"
bucket = "my-lockit-vault"
prefix = "lockit/"            # 可选，key 前缀
endpoint = "https://..."      # 可选，非 AWS 时填写
region = "us-east-1"
path_style = false            # MinIO 需要 true
access_key_id = ""            # 可选，从环境变量读取
secret_access_key = ""        # 可选，从环境变量读取
```

支持：AWS S3、阿里云 OSS、腾讯 COS、华为 OBS、MinIO、七牛 Kodo。

### 3.3 Git（未实现）

```toml
[sync]
backend = "git"
repo_url = "git@gitee.com:xqicxx/lockit-vault.git"
branch = "main"
```

### 3.4 WebDAV（未实现）

```toml
[sync]
backend = "webdav"
url = "https://dav.jianguoyun.com/dav/lockit/"
username = "xxx"
password = "xxx"
```

---

## 四、S3 Backend 实现细节

### 4.1 upload

```rust
async fn upload(&self, key: &str, data: &[u8]) -> Result<()>
```

- 使用 `put_object` 直接覆盖远端
- 无冲突检测（last-write-wins）
- data 通过 `ByteStream::from(data.to_vec())` 上传

### 4.2 download

```rust
async fn download(&self, key: &str) -> Result<Vec<u8>>
```

- 通过 `get_object` 下载
- 错误消息包含 "NoSuchKey" 或 "404" 时，返回 `Error::NotFound`

### 4.3 list

```rust
async fn list(&self, prefix: &str) -> Result<Vec<String>>
```

- 使用 `list_objects_v2` 分页查询
- 支持 continuation token 分页
- 结果按字典序排序返回

### 4.4 metadata

```rust
async fn metadata(&self, key: &str) -> Result<SyncMetadata>
```

- 使用 `head_object` 获取元数据
- last_modified 从 S3 的 LastModified 字段获取
- checksum 使用 E-Tag（去除引号）
- **注意：** E-Tag 不一定是 SHA-256（多段上传时格式不同）

---

## 五、Factory

```rust
pub struct SyncBackendFactory;

impl SyncBackendFactory {
    pub fn from_config(config: BackendConfig) -> Result<Box<dyn SyncBackend>>
}
```

```rust
pub enum BackendConfig {
    Local(LocalConfig),
    S3(S3Config),
    WebDav(WebDavConfig),
    Git(GitConfig),
}
```

从 config 构造对应的后端实例。未实现的后端返回 `Error::NotImplemented`。

---

## 六、同步流程（lockit-cli 层）

```
lk sync push:
  1. 读取本地 vault 文件
  2. 计算 SHA-256 checksum
  3. 获取远端 metadata
  4. 对比本地 vs 远端 checksum
  5. 如果本地 != 远端 → 上传
  6. 无冲突检测（last-write-wins）

lk sync pull:
  1. 获取远端 metadata
  2. 获取本地 checksum（如果有）
  3. 对比远端 vs 本地 checksum
  4. 如果远端 != 本地 → 下载覆盖本地 vault 文件

lk sync status:
  1. 显示本地 checksum + 修改时间
  2. 显示远端 checksum + 修改时间
  3. 标记是否同步
```

---

## 七、已知问题

| 问题 | 编号 | 说明 |
|------|------|------|
| 无冲突检测 | #72 | last-write-wins 可能丢失修改 |
| 无版本冲突解决 | #72 | 两台设备同时修改时谁赢？ |
| S3 metadata unwrap_or(0) | #79 | last_modified 缺失时返回 0 |
| 随着 vault 增大同步变慢 | — | 全量上传，无增量 |

---

## 八、依赖

| 依赖 | 用途 |
|------|------|
| aws-sdk-s3 | S3 客户端 |
| aws-credential-types | AWS 凭据 |
| async-trait | async trait 支持 |
| serde | 序列化 |
| thiserror | 错误类型 |
| tokio | 异步运行时（S3 用） |
| tracing | 日志 |
