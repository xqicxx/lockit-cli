# lockit

[English](README.md) | 安全凭证管理器——加密保险库、云端同步、AI 编程计划配额追踪。

## 架构

```
crates/
├── lockit-core/     # 核心库（加密、保险库、同步、编程计划）
└── lockit-cli/      # CLI 二进制（14 个子命令，基于 clap）
```

## 快速开始

```bash
# 编译
cargo build --release

# 初始化保险库
lockit init
#  → 创建 ~/.local/share/lockit/vault.enc

# 指定自定义路径
lockit --vault ./my-vault.enc init
```

## 核心操作

```bash
# 交互式添加凭证
lockit add

# 通过管道添加
echo '{"type":"api_key","name":"OPENAI","fields":{"secret_value":"sk-abc"}}' | lockit add --stdin

# 列出所有凭证（表格）
lockit list

# 列出（JSON）
lockit list --json

# 按名称或 ID 前缀查看
lockit show openai

# 查看明文
lockit reveal openai secret_value

# 交互式编辑
lockit edit openai

# 删除
lockit delete openai
```

## Shell 集成

```bash
# 输出 export 语句，可 eval 注入 shell
lockit env OPENAI
# → export OPENAI_SECRET_VALUE='sk-abc'

# 在子进程中注入环境变量运行命令
lockit run OPENAI -- curl -H "Authorization: Bearer $OPENAI_SECRET_VALUE" api.example.com
```

## 导出与导入

```bash
# 导出全部凭证（备份）
lockit export --json > backup.json

# 从备份导入
lockit import backup.json
```

## 云同步（Google Drive）

```bash
# OAuth 登录（打开浏览器）
lockit login

# 查看登录状态
lockit whoami

# 同步操作
lockit sync status   # 查看状态
lockit sync push     # 推送到云端
lockit sync pull     # 从云端拉取
lockit sync config   # 配置同步参数
```

## 编程计划配额

```bash
# 列出编程计划凭证
lockit coding-plan list

# 刷新指定 provider 的配额
lockit coding-plan refresh openai
```

## 凭证类型

支持 18 种凭证类型：`api_key`、`github`、`account`、`coding_plan`、`password`、`phone`、`bank_card`、`email`、`token`、`ssh_key`、`webhook_secret`、`oauth_client`、`aws_credential`、`gpg_key`、`database_url`、`id_card`、`note`、`custom`

## 安全

- **AES-256-GCM** 认证加密
- **Argon2id** 密钥派生
- 显示输出自动遮盖敏感值，仅 `reveal` 命令可查看明文
- 原子写入（先写临时文件再 rename，防止断电损坏）
- 审计日志记录所有安全事件

## 环境变量

| 变量 | 说明 |
|------|------|
| `LOCKIT_MASTER_PASSWORD` | 保险库密码（跳过交互式输入） |

## 开发

```bash
# 运行测试
cargo test

# 运行 lint
cargo clippy --all-targets -- -D warnings

# 格式化代码
cargo fmt --all
```

## License

MIT
