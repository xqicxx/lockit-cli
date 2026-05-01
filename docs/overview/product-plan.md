# Lockit Product Plan

## 1. 项目定位

`lockit` 不是单纯的“密码本”，而是一个面向开发者和 agent 的敏感信息编排层。

它需要解决三件事：
- 安全地存储 API、账号密码、邮箱、Cookie、Token、SSH Key 等敏感信息
- 在 CLI、桌面端、Android 等多端之间保持一致的数据和同步体验
- 让 agent 可以通过 CLI 一键拿到正确的 secret，但又尽量避免泄露到日志、历史记录或错误位置

## 2. 目标用户

- 个人开发者：需要集中管理多个 AI / 云服务 API Key
- 自动化使用者：希望脚本、agent、CLI 能按项目自动导入凭据
- 多设备用户：在手机、电脑、不同 shell 环境之间同步 secret

## 3. 核心使用场景

### 场景 A：保存凭据
- 用户通过 CLI、桌面端或 Android 新增一条 credential
- 数据本地加密保存，可选择同步到云盘

### 场景 B：一键导入给 agent
- 用户在项目目录执行 `lockit use openai` 或 `lockit run --profile project-a -- <command>`
- CLI 根据映射关系把需要的环境变量或配置临时注入到当前命令

### 场景 C：跨设备同步
- 用户在 Android 上新增或修改 credential
- 其他设备通过 WebDAV / Google Drive 拉取到同一份加密 vault

### 场景 D：按类型管理
- 不同类型 secret 有不同字段、校验规则、展示方式和导出方式
- 例如 `api_key` 导出为 env，`account` 导出为用户名密码对，`cookie` 导出为 header 或 cookie file

## 4. MVP 范围

### 必做
- 加密 vault
- 主密码与密钥派生
- 基础 credential 增删查改
- CLI 一键导出 env / `.env` / 子进程环境
- WebDAV 同步
- 基础测试与迁移能力

### 次优先级
- Google Drive 同步
- 桌面端图形界面
- 团队共享与权限模型
- 审计与操作记录可视化

### 暂不纳入第一阶段
- 浏览器插件
- 多人实时协作
- 云端托管账户系统

## 5. 第一版建议支持的凭据类型

- `api_key`
- `token`
- `account`
- `cookie`
- `password`
- `ssh_key`
- `oauth_client`
- `webhook_secret`
- `email_account`
- `custom`

## 6. 成功标准

- 用户能在 5 分钟内完成初始化并录入第一批 secret
- 用户能在不手动复制 secret 的前提下，让 agent 或脚本拿到所需凭据
- 用户在第二台设备上可恢复同一份加密 vault
- 任何 secret 默认都不会以明文长期落盘在不受控位置
