# lockit-cli 功能文档

## 一、概览

lockit-cli 是 lockit 的命令行界面，封装所有用户操作。CLI 不直接接触加密逻辑，通过 `lockit-core` 的 `UnlockedVault` API 和 `lockit-ipc` 的 IPC 协议完成所有功能。

**公共入口：** `lk` 命令

---

## 二、命令一览

| 命令 | 功能 | 需要密码？ |
|------|------|----------|
| `lk init` | 创建新 vault | 是（设置密码） |
| `lk add` | 添加/更新凭据 | 是（自动检测） |
| `lk get` | 读取凭据 | 是 |
| `lk list` | 列出 profiles | 是 |
| `lk delete` | 删除凭据 | 是 |
| `lk run` | 注入凭据执行命令 | 是 |
| `lk export` | 导出凭据到 stdout | 是 |
| `lk import` | 从 .env 导入凭据 | 是 |
| `lk recover` | 助记词恢复密码 | 否（用助记词） |
| `lk sync push/pull` | 同步 vault | 是 |
| `lk daemon start/stop` | 管理 daemon | 否 |
| `lk generate-completion` | 生成 shell 补全 | 否 |

---

## 三、核心命令详解

### 3.1 lk init

```bash
lk init
```

交互流程：
1. 检查 vault 是否已存在（`~/.lockit/vault.lockit`）
2. 提示输入新密码（两次确认）
3. 验证密码强度（≥12 字符）
4. 生成 vault + BIP39 助记词
5. 显示助记词（24 词，分 4 行显示，每行 6 词）
6. 要求用户输入 "yes" 确认已保存
7. 保存 vault 到磁盘

```
╔══════════════════════════════════════════════════╗
║          RECOVERY PHRASE — SAVE THIS NOW         ║
╚══════════════════════════════════════════════════╝

  1. word1    2. word2    3. word3    ...
```

### 3.2 lk add

```bash
lk add <profile> --key <key> --value <value>
lk add <profile> --key <key>                    # 交互式输入（隐藏输入）
lk add <profile> --from-env                     # 导入匹配的环境变量
lk add <profile> --from-dotenv <file>           # 从 .env 文件导入
```

- 如果 vault 不存在，自动创建（显示助记词一次）
- 如果 key 已存在，提示确认覆盖
- `--from-env` 导入所有环境变量前缀匹配的（如 `lk add aws --from-env AWS_`）

### 3.3 lk get

```bash
lk get <profile> <key>              # 输出纯文本
lk get <profile> <key> --json       # JSON 格式
lk get <profile> <key> --export     # export KEY=VALUE 格式
lk get <profile>                    # 列出该 profile 的所有 key
```

**输出规则：**
- 正常值 → stdout
- 错误信息 → stderr
- key 不存在 → exit code 1

### 3.4 lk run

```bash
lk run -- <command>                         # 所有凭据注入为环境变量
lk run --prefix <PREFIX> -- <command>       # 加前缀注入
lk run --no-inherit -- <command>            # 清空原有环境变量
lk run --profile <p> -- <command>           # 只注入指定 profile
```

注入逻辑：
1. 从 vault 读取所有凭据（或指定 profile）
2. key 转大写 → 环境变量名
3. 前缀可选（`--prefix GITHUB_` → `GITHUB_TOKEN`）
4. 透传子进程 exit code

### 3.5 lk recover

```bash
lk recover
```

1. 提示输入 24 词助记词（空格分隔）
2. 提示设置新密码（两次确认）
3. 验证助记词匹配 vault 文件中的 recovery_wrapped_vek
4. 重置密码，原密码失效

### 3.6 lk export / import

```bash
lk export                    # 所有凭据，INI 格式
lk export --json             # JSON 格式
lk export <profile>          # 指定 profile

lk import <profile> <file>   # 从文件导入
lk import <profile> -        # 从 stdin 导入
```

INI 格式兼容 AWS CLI 的 `~/.aws/credentials` 格式。

---

## 四、Vault 文件管理

### 4.1 文件路径

```
~/.lockit/
├── vault.lockit          # 加密 vault 文件
├── device.key            # 设备密钥（32 bytes，权限 0600）
├── credentials           # 明文凭据（INI 格式，权限 0600）
├── config.toml           # 配置文件（同步后端等）
├── daemon.pid            # daemon PID 文件
└── daemon.sock           # IPC socket 文件
```

### 4.2 设备密钥

首次运行时自动生成 32 字节随机密钥，存入 `~/.lockit/device.key`（Unix 权限 0600）。

设备密钥 + 密码 = 双因素。即使 vault 文件和密码同时泄露，没有 device key 也无法解密。

---

## 五、密码强度验证

```rust
fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 12 {
        bail!("Password too short — minimum 12 characters required.");
    }
    Ok(())
}
```

当前只检查长度，不检查复杂度。

---

## 六、Shell 行为

### 6.1 Shell 转义

`lk run` 注入环境变量时，`lk get --export` 使用 shell_quote 转义：

- 包含空格/特殊字符的值用单引号包裹
- 单引号内部的单引号转义为 `'\''`
- 特殊字符：`Space  \t  "  '  \  $  \  `  !  \n  ;`

### 6.2 Shell 补全

```bash
lk generate-completion bash > ~/.local/share/bash-completion/completions/lk
lk generate-completion zsh > ~/.local/share/zsh/site-functions/_lk
lk generate-completion fish > ~/.config/fish/completions/lk.fish
```

使用 clap_complete 生成。

---

## 七、错误码

| exit code | 含义 |
|-----------|------|
| 0 | 成功 |
| 1 | 通用错误（密码错误、key 不存在、子进程失败等） |
| 2 | 参数错误 |

当前所有错误共用 exit code 1，无法区分。

---

## 八、关键设计决策

1. **Vault 不存在时自动创建**：`lk add` 如果 vault 不存在，自动初始化，显示助记词。避免用户忘记 `lk init`。
2. **明文凭据文件自动同步**：每次 add/delete 都自动写入 `~/.lockit/credentials`，兼容 AWS CLI 等工具。
3. **助记词只显示一次**：`lk init` 和 `lk add`（自动创建）只在创建时显示助记词，之后不存储、不再显示。
4. **stdin 交互**：输入密码时用 `rpassword`，隐藏终端输入；值未提供时也交互输入。

---

## 九、依赖

| 依赖 | 用途 |
|------|------|
| clap | CLI 参数解析 |
| rpassword | 隐藏密码输入 |
| home | 获取 home 目录 |
| tracing-subscriber | 日志输出 |
| lockit-core | 加密核心 |
| lockit-ipc | daemon 通信 |
| lockit-sync | 同步引擎 |
| lockit-sdk | （未使用，预留给未来） |
