# Lockit Roadmap And TODO

## P0: 先把方向定住

- [ ] 冻结统一 credential schema
- [ ] 冻结 vault 文件格式与版本号策略
- [ ] 冻结 manifest 字段
- [ ] 明确 CLI 第一版命令面
- [ ] 明确 Android 旧数据向新格式的兼容策略

## P1: Secure CLI MVP ✅

### 目标
让 `lockit` 先成为一个真正可用、可加密、可测试的 CLI 凭据库。**（已完成）**

### TODO
- [x] 引入 `security` 模块，补齐 Argon2id + AES-256-GCM
- [x] 定义 vault 仓储接口，替换当前明文 Markdown 存储
- [x] 增加 `lockit init` 主密码初始化流程
- [x] 增加 `lockit add/list/get/delete/update`
- [x] 增加 secret redaction，避免默认明文输出
- [x] 增加 schema migration 基础设施
- [x] 为 CLI 命令补齐单元测试和集成测试

### 完成标志
- [x] 本地只保存加密数据
- [x] 不输入正确主密码无法读取 secret
- [x] 测试可自动验证增删查改与解密流程

## P2: Agent Import MVP

### 目标
让 agent 和脚本不需要手动复制 secret，就能按项目拿到正确凭据。

### TODO
- [ ] 设计 `profile` 概念，支持按项目组织凭据
- [ ] 增加 `lockit export env`
- [ ] 增加 `lockit export dotenv`
- [ ] 增加 `lockit run --profile ... -- <command>`
- [ ] 增加按 `service` 自动映射 env name 的策略
- [ ] 为 cookie / ssh key / account 增加专用导出器
- [ ] 为导入过程增加脱敏日志

### 完成标志
- [ ] 可以在新 shell 或子进程中安全注入 secret
- [ ] 默认不会把完整 secret 回显到终端
- [ ] 常见 AI 服务至少支持一键导出

## P3: Sync MVP

### 目标
实现多设备共享同一份加密 vault。

### TODO
- [ ] 复用 Android 端已有思路，抽象 Rust 侧 `SyncBackend`
- [ ] 优先实现 Google Drive backend
- [ ] 实现 manifest 序列化与 checksum
- [ ] 实现 push / pull / status
- [ ] 实现冲突检测与手动解决
- [ ] 增加同步失败重试和错误分级

### 完成标志
- [ ] 两台设备可在 Google Drive 上同步同一 vault
- [ ] manifest 可识别远端更新
- [ ] 发生冲突时不会静默覆盖

## P4: Desktop / Android Alignment

### TODO
- [ ] 提炼共享协议文档
- [ ] 统一 credential type 枚举和字段定义
- [ ] 定义桌面端最小 MVP：查看、搜索、录入、同步、复制
- [ ] 评估 Android 端迁移成本与兼容策略

## 建议命令演进

### 当前已有
- `init`
- `add`
- `get`
- `list`
- `delete`
- `import`
- `export`

### 建议新增
- `update`
- `use`
- `run`
- `sync status`
- `sync push`
- `sync pull`
- `profile add`
- `profile use`

## 优先级排序

1. ~~从明文存储迁移到加密 vault~~ ✅
2. 设计统一 schema + 补类型感知表单
3. 做 agent 一键导入
4. 做 Google Drive 同步
5. 再考虑桌面端和其他云盘
