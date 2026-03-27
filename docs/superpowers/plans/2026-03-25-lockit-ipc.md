# lockit-ipc 功能文档

## 一、概览

lockit-ipc 提供 daemon 和 CLI 之间的通信协议。基于 Unix Domain Socket（Linux/macOS），使用 MessagePack 序列化。

**核心组件：**
- `IpcServer`：daemon 端，监听 socket，接受请求
- `IpcClient`：CLI 端，连接 socket，发送请求
- `Request` / `Response`：协议消息
- `RequestHandler`：daemon 实现的请求处理 trait

---

## 二、协议设计

### 2.1 请求类型

```rust
pub enum Request {
    UnlockVault { password: String, device_key: Vec<u8> },
    LockVault,
    GetCredential { profile: String, key: String },
    SetCredential { profile: String, key: String, value: Vec<u8> },
    DeleteCredential { profile: String, key: String },
    ListProfiles,
    ListKeys { profile: String },
    DaemonStatus,
}
```

### 2.2 响应类型

```rust
pub enum Response {
    Ok,
    Value { value: Option<Vec<u8>> },
    Profiles { profiles: Vec<String> },
    Keys { keys: Vec<String> },
    Status { locked: bool, version: String, uptime_secs: u64 },
    Error { kind: ErrorKind, message: String },
}
```

### 2.3 错误类型

```rust
pub enum ErrorKind {
    VaultLocked,       // vault 未解锁
    IncorrectPassword, // 密码错误
    NotFound,          // key 不存在
    Internal,          // 内部错误
}
```

### 2.4 消息帧格式

```
[4 bytes: payload length (big-endian u32)] [payload: msgpack(RawMessage)]
```

RawMessage 是 Request 或 Response 的 msgpack 编码。4 字节长度头保证消息边界清晰。

---

## 三、Socket 管理

### 3.1 路径

```
~/.lockit/daemon.sock
```

macOS 特殊处理：Unix socket 长度限制 104 字节，路径会自动缩短。

### 3.2 权限

server bind 时自动删除残留的 stale socket。**未显式设置 socket 文件权限**（已知问题）。

### 3.3 生命周期

- `IpcServer::bind()` → 创建 socket，监听
- `IpcServer::serve(handler)` → 接受连接，每个连接 spawn 一个 task
- `Drop for IpcServer` → 删除 socket 文件
- 连接断开 → task 自动结束

---

## 四、连接行为

### 4.1 Server 端

```rust
pub async fn serve<H: RequestHandler>(self, handler: Arc<H>) -> Result<()>
```

- 每个连接 spawn 一个 tokio task
- 同一个连接内支持多个请求（loop 读取直到 EOF）
- 连接断开时自动清理

### 4.2 Client 端

```rust
impl IpcClient {
    pub async fn new_default() -> Result<Self>  // 连接默认 socket
    pub async fn new(path: PathBuf) -> Result<Self>  // 连接指定 socket
    pub async fn send_request(&self, request: &Request) -> Result<Response>
}
```

客户端维护一个持久连接，不每次新建。

---

## 五、帧传输实现

```rust
// 写入
async fn write_message<W: AsyncWrite>(writer: &mut W, msg: &impl Serialize) -> Result<()>
// 1. msgpack 序列化
// 2. 写 4 字节长度头
// 3. 写 payload

// 读取
async fn read_message<R: AsyncRead, T: DeserializeOwned>(reader: &mut R) -> Result<T>
// 1. 读 4 字节长度
// 2. 读 payload
// 3. msgpack 反序列化
```

**长度限制：** 无硬编码上限。如果 payload 超过 `u32::MAX`（4GB），会出问题。

---

## 六、安全考虑

| 风险 | 当前状态 | 改进建议 |
|------|---------|---------|
| 无认证 | ❌ 任意进程可连接 | 使用 SCM_CREDENTIALS 或 token |
| socket 权限 | ❌ 未显式设置 | 设为 0600 |
| 请求频率 | ❌ 无限制 | 添加限速 |
| 大 payload | ❌ 无限制 | 添加最大帧长度（如 10MB） |

---

## 七、依赖

| 依赖 | 用途 |
|------|------|
| tokio | 异步运行时 |
| serde | 序列化框架 |
| rmp-serde | MessagePack 序列化 |
| thiserror | 错误类型 |
| tracing | 日志 |
