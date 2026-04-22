# 密码管理器 CLI + Agent 读取方案调研

> Lockit CLI 设计参考

---

## 一、1Password CLI（`op`）— 业界标杆

### 基本信息

| 项目 | 值 |
|------|-----|
| 命令 | `op` |
| 认证 | Touch ID / Windows Hello / biometric（通过桌面 app） |
| 缓存 | daemon 进程，内存加密存储 |
| 安装 | `brew install 1password-cli` / 官方下载 |

### 认证流程

```bash
op signin              # 生物识别解锁
op whoami              # 检查登录状态，返回账号信息
```

### 读取凭据

**读取整个 item（JSON）：**
```bash
op item get "GitHub" --format json
```

**JSON 输出结构：**
```json
{
  "id": "abc123def456",
  "title": "GitHub",
  "vault": { "id": "xyz", "name": "Private" },
  "category": "LOGIN",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-06-20T14:22:00Z",
  "fields": [
    {
      "id": "username",
      "type": "STRING",
      "label": "username",
      "value": "user@example.com",
      "purpose": "USERNAME"
    },
    {
      "id": "password",
      "type": "CONCEALED",
      "label": "password",
      "value": "s3cretP@ss!",
      "purpose": "PASSWORD",
      "reference": "op://Private/GitHub/password"
    },
    {
      "id": "notesPlain",
      "type": "STRING",
      "label": "notes",
      "value": "API token: sk-xxx"
    }
  ],
  "sections": [
    {
      "id": "custom_section",
      "label": "API Details"
    }
  ]
}
```

**读取单个字段：**
```bash
op item get "GitHub" --format json --fields label=password,label=username
```

**Secret Reference URI（核心创新）：**
```bash
# 语法: op://vault/item/section/field?attribute=value
op read "op://Private/GitHub/password"
op read "op://vault/DB/Database Credentials/host"
op read "op://dev/Stripe/publishable-key"

# 带查询参数
op read "op://vault/Item/SSH+Key/private+key?ssh-format=openssh"
op read "op://vault/Item/one-time+password?attribute=otp"
op read "op://vault/Item/field?attribute=type"     # 返回字段类型
op read "op://vault/Item/field?attribute=value"    # 返回值
op read "op://vault/Item/field?attribute=id"       # 返回字段 ID
```

### Agent 注入（三大核心命令）

**1. `op run` — 环境变量注入**
```bash
# 设置 secret reference 为环境变量
DB_USER="op://vault/DB/username" \
DB_PASS="op://vault/DB/password" \
op run -- node app.js

# 使用 .env 文件
# node.env:
#   DATABASE_URL="op://vault/DB/connection-string"
#   API_KEY="op://vault/API/service-key"
op run --env-file node.env -- python main.py

# 子进程输出自动屏蔽密码（--no-masking 可关闭）
```

**2. `op inject` — 配置文件注入**
```bash
# 模板文件 config.yml.tpl:
#   database:
#     password: "op://vault/DB/password"
cat config.yml.tpl | op inject > config.yml

# 直接读写文件
op inject -i config.yml.tpl -o config.yml
```

**3. `op read` — 输出到 stdout 或文件**
```bash
# 输出到 stdout
op read "op://vault/Item/password"

# 写入文件
op read "op://vault/Item/SSH+Key/private+key?ssh-format=openssh" -o ~/.ssh/id_rsa
```

### Service Accounts（机器账户 / Agent 专用）

```bash
# 创建机器账户（scoped access token，无交互）
op service-account create --name "CI/CD" --vault "Deploy"

# 使用 access token（适合 CI/CD、Agent）
export OP_SERVICE_ACCOUNT_TOKEN="ops_xxx"
op read "op://Deploy/DB/password"
# 不需要 op signin，token 自带权限范围
```

### Shell Plugins（CLI 认证代理）

```bash
# 存储 CLI 工具的 API key 到 1Password
op plugin init gh           # GitHub CLI
op plugin init aws          # AWS CLI

# 之后使用这些 CLI 时自动从 1Password 取凭据
gh pr list                  # 自动用 1Password 里的 token
```

### 字段类型系统

| 类型 | 说明 | 示例 |
|------|------|------|
| `STRING` | 明文文本 | username, notes |
| `CONCEALED` | 隐藏文本（密码） | password, API key |
| `EMAIL` | 邮箱地址 | email |
| `URL` | 网址 | website |
| `OTP` | 一次性密码 | TOTP |
| `DATE` | 日期 | expiry |
| `MONTH_YEAR` | 月/年 | credit card expiry |
| `MENU` | 下拉选项 | — |
| `REFERENCE` | 引用其他 item | — |
| `FILE` | 文件附件 | SSH key, cert |

### Item 类别（20 种）

Login, Password, API Credential, Database, Server, Credit Card, Identity, Secure Note, Document, Email Account, Social Security Number, Software License, Wireless Router, Bank Account, Driver License, Outdoor License, Membership, Passport, Reward Program

---

## 二、Bitwarden CLI（`bw`）— 经典方案

### 基本信息

| 项目 | 值 |
|------|-----|
| 命令 | `bw` |
| 认证 | 主密码 + email，session key 保持登录态 |
| 缓存 | session key 环境变量，`data.json` 本地加密存储 |
| 安装 | `npm i -g @bitwarden/cli` |

### 认证流程

```bash
bw login                        # 交互式登录
export BW_SESSION=$(bw unlock --raw)  # 获取 session key
bw unlock --check               # 检查是否解锁
```

### 读取凭据

**快捷读取：**
```bash
bw get password "GitHub"         # 只返回密码
bw get username "GitHub"         # 只返回用户名
bw get totp "GitHub"             # 返回 TOTP 码
bw get uri "GitHub"              # 返回 URI
```

**完整 JSON：**
```bash
bw get item "GitHub"
```

**JSON 输出结构：**
```json
{
  "id": "abc123",
  "organizationId": null,
  "folderId": null,
  "type": 1,
  "reprompt": 0,
  "name": "GitHub",
  "notes": "API token here",
  "favorite": false,
  "login": {
    "username": "user@example.com",
    "password": "s3cretP@ss!",
    "totp": "otpauth://totp/GitHub?secret=xxx",
    "uris": [
      { "uri": "https://github.com", "match": null }
    ]
  },
  "fields": [
    {
      "name": "api_key",
      "value": "sk-xxx",
      "type": 1
    }
  ],
  "passwordHistory": [
    { "password": "old_pass", "lastUsedDate": "2024-01-01T00:00:00Z" }
  ]
}
```

**搜索：**
```bash
bw list items --search "github"
bw get item "GitHub" | jq '.fields[] | select(.name=="api_key") | .value'
```

**字段类型：**
- `0` = Text
- `1` = Hidden
- `2` = Boolean
- `3` = Linked

### 操作凭据

```bash
# 修改
bw get item "GitHub" | jq '.login.password="newpass"' | bw encode | bw edit item <id>

# 创建
bw get template item | jq '.name="New Item" | .login.username="user"' | bw encode | bw create item

# 删除
bw delete item <id>

# 同步
bw sync
```

---

## 三、Bitwarden Secrets Manager（`bws`）— Agent/机器专用

### 基本信息

| 项目 | 值 |
|------|-----|
| 命令 | `bws` |
| 认证 | Machine Account Access Token（无交互） |
| 语言 | Rust |
| 安装 | GitHub Releases / Docker |

### 认证流程

```bash
export BWS_ACCESS_TOKEN="0.xxxxx.xxxxx"
# 或使用 --access-token 参数
```

### 读写凭据

**读取：**
```bash
bws secret get <secret-id>       # JSON 输出
bws secret list                  # 列出所有
bws secret list <project-id>     # 按项目列出
```

**JSON 输出结构：**
```json
{
  "object": "secret",
  "id": "be8e0ad8-d545-4017-a55a-b02f014d4158",
  "organizationId": "10e8cbfa-...",
  "projectId": "e325ea69-...",
  "key": "SES_KEY",
  "value": "0.982492bc-7f37-4475-9e60",
  "note": "API Key for AWS SES",
  "creationDate": "2023-06-28T20:13:20Z",
  "revisionDate": "2023-06-28T20:13:20Z"
}
```

**创建/编辑/删除：**
```bash
bws secret create KEY VALUE <project-id> --note "note"
bws secret edit <id> --key NEW_KEY --value NEW_VALUE
bws secret delete <id1> <id2>
```

**Agent 注入（核心）：**
```bash
# 将 secrets 注入为环境变量后运行命令
bws run -- 'npm start'

# 指定项目
bws run --project-id <id> -- 'python main.py'

# 使用 UUID 作为变量名（POSIX 兼容）
bws run --uuids-as-keynames -- 'echo $_64246aa4_70b3_4332_8587_8b1284ce6d76'

# 不继承父进程环境变量
bws run --no-inherit-env -- 'env'
```

---

## 四、KeePass CLI — 本地数据库

### 基本信息

| 项目 | 值 |
|------|-----|
| 工具 | KPScript（非交互）、kpcli（交互 shell） |
| 认证 | 主密码 + 可选 keyfile |
| 存储 | 本地 .kdbx 加密文件 |
| JSON | ❌ 不支持 |

### KPScript（非交互）

```bash
# 读取密码
KPScript -c:GetEntryString MyDb.kdbx -pw:mypassword -Field:Password -ref-Title:"GitHub"

# 读取用户名
KPScript -c:GetEntryString MyDb.kdbx -pw:mypassword -Field:UserName -ref-Title:"GitHub"

# 查找条目
KPScript -c:FindEntries MyDb.kdbx -pw:mypassword -Field:Title -ref-Title:"GitHub"

# 添加条目
KPScript -c:AddEntry MyDb.kdbx -pw:mypassword -group:"Internet" -title:"GitHub" -username:"user" -password:"pass" -url:"https://github.com"
```

### kpcli（交互 shell）

```bash
kpcli --kdb=MyDb.kdbx --key=keyfile.key

kpcli:/> ls           # 列出当前组
kpcli:/> cd Internet
kpcli:/> ls
kpcli:/> show -f 0    # 显示第 0 个条目所有字段
kpcli:/> show -f 0 -p # 只显示密码
kpcli:/> exit
```

**特点：** 无 JSON 输出、无 API、纯本地文件、无 Agent 友好设计

---

## 五、竞品对比总结

### CLI 能力对比

| 特性 | 1Password (`op`) | Bitwarden (`bw`) | BW Secrets (`bws`) | KeePass |
|------|:-:|:-:|:-:|:-:|
| JSON 输出 | ✅ `--format json` | ✅ 默认 JSON | ✅ 默认 JSON | ❌ |
| 单字段读取 | ✅ `op read URI` | ✅ `bw get password` | ✅ `bws secret get` | ⚠️ KPScript |
| URI 引用 | ✅ `op://vault/item/field` | ❌ | ❌ | ❌ |
| 环境变量注入 | ✅ `op run` | ❌ | ✅ `bws run` | ❌ |
| 配置文件注入 | ✅ `op inject` | ❌ | ❌ | ❌ |
| 机器账户 | ✅ Service Account | ✅ Machine Account | ✅（默认） | ❌ |
| 搜索 | ⚠️ `op item list --tags` | ✅ `--search` | ❌ | ⚠️ kpcli |
| 自定义字段 | ✅ sections + fields | ✅ fields[] | ✅ key-value | ⚠️ |
| OTP 读取 | ✅ `?attribute=otp` | ✅ `bw get totp` | ❌ | ❌ |
| SSH Key 导出 | ✅ `?ssh-format=openssh` | ❌ | ❌ | ❌ |
| 密码生成 | ✅ `--generate-password` | ⚠️ 需脚本 | ❌ | ✅ |
| 缓存机制 | ✅ daemon 进程 | ✅ session key | ✅ state file | ❌ |
| Agent 友好度 | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐ |
| 设计哲学 | 开发者体验优先 | 通用密码管理 | 机器/CI 专用 | 本地安全第一 |

### 认证方式对比

| 方式 | 1Password | Bitwarden | BW Secrets | KeePass |
|------|-----------|-----------|------------|---------|
| 生物识别 | ✅ Touch ID / Hello | ❌ | ❌ | ❌ |
| 主密码 | ✅（可选） | ✅ 必须 | ❌ | ✅ 必须 |
| Access Token | ✅ Service Account | ✅ API Key | ✅ Machine Account | ❌ |
| 桌面 App 集成 | ✅ 必须 | ❌ | ❌ | ❌ |
| 无交互模式 | ✅ | ⚠️ 需 session key | ✅ | ⚠️ 需密码参数 |

---

## 六、Lockit CLI 设计建议

### 核心设计原则

1. **Agent 第一** — Lockit 的目标是 Agent 级别的认证代理基础设施
2. **Secret Reference URI** — 借鉴 `op://` 语法，设计 `lk://` 协议
3. **JSON 输出** — 所有命令支持 `--format json`
4. **无交互模式** — 机器账户 / token 认证，适合 CI/CD 和 Agent

### 建议命令设计

```bash
# 认证
lk signin                    # 生物识别 / PIN 解锁
lk whoami                    # 检查状态
lk token create              # 生成 Agent token（类似 Service Account）

# 读取凭据
lk get <item>                # 读取单个凭据
lk get <item> --format json  # JSON 输出
lk get <item> password       # 只返回密码
lk get <item> api_key        # 只返回指定字段
lk list                      # 列出所有凭据
lk list --type CodingPlan    # 按类型过滤
lk list --search "github"    # 搜索

# Secret Reference URI
lk read "lk://vault/item/field"
lk read "lk://CodingPlan/Bailian/apiKey"
lk read "lk://GitHub/token"

# Agent 注入
lk run -- 'claude "fix the bug"'          # 环境变量注入
lk inject -i config.tpl -o config.yml     # 配置文件注入

# 管理
lk create --type CodingPlan --name "百炼"
lk edit <item> --field apiKey --value "new"
lk delete <item>
```

### Secret Reference URI 设计

```
lk://vault/item/field
lk://CodingPlan/Bailian/apiKey
lk://GitHub/user1/token
lk://Email/gmail/password

# 带查询参数
lk://Item/SSH+Key/private+key?format=openssh
lk://Item/TOTP/field?attribute=otp
lk://CodingPlan/Bailian/cookie?decode=url
```

### Agent Token 设计

```bash
# 创建 Agent token（限定 scope、可撤销、有过期时间）
lk token create \
  --name "Claude Code" \
  --scope "read:CodingPlan" \
  --scope "read:GitHub" \
  --expires 24h

# 使用
export LK_TOKEN="lk_tok_xxx"
lk get "Bailian" apiKey
```

### JSON 输出 Schema

```json
{
  "id": "uuid",
  "name": "百炼 Coding Plan",
  "type": "CodingPlan",
  "service": "qwen_bailian",
  "fields": [
    { "name": "provider", "value": "qwen_bailian", "type": "text" },
    { "name": "apiKey", "value": "sk-xxx", "type": "secret" },
    { "name": "baseUrl", "value": "https://coding.dashscope.aliyuncs.com/v1", "type": "url" },
    { "name": "cookie", "value": "...", "type": "secret" },
    { "name": "rawCurl", "value": "curl ...", "type": "text" }
  ],
  "metadata": {
    "createdAt": "2026-04-22T00:00:00Z",
    "updatedAt": "2026-04-22T00:00:00Z",
    "reference": "lk://CodingPlan/Bailian/apiKey"
  }
}
```

---

## 七、参考链接

- [1Password CLI 文档](https://developer.1password.com/docs/cli/)
- [1Password Secret Reference Syntax](https://developer.1password.com/docs/cli/secret-reference-syntax/)
- [1Password CLI Reference](https://developer.1password.com/docs/cli/reference/)
- [Bitwarden CLI 文档](https://bitwarden.com/help/cli/)
- [Bitwarden Secrets Manager CLI](https://bitwarden.com/help/secrets-manager-cli/)
- [KeePass KPScript](https://keepass.info/help/v2_dev/scr_sc_index.html)
- [kpcli](https://kpcli.sourceforge.io/)
