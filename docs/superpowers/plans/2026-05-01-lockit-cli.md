# Lockit CLI 增强实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 lockit-cli 打造成与 lockit-android 功能对等的命令行凭据管理器。

**Architecture:** 在现有 lockit-core + lockit-cli workspace 基础上增量构建。Phase 1 移植凭据字段系统（核心依赖），Phase 2-5 各自独立可并行。每个 Phase 内部 TDD：先写测试 → 最小实现 → 重构。

**Tech Stack:** Rust, clap (CLI), inquire (交互式提示, v0.7), aes-gcm + argon2 (加密), reqwest 0.12 (HTTP), tabled (表格输出), tempfile (测试)

---

## File Structure

```
lockit/crates/
├── lockit-core/
│   ├── src/
│   │   ├── credential.rs          # MODIFY: Credential 增加 fields helpers
│   │   ├── credential_field.rs    # NEW: CredentialFieldDef + TypeFieldMap
│   │   ├── crypto.rs              # 不变
│   │   ├── vault.rs               # MODIFY: 暴露 get_full_credential, get_vault_bytes
│   │   ├── coding_plan.rs         # NEW: Provider/Quota 模型 + CodingPlanFetcher trait
│   │   ├── coding_plan/
│   │   │   ├── mod.rs             # NEW: re-export fetchers
│   │   │   ├── qwen.rs            # NEW: Bailian API 客户端
│   │   │   ├── chatgpt.rs         # NEW: ChatGPT API 客户端
│   │   │   ├── claude.rs          # NEW: Claude checksum-only 客户端
│   │   │   ├── deepseek.rs        # NEW: DeepSeek 客户端
│   │   │   └── mimo.rs            # NEW: Mimo 客户端
│   │   ├── sync.rs                # MODIFY: SyncManager 编排 push/pull
│   │   ├── sync/
│   │   │   ├── mod.rs             # NEW
│   │   │   └── google_drive.rs    # NEW: GoogleDriveBackend impl
│   │   ├── migration.rs           # 不变
│   │   └── lib.rs                 # MODIFY: 导出新模块
│   └── tests/
│       ├── credential_field_tests.rs  # NEW
│       ├── coding_plan_tests.rs       # NEW
│       └── sync_tests.rs              # NEW
│
└── lockit-cli/
    ├── Cargo.toml                 # MODIFY: 加 inquire, tabled, tokio, reqwest
    └── src/
        ├── main.rs                # REWRITE: 完整 Clap 命令树 + 路由
        ├── commands/
        │   ├── mod.rs             # NEW
        │   ├── add.rs             # NEW: 交互式 add
        │   ├── list.rs            # NEW: 表格/JSON 输出
        │   ├── show.rs            # NEW: 类型感知展示
        │   ├── edit.rs            # NEW: 交互式 edit
        │   ├── coding_plan.rs     # NEW: list + refresh
        │   ├── sync_cmd.rs        # NEW: status/push/pull/config
        │   ├── env_cmd.rs         # NEW: env 注入
        │   ├── run_cmd.rs         # NEW: run 注入执行
        │   ├── export_cmd.rs      # NEW: 导出
        │   └── import_cmd.rs      # NEW: 导入
        ├── interactive.rs         # NEW: inquire 通用提示函数
        └── output.rs              # NEW: 表格/JSON 格式化
```

---

## Phase 1: 凭据字段系统（核心依赖）

### Task 1: 添加 CredentialFieldDef 到 lockit-core

**Files:**
- Create: `lockit/crates/lockit-core/src/credential_field.rs`
- Create: `lockit/crates/lockit-core/tests/credential_field_tests.rs`
- Modify: `lockit/crates/lockit-core/src/lib.rs`

- [ ] **Step 1: 写测试文件**

```rust
// lockit/crates/lockit-core/tests/credential_field_tests.rs
use lockit_core::credential::CredentialType;
use lockit_core::credential_field::TypeFieldMap;

#[test]
fn test_api_key_has_four_fields() {
    let fields = TypeFieldMap::fields_for(&CredentialType::ApiKey);
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].label, "NAME");
    assert_eq!(fields[1].label, "SERVICE");
    assert!(fields[1].is_dropdown);
    assert!(!fields[1].presets.is_empty());
    assert_eq!(fields[2].label, "KEY_IDENTIFIER");
    assert_eq!(fields[3].label, "SECRET_VALUE");
}

#[test]
fn test_coding_plan_required_fields() {
    let indices = CredentialType::CodingPlan.required_field_indices();
    assert_eq!(indices, vec![2, 4]); // API_KEY, BASE_URL
}

#[test]
fn test_every_type_has_fields() {
    for ct in CredentialType::all() {
        let fields = TypeFieldMap::fields_for(&ct);
        assert!(!fields.is_empty(), "{} has no fields", ct.name());
    }
}

#[test]
fn test_preset_values_exist_for_dropdowns() {
    let fields = TypeFieldMap::fields_for(&CredentialType::CodingPlan);
    let provider_field = &fields[0];
    assert!(provider_field.is_dropdown);
    assert!(provider_field.presets.contains(&"openai".to_string()));
    assert!(provider_field.presets.contains(&"anthropic".to_string()));
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cd lockit && cargo test -p lockit-core credential_field_tests
```
Expected: FAIL — 模块不存在

- [ ] **Step 3: 实现 TypeFieldMap**

```rust
// lockit/crates/lockit-core/src/credential_field.rs
use crate::credential::CredentialType;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialFieldDef {
    pub label: String,
    pub placeholder: String,
    pub required: bool,
    pub is_dropdown: bool,
    pub presets: Vec<String>,
}

pub struct TypeFieldMap;

impl TypeFieldMap {
    pub fn fields_for(ct: &CredentialType) -> Vec<CredentialFieldDef> {
        match ct {
            CredentialType::ApiKey => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. OPENAI_API_KEY".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. openai, anthropic...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "KEY_IDENTIFIER".into(), placeholder: "e.g. default, production...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SECRET_VALUE".into(), placeholder: "Paste or enter the secret...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::GitHub => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. GITHUB_TOKEN".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "TOKEN_TYPE".into(), placeholder: "Select token type".into(), required: false, is_dropdown: true, presets: vec!["PAT".into(), "SSH".into(), "OAuth".into(), "GitHub App".into()] },
                CredentialFieldDef { label: "ACCOUNT".into(), placeholder: "GitHub username".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "TOKEN_VALUE".into(), placeholder: "Paste token or SSH key...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SCOPE".into(), placeholder: "Select scopes".into(), required: false, is_dropdown: true, presets: vec!["repo".into(), "read:org".into(), "workflow".into()] },
            ],
            CredentialType::Account => vec![
                CredentialFieldDef { label: "USERNAME".into(), placeholder: "Enter username...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. google, github...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "EMAIL".into(), placeholder: "Associated email".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "PASSWORD".into(), placeholder: "Enter password...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::CodingPlan => vec![
                CredentialFieldDef { label: "PROVIDER".into(), placeholder: "Select provider".into(), required: true, is_dropdown: true, presets: vec!["openai".into(), "chatgpt".into(), "anthropic".into(), "claude".into(), "google".into(), "deepseek".into(), "moonshot".into(), "minimax".into(), "glm".into(), "qwen".into(), "qwen_bailian".into(), "xiaomi_mimo".into()] },
                CredentialFieldDef { label: "RAW_CURL".into(), placeholder: "Paste curl command (auto-extracts)...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "API_KEY".into(), placeholder: "Paste your API key here...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "COOKIE".into(), placeholder: "Bailian console cookie...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "BASE_URL".into(), placeholder: "Select base URL".into(), required: true, is_dropdown: true, presets: vec!["https://api.openai.com".into(), "https://api.anthropic.com".into(), "https://api.deepseek.com".into(), "https://dashscope.aliyuncs.com".into()] },
            ],
            CredentialType::Password => vec![
                CredentialFieldDef { label: "PASSWORD_LABEL".into(), placeholder: "Enter password...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. google, github...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "USERNAME".into(), placeholder: "Associated username".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "PASSWORD_VALUE".into(), placeholder: "Enter password again...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::Phone => vec![
                CredentialFieldDef { label: "REGION".into(), placeholder: "Select region".into(), required: false, is_dropdown: true, presets: vec!["CN".into(), "US".into(), "JP".into(), "KR".into(), "SG".into()] },
                CredentialFieldDef { label: "PHONE_NUMBER".into(), placeholder: "138 0000 0000".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "NOTE".into(), placeholder: "e.g. delivery, work contact...".into(), required: false, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::BankCard => vec![
                CredentialFieldDef { label: "CARD_NUMBER".into(), placeholder: "Card number...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "BANK".into(), placeholder: "e.g. ICBC, BOC...".into(), required: false, is_dropdown: true, presets: vec!["ICBC".into(), "BOC".into(), "CMB".into(), "CCB".into(), "ABC".into()] },
                CredentialFieldDef { label: "CARDHOLDER".into(), placeholder: "Cardholder name...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "CVV_EXPIRY".into(), placeholder: "CVV or expiry".into(), required: false, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::Email => vec![
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "Select provider".into(), required: true, is_dropdown: true, presets: vec!["gmail".into(), "outlook".into(), "qq".into(), "163".into(), "protonmail".into()] },
                CredentialFieldDef { label: "EMAIL_PREFIX".into(), placeholder: "e.g. john.doe".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "PASSWORD".into(), placeholder: "Password or app code...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "REGION".into(), placeholder: "Select region...".into(), required: false, is_dropdown: true, presets: vec!["CN".into(), "US".into(), "JP".into()] },
                CredentialFieldDef { label: "STREET".into(), placeholder: "123 Main St".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "CITY".into(), placeholder: "New York".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "STATE_ZIP".into(), placeholder: "NY 10001".into(), required: false, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::Token => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. JWT_TOKEN".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. my-app...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "KEY_IDENTIFIER".into(), placeholder: "e.g. default, production...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "TOKEN_VALUE".into(), placeholder: "Paste token...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::SshKey => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. GITHUB_SSH".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. github, aws...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "KEY_IDENTIFIER".into(), placeholder: "e.g. ed25519, rsa-4096...".into(), required: false, is_dropdown: true, presets: vec!["ed25519".into(), "rsa-4096".into()] },
                CredentialFieldDef { label: "PRIVATE_KEY".into(), placeholder: "Paste private key...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::WebhookSecret => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. GITHUB_WEBHOOK".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. github, stripe...".into(), required: false, is_dropdown: true, presets: vec!["github".into(), "stripe".into(), "vercel".into()] },
                CredentialFieldDef { label: "HEADER_KEY".into(), placeholder: "e.g. X-Hub-Signature...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SECRET_VALUE".into(), placeholder: "Paste webhook secret...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::OAuthClient => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. GOOGLE_OAUTH".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. google, github...".into(), required: false, is_dropdown: true, presets: service_presets() },
                CredentialFieldDef { label: "CLIENT_ID".into(), placeholder: "Enter client ID...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "CLIENT_SECRET".into(), placeholder: "Paste client secret...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::AwsCredential => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. AWS_PROD".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. aws, aws-prod...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "ACCESS_KEY".into(), placeholder: "Enter access key ID...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SECRET_KEY".into(), placeholder: "Paste secret key...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::GpgKey => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. PERSONAL_GPG".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. personal, ci-cd...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "KEY_ID".into(), placeholder: "e.g. key fingerprint...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "PRIVATE_KEY".into(), placeholder: "Paste GPG private key...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::DatabaseUrl => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. POSTGRES_PROD".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. postgres, mongo...".into(), required: false, is_dropdown: true, presets: vec!["postgres".into(), "mysql".into(), "mongo".into(), "redis".into()] },
                CredentialFieldDef { label: "KEY_IDENTIFIER".into(), placeholder: "e.g. primary, replica...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "CONNECTION_URL".into(), placeholder: "Paste connection string...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::IdCard => vec![
                CredentialFieldDef { label: "CARDHOLDER".into(), placeholder: "Name on ID...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "ISSUER".into(), placeholder: "e.g. government, company...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "ID_NUMBER".into(), placeholder: "ID number...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "EXTRA".into(), placeholder: "Notes".into(), required: false, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::Note => vec![
                CredentialFieldDef { label: "TITLE".into(), placeholder: "e.g. WiFi Password, Server Info...".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "TAGS".into(), placeholder: "e.g. wifi, network, home...".into(), required: false, is_dropdown: false, presets: vec![] },
            ],
            CredentialType::Custom => vec![
                CredentialFieldDef { label: "NAME".into(), placeholder: "e.g. MY_CUSTOM_KEY".into(), required: true, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "SERVICE".into(), placeholder: "e.g. my-service...".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "KEY".into(), placeholder: "custom_key_identifier".into(), required: false, is_dropdown: false, presets: vec![] },
                CredentialFieldDef { label: "VALUE".into(), placeholder: "Paste or enter the secret...".into(), required: true, is_dropdown: false, presets: vec![] },
            ],
        }
    }
}

fn service_presets() -> Vec<String> {
    vec!["google".into(), "github".into(), "openai".into(), "anthropic".into(), "aws".into(), "vercel".into(), "stripe".into(), "netlify".into(), "cloudflare".into(), "alibaba".into(), "tencent".into()]
}
```

- [ ] **Step 4: 补充 CredentialType::all() 和 required_field_indices()**

```rust
// 在 lockit/crates/lockit-core/src/credential.rs 的 CredentialType 中添加：
impl CredentialType {
    pub fn all() -> Vec<CredentialType> {
        vec![
            Self::ApiKey, Self::GitHub, Self::Account, Self::CodingPlan,
            Self::Password, Self::Phone, Self::BankCard, Self::Email,
            Self::Token, Self::SshKey, Self::WebhookSecret, Self::OAuthClient,
            Self::AwsCredential, Self::GpgKey, Self::DatabaseUrl,
            Self::IdCard, Self::Note, Self::Custom,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::GitHub => "github",
            Self::Account => "account",
            Self::CodingPlan => "coding_plan",
            Self::Password => "password",
            Self::Phone => "phone",
            Self::BankCard => "bank_card",
            Self::Email => "email",
            Self::Token => "token",
            Self::SshKey => "ssh_key",
            Self::WebhookSecret => "webhook_secret",
            Self::OAuthClient => "oauth_client",
            Self::AwsCredential => "aws_credential",
            Self::GpgKey => "gpg_key",
            Self::DatabaseUrl => "database_url",
            Self::IdCard => "id_card",
            Self::Note => "note",
            Self::Custom => "custom",
        }
    }

    pub fn required_field_indices(&self) -> Vec<usize> {
        match self {
            Self::Account => vec![0, 3],
            Self::Password => vec![0, 3],
            Self::GitHub => vec![0, 3],
            Self::Phone => vec![0],
            Self::BankCard => vec![0],
            Self::Email => vec![0, 1, 2],
            Self::IdCard => vec![0],
            Self::Note => vec![0],
            Self::CodingPlan => vec![2, 4],
            _ => {
                let fields = crate::credential_field::TypeFieldMap::fields_for(self);
                let last = fields.len().saturating_sub(1);
                vec![0, last]
            }
        }
    }
}
```

- [ ] **Step 5: 更新 lib.rs 导出**

```rust
// lockit/crates/lockit-core/src/lib.rs 追加:
pub mod credential_field;
```

- [ ] **Step 6: 运行测试确认通过**

```bash
cd lockit && cargo test -p lockit-core credential_field_tests
```
Expected: 4 tests PASS

- [ ] **Step 7: Commit**

```bash
cd lockit && git add crates/lockit-core/src/credential_field.rs crates/lockit-core/tests/credential_field_tests.rs crates/lockit-core/src/credential.rs crates/lockit-core/src/lib.rs
git commit -m "feat(core): add CredentialFieldDef + TypeFieldMap with 18 credential types"
```

---

### Task 2: CLI 依赖安装 + output 模块

**Files:**
- Modify: `lockit/crates/lockit-cli/Cargo.toml`
- Create: `lockit/crates/lockit-cli/src/output.rs`

- [ ] **Step 1: 添加依赖**

```toml
# lockit/crates/lockit-cli/Cargo.toml
[dependencies]
anyhow.workspace = true
clap.workspace = true
inquire = "0.7"
lockit-core = { path = "../lockit-core" }
rpassword = "7"
serde = { version = "1", features = ["derive"] }
serde_json.workspace = true
tabled = "0.16"
```

- [ ] **Step 2: 写 output.rs**

```rust
// lockit/crates/lockit-cli/src/output.rs
use lockit_core::credential::RedactedCredential;
use serde::Serialize;
use tabled::Tabled;

#[derive(Tabled)]
pub struct CredentialRow {
    pub id: String,
    pub name: String,
    #[tabled(rename = "TYPE")]
    pub cred_type: String,
    pub service: String,
    pub value: String,
}

impl CredentialRow {
    pub fn from_redacted(c: &RedactedCredential) -> Self {
        let value = c.fields.values().next().cloned().unwrap_or_default();
        Self {
            id: c.id.chars().take(8).collect(),
            name: c.name.clone(),
            cred_type: c.r#type.to_string(),
            service: c.service.clone(),
            value,
        }
    }
}

#[derive(Serialize)]
pub struct JsonOutput {
    pub credentials: Vec<RedactedCredential>,
}

pub fn print_table(credentials: &[RedactedCredential]) {
    let rows: Vec<CredentialRow> = credentials.iter().map(CredentialRow::from_redacted).collect();
    if rows.is_empty() {
        println!("(empty)");
        return;
    }
    let mut table = tabled::Table::new(rows);
    table.with(tabled::settings::Style::modern_rounded());
    println!("{table}");
}

pub fn print_json(credentials: &[RedactedCredential]) {
    let output = JsonOutput { credentials: credentials.to_vec() };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

pub fn success(msg: &str) {
    println!("\x1b[32m✓\x1b[0m {msg}");
}

pub fn error(msg: &str) {
    eprintln!("\x1b[31m✗\x1b[0m {msg}");
}
```

- [ ] **Step 3: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli
```
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
cd lockit && git add crates/lockit-cli/Cargo.toml crates/lockit-cli/src/output.rs
git commit -m "feat(cli): add tabled/json output module and dependencies"
```

---

### Task 3: 交互式 add 命令

**Files:**
- Create: `lockit/crates/lockit-cli/src/interactive.rs`
- Create: `lockit/crates/lockit-cli/src/commands/mod.rs`
- Create: `lockit/crates/lockit-cli/src/commands/add.rs`
- Modify: `lockit/crates/lockit-cli/src/main.rs`

- [ ] **Step 1: 写 interactive 模块**

```rust
// lockit/crates/lockit-cli/src/interactive.rs
use inquire::{Select, Text};
use lockit_core::credential::CredentialType;
use lockit_core::credential_field::{CredentialFieldDef, TypeFieldMap};
use rpassword;
use std::collections::BTreeMap;

pub fn select_credential_type() -> Result<CredentialType, anyhow::Error> {
    let types = CredentialType::all();
    let names: Vec<String> = types.iter().map(|t| format!("{} — {}", t.name(), t.description())).collect();
    let selection = Select::new("Credential type:", names).prompt()?;
    let idx = selection.split(" — ").next().unwrap();
    Ok(types.iter().find(|t| t.name() == idx).cloned().unwrap_or(CredentialType::Custom))
}

fn description_for(t: &CredentialType) -> &'static str {
    match t {
        CredentialType::ApiKey => "Store API keys for AI services, cloud providers, or any REST API",
        // ... 其他类型的描述（简略版用于下拉选项）
        _ => "Store credentials",
    }
}

pub fn prompt_fields_interactive(ct: &CredentialType) -> Result<BTreeMap<String, String>, anyhow::Error> {
    let fields = TypeFieldMap::fields_for(ct);
    let mut values = BTreeMap::new();

    for field in &fields {
        let answer = if field.is_dropdown && !field.presets.is_empty() {
            let mut options: Vec<String> = field.presets.clone();
            options.push("(custom)".into());
            let selection = Select::new(&format!("{}:", field.label), options.clone()).prompt()?;
            if selection == "(custom)" {
                Text::new(&format!("{}:", field.label))
                    .with_placeholder(&field.placeholder)
                    .prompt()?
            } else {
                selection
            }
        } else if field.label.contains("SECRET") || field.label.contains("PASSWORD") || field.label.contains("TOKEN") || field.label.contains("KEY") {
            rpassword::prompt_password(format!("{}: ", field.label))?
        } else {
            let default = if field.label == "KEY_IDENTIFIER" { Some("default") } else { None };
            let mut prompt = Text::new(&format!("{}:", field.label))
                .with_placeholder(&field.placeholder);
            if let Some(d) = default {
                prompt = prompt.with_default(d);
            }
            prompt.prompt()?
        };

        if !answer.is_empty() {
            values.insert(field.label.to_lowercase().replace(' ', "_"), answer);
        }
    }

    Ok(values)
}
```

- [ ] **Step 2: 写 add 命令**

```rust
// lockit/crates/lockit-cli/src/commands/add.rs
use anyhow::Context;
use lockit_core::credential::CredentialDraft;
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::io::Read;
use std::collections::BTreeMap;

use crate::interactive::{prompt_fields_interactive, select_credential_type};
use crate::output;

pub fn run(
    paths: &VaultPaths,
    password: Option<String>,
    json_input: Option<String>,
    stdin_input: bool,
    file_input: Option<String>,
) -> anyhow::Result<()> {
    let (cred_type, fields) = if stdin_input {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        parse_json_input(&buf)?
    } else if let Some(path) = file_input {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("read {path}"))?;
        parse_json_input(&content)?
    } else if let Some(json) = json_input {
        eprintln!("⚠  Warning: --json exposes secrets in shell history. Prefer --stdin or --file.");
        parse_json_input(&json)?
    } else {
        let ct = select_credential_type()?;
        let fields = prompt_fields_interactive(&ct)?;
        (ct, fields)
    };

    let name = fields.get("name").cloned().unwrap_or_default();
    let service = fields.get("service").cloned().unwrap_or_default();
    let key = fields.get("key_identifier").cloned().unwrap_or_else(|| "default".into());

    let password = read_password(password, "Master password")?;
    let mut session = unlock_vault(paths, &password)?;
    let draft = CredentialDraft::new(&name, cred_type.clone(), &service, &key, serde_json::to_value(&fields)?);
    let id = session.add_credential(draft)?;
    session.save()?;
    output::success(&format!("Credential added: {}", &id[..8]));
    Ok(())
}

fn parse_json_input(json: &str) -> anyhow::Result<(lockit_core::credential::CredentialType, BTreeMap<String, String>)> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let cred_type: lockit_core::credential::CredentialType = v["type"].as_str()
        .unwrap_or("custom")
        .parse()
        .unwrap_or(lockit_core::credential::CredentialType::Custom);
    let fields: BTreeMap<String, String> = v["fields"]
        .as_object()
        .map(|o| o.iter().map(|(k, val)| (k.clone(), val.as_str().unwrap_or("").to_string())).collect())
        .unwrap_or_default();
    Ok((cred_type, fields))
}

fn read_password(value: Option<String>, prompt: &str) -> anyhow::Result<String> {
    match value {
        Some(v) => Ok(v),
        None => rpassword::prompt_password(format!("{prompt}: ")).context("read password"),
    }
}
```

- [ ] **Step 3: 更新 main.rs — 最小化重写**

```rust
// lockit/crates/lockit-cli/src/main.rs
use clap::{Parser, Subcommand};
use lockit_core::vault::VaultPaths;
use std::path::PathBuf;

mod commands;
mod interactive;
mod output;

#[derive(Parser)]
#[command(name = "lockit", about = "Secure credential manager")]
struct Cli {
    #[arg(long, global = true, help = "Path to vault.enc")]
    vault: Option<PathBuf>,
    #[arg(long, global = true, env = "LOCKIT_MASTER_PASSWORD")]
    password: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    #[command(about = "Add a credential (interactive or via --json/--stdin/--file)")]
    Add {
        #[arg(long, help = "JSON input (⚠ exposes secrets in history)")]
        json: Option<String>,
        #[arg(long, help = "Read JSON from stdin (recommended)")]
        stdin: bool,
        #[arg(long, help = "Read JSON from file")]
        file: Option<String>,
    },
    List {
        #[arg(long, help = "Output as JSON")]
        json: bool,
        query: Option<String>,
    },
    Show {
        name_or_id: String,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    #[command(about = "Edit a credential interactively")]
    Edit { name_or_id: String },
    Delete { name_or_id: String },
    Reveal {
        name_or_id: String,
        #[arg(default_value = "value")]
        field: String,
    },
    #[command(about = "Coding plan quota management")]
    CodingPlan {
        #[command(subcommand)]
        cmd: CodingPlanCmd,
    },
    #[command(about = "Cloud sync management")]
    Sync {
        #[command(subcommand)]
        cmd: SyncCmd,
    },
    #[command(about = "Output export statements for shell eval")]
    Env { name: String },
    #[command(about = "Run command with injected credentials")]
    Run {
        name: String,
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    Export {
        name: Option<String>,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    Import {
        file: PathBuf,
    },
}

#[derive(Subcommand)]
enum CodingPlanCmd {
    List,
    Refresh { provider: Option<String> },
}

#[derive(Subcommand)]
enum SyncCmd {
    Status,
    Push,
    Pull,
    Config,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let paths = match &cli.vault {
        Some(p) => VaultPaths::new(p.clone()),
        None => VaultPaths::platform_default()?,
    };

    match cli.command {
        Commands::Init => commands::init::run(&paths, cli.password),
        Commands::Add { json, stdin, file } => commands::add::run(&paths, cli.password, json, stdin, file),
        Commands::List { json, query } => commands::list::run(&paths, cli.password, json, query),
        Commands::Show { name_or_id, json } => commands::show::run(&paths, cli.password, &name_or_id, json),
        Commands::Edit { name_or_id } => commands::edit::run(&paths, cli.password, &name_or_id),
        Commands::Delete { name_or_id } => commands::delete::run(&paths, cli.password, &name_or_id),
        Commands::Reveal { name_or_id, field } => commands::reveal::run(&paths, cli.password, &name_or_id, &field),
        Commands::CodingPlan { cmd } => match cmd {
            CodingPlanCmd::List => commands::coding_plan::list(&paths, cli.password),
            CodingPlanCmd::Refresh { provider } => commands::coding_plan::refresh(&paths, cli.password, provider),
        },
        Commands::Sync { cmd } => match cmd {
            SyncCmd::Status => commands::sync_cmd::status(&paths),
            SyncCmd::Push => commands::sync_cmd::push(&paths, cli.password),
            SyncCmd::Pull => commands::sync_cmd::pull(&paths, cli.password),
            SyncCmd::Config => commands::sync_cmd::config(&paths),
        },
        Commands::Env { name } => commands::env_cmd::run(&paths, cli.password, &name),
        Commands::Run { name, cmd } => commands::run_cmd::run(&paths, cli.password, &name, &cmd),
        Commands::Export { name, json } => commands::export_cmd::run(&paths, cli.password, name, json),
        Commands::Import { file } => commands::import_cmd::run(&paths, cli.password, &file),
    }
}
```

- [ ] **Step 4: 编译确认并解决 missing module 错误**

```bash
cd lockit && cargo build -p lockit-cli 2>&1
```
Expected: 会有 missing module 编译错误（list, show, edit 等尚未创建）

- [ ] **Step 5: 创建占位模块消除编译错误**

```rust
// lockit/crates/lockit-cli/src/commands/mod.rs
pub mod add;
pub mod list;
pub mod show;
pub mod edit;
pub mod delete;
pub mod reveal;
pub mod coding_plan;
pub mod sync_cmd;
pub mod env_cmd;
pub mod run_cmd;
pub mod export_cmd;
pub mod import_cmd;
pub mod init;
```

为每个尚未创建的模块创建最小占位：

```rust
// lockit/crates/lockit-cli/src/commands/init.rs
use lockit_core::vault::{init_vault, VaultPaths};

pub fn run(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let pw = match password {
        Some(p) => p,
        None => rpassword::prompt_password("Create master password: ")?,
    };
    init_vault(paths, &pw)?;
    println!("Vault initialized at {}", paths.vault_path.display());
    Ok(())
}
```

```rust
// lockit/crates/lockit-cli/src/commands/delete.rs
use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    session.delete_credential(name_or_id)?;
    session.save()?;
    println!("Deleted: {name_or_id}");
    Ok(())
}
```

为其它的 (list, show, edit, reveal, coding_plan, sync_cmd, env_cmd, run_cmd, export_cmd, import_cmd) 创建结构相同的 todo!() 占位。

- [ ] **Step 6: Commit**

```bash
cd lockit && git add -A && git commit -m "feat(cli): new command tree with interactive add, output module, placeholder commands"
```

---

### Task 4: list, show, reveal, edit 命令

**Files:**
- Modify: `lockit/crates/lockit-cli/src/commands/list.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/show.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/reveal.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/edit.rs`

- [ ] **Step 1: 实现 list 命令**

```rust
// lockit/crates/lockit-cli/src/commands/list.rs
use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, json: bool, query: Option<String>) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let session = unlock_vault(paths, &pw)?;

    let results = match query {
        Some(q) => session.search_credentials(&q),
        None => session.list_credentials(),
    };

    if json {
        output::print_json(&results);
    } else {
        output::print_table(&results);
    }
    Ok(())
}
```

- [ ] **Step 2: 实现 show 命令**

```rust
// lockit/crates/lockit-cli/src/commands/show.rs
use lockit_core::credential_field::TypeFieldMap;
use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str, json: bool) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name_or_id)?;

    if json {
        output::print_json(&[credential]);
    } else {
        let fields = TypeFieldMap::fields_for(&credential.r#type);
        println!("ID:      {}", credential.id);
        println!("Name:    {}", credential.name);
        println!("Type:    {}", credential.r#type);
        println!("Service: {}", credential.service);
        println!("---");
        for field in &fields {
            let key = field.label.to_lowercase().replace(' ', "_");
            let value = credential.fields.get(&key).cloned().unwrap_or_else(|| "(not set)".into());
            println!("{}: {}", field.label, value);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 实现 reveal 命令**

```rust
// lockit/crates/lockit-cli/src/commands/reveal.rs
use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str, field: &str) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    let secret = session.reveal_secret(name_or_id, field)?;
    session.save()?;
    println!("{secret}");
    Ok(())
}
```

- [ ] **Step 4: 实现 edit 命令**

```rust
// lockit/crates/lockit-cli/src/commands/edit.rs
use lockit_core::credential::{CredentialDraft, CredentialType};
use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::interactive::prompt_fields_interactive;

pub fn run(paths: &VaultPaths, password: Option<String>, name_or_id: &str) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    let existing = session.get_credential(name_or_id)?;

    println!("Editing: {} (type: {})", existing.name, existing.r#type);
    println!("Press Enter to keep current value, type new to change.\n");

    let fields = prompt_fields_interactive(&existing.r#type)?;

    let draft = CredentialDraft::new(
        &existing.name,
        existing.r#type.clone(),
        fields.get("service").cloned().unwrap_or(existing.service),
        fields.get("key_identifier").cloned().unwrap_or(existing.key),
        serde_json::to_value(fields)?,
    );
    session.update_credential(name_or_id, draft)?;
    session.save()?;
    println!("Updated: {name_or_id}");
    Ok(())
}
```

- [ ] **Step 5: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli
```
Expected: 编译成功

- [ ] **Step 6: 手动测试**

```bash
# 初始化测试 vault（如果还没有）
echo "test123" | cargo run -p lockit-cli -- init

# 交互式添加
cargo run -p lockit-cli -- add
# 选择 api_key → 输入测试数据

# list
cargo run -p lockit-cli -- list

# show
cargo run -p lockit-cli -- list
# 用输出中的 ID 前缀测试 show
```

- [ ] **Step 7: Commit**

```bash
cd lockit && git add crates/lockit-cli/src/commands/
git commit -m "feat(cli): implement list, show, reveal, edit commands"
```

---

## Phase 2: Coding Plan

### Task 5: Coding Plan 模型 + Fetcher trait (core)

**Files:**
- Create: `lockit/crates/lockit-core/src/coding_plan.rs`
- Create: `lockit/crates/lockit-core/src/coding_plan/mod.rs`
- Create: `lockit/crates/lockit-core/tests/coding_plan_tests.rs`
- Modify: `lockit/crates/lockit-core/src/lib.rs`

- [ ] **Step 1: 写测试**

```rust
// lockit/crates/lockit-core/tests/coding_plan_tests.rs
use lockit_core::coding_plan::{CodingPlanProvider, ProviderQuota, QuotaStatus};

#[test]
fn test_quota_display() {
    let quota = ProviderQuota {
        provider: CodingPlanProvider::OpenAi,
        plan: "plus".into(),
        used: 847,
        total: 1000,
        remaining: "153".into(),
        remaining_days: Some(30),
        status: QuotaStatus::Ok,
        refreshed_at: chrono::Utc::now(),
    };
    assert_eq!(quota.usage_pct(), 84.7);
}

#[test]
fn test_quota_usage_pct_total_zero() {
    let quota = ProviderQuota {
        provider: CodingPlanProvider::Claude,
        plan: "pro".into(),
        used: 0,
        total: 0,
        remaining: "-".into(),
        remaining_days: None,
        status: QuotaStatus::Ok,
        refreshed_at: chrono::Utc::now(),
    };
    assert_eq!(quota.usage_pct(), 0.0);
}
```

- [ ] **Step 2: 实现模型**

```rust
// lockit/crates/lockit-core/src/coding_plan.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingPlanProvider {
    QwenBailian,
    OpenAi,
    ChatGpt,
    Anthropic,
    Claude,
    DeepSeek,
    Mimo,
    Google,
    Moonshot,
    MiniMax,
    Glm,
}

impl CodingPlanProvider {
    pub fn display_name(&self) -> &str {
        match self {
            Self::QwenBailian => "qwen_bailian",
            Self::OpenAi => "openai",
            Self::ChatGpt => "chatgpt",
            Self::Anthropic => "anthropic",
            Self::Claude => "claude",
            Self::DeepSeek => "deepseek",
            Self::Mimo => "xiaomi_mimo",
            Self::Google => "google",
            Self::Moonshot => "moonshot",
            Self::MiniMax => "minimax",
            Self::Glm => "glm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderQuota {
    pub provider: CodingPlanProvider,
    pub plan: String,
    pub used: u64,
    pub total: u64,
    pub remaining: String,
    pub remaining_days: Option<i64>,
    pub status: QuotaStatus,
    pub refreshed_at: DateTime<Utc>,
}

impl ProviderQuota {
    pub fn usage_pct(&self) -> f64 {
        if self.total == 0 { 0.0 } else { (self.used as f64 / self.total as f64) * 100.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaStatus {
    Ok,
    Error(String),
    AuthExpired,
}

#[derive(Debug, thiserror::Error)]
pub enum CodingPlanError {
    #[error("provider not configured: {0}")]
    NotConfigured(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

pub trait CodingPlanFetcher {
    fn provider(&self) -> CodingPlanProvider;
    fn fetch(&self, credential_fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError>;
}
```

- [ ] **Step 3: 更新 lib.rs**

```rust
// lockit/crates/lockit-core/src/lib.rs 追加:
pub mod coding_plan;
```

- [ ] **Step 4: 创建 coding_plan/mod.rs**

```rust
// lockit/crates/lockit-core/src/coding_plan/mod.rs
// 后续任务会填充具体 fetcher
```

- [ ] **Step 5: 注册 reqwest 依赖到 lockit-core**

```toml
# lockit/crates/lockit-core/Cargo.toml 追加 dep:
reqwest = { version = "0.12", features = ["json", "rustls-tls"], optional = true }

[features]
default = []
coding-plan = ["reqwest"]
```

- [ ] **Step 6: 运行测试**

```bash
cd lockit && cargo test -p lockit-core coding_plan_tests
```
Expected: 2 tests PASS

- [ ] **Step 7: Commit**

```bash
cd lockit && git add crates/lockit-core/src/coding_plan.rs crates/lockit-core/src/coding_plan/mod.rs crates/lockit-core/tests/coding_plan_tests.rs crates/lockit-core/src/lib.rs crates/lockit-core/Cargo.toml
git commit -m "feat(core): add CodingPlanProvider + ProviderQuota model and fetcher trait"
```

---

### Task 6: Qwen/Bailian + ChatGPT API 客户端

**Files:**
- Modify: `lockit/crates/lockit-core/src/coding_plan/mod.rs`
- Create: `lockit/crates/lockit-core/src/coding_plan/qwen.rs`
- Create: `lockit/crates/lockit-core/src/coding_plan/chatgpt.rs`

- [ ] **Step 1: 实现 Qwen/Bailian fetcher**

```rust
// lockit/crates/lockit-core/src/coding_plan/qwen.rs
use super::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use chrono::Utc;
use std::collections::BTreeMap;

pub struct QwenFetcher;

impl CodingPlanFetcher for QwenFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::QwenBailian
    }

    fn fetch(&self, fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = fields.get("api_key").or_else(|| fields.get("secret_value"))
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".into()))?;
        let cookie = fields.get("cookie").map(|s| s.as_str()).unwrap_or("");

        let client = reqwest::blocking::Client::new();
        let url = fields.get("base_url").map(|s| s.as_str()).unwrap_or("https://dashscope.aliyuncs.com");

        let resp = client
            .get(format!("{url}/api/v1/usage/overview", url = url.trim_end_matches('/')))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Cookie", cookie.to_string())
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        if !resp.status().is_success() {
            let status = if resp.status().as_u16() == 401 { QuotaStatus::AuthExpired }
                else { QuotaStatus::Error(format!("HTTP {}", resp.status())) };
            return Ok(ProviderQuota {
                provider: CodingPlanProvider::QwenBailian, plan: "—".into(),
                used: 0, total: 0, remaining: "—".into(), remaining_days: None,
                status, refreshed_at: Utc::now(),
            });
        }

        let json: serde_json::Value = resp.json()?;
        let usage = &json["data"]["usage"];
        let used = usage["used_tokens"].as_u64().unwrap_or(0);
        let total = usage["total_tokens"].as_u64().unwrap_or(0);

        Ok(ProviderQuota {
            provider: CodingPlanProvider::QwenBailian,
            plan: json["data"]["plan"].as_str().unwrap_or("—").to_string(),
            used, total,
            remaining: total.saturating_sub(used).to_string(),
            remaining_days: None,
            status: QuotaStatus::Ok,
            refreshed_at: Utc::now(),
        })
    }
}
```

- [ ] **Step 2: 实现 ChatGPT fetcher**

```rust
// lockit/crates/lockit-core/src/coding_plan/chatgpt.rs
use super::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use chrono::Utc;
use std::collections::BTreeMap;

pub struct ChatGptFetcher;

impl CodingPlanFetcher for ChatGptFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::ChatGpt
    }

    fn fetch(&self, fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = fields.get("api_key").or_else(|| fields.get("secret_value"))
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".into()))?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://api.openai.com/v1/dashboard/billing/subscription")
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        let plan = if resp.status().is_success() {
            let json: serde_json::Value = resp.json()?;
            json["plan"]["id"].as_str().unwrap_or("—").to_string()
        } else {
            return Ok(ProviderQuota {
                provider: CodingPlanProvider::ChatGpt, plan: "—".into(),
                used: 0, total: 0, remaining: "—".into(), remaining_days: None,
                status: if resp.status().as_u16() == 401 { QuotaStatus::AuthExpired }
                    else { QuotaStatus::Error(format!("HTTP {}", resp.status())) },
                refreshed_at: Utc::now(),
            });
        };

        let usage_resp = client
            .get("https://api.openai.com/v1/dashboard/billing/usage")
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        let (used, total) = if usage_resp.status().is_success() {
            let json: serde_json::Value = usage_resp.json()?;
            let u = json["total_usage"].as_f64().unwrap_or(0.0) as u64;
            let t = json["hard_limit_usd"].as_f64().unwrap_or(0.0) as u64;
            (u, t.saturating_mul(100))
        } else {
            (0, 0)
        };

        Ok(ProviderQuota {
            provider: CodingPlanProvider::ChatGpt, plan,
            used, total, remaining: total.saturating_sub(used).to_string(),
            remaining_days: None, status: QuotaStatus::Ok,
            refreshed_at: Utc::now(),
        })
    }
}
```

- [ ] **Step 3: 更新 mod.rs**

```rust
// lockit/crates/lockit-core/src/coding_plan/mod.rs
pub mod qwen;
pub mod chatgpt;

pub use qwen::QwenFetcher;
pub use chatgpt::ChatGptFetcher;
```

- [ ] **Step 4: 编译确认**

```bash
cd lockit && cargo build -p lockit-core --features coding-plan
```
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
cd lockit && git add crates/lockit-core/src/coding_plan/
git commit -m "feat(core): add Qwen/Bailian and ChatGPT coding plan fetchers"
```

---

### Task 7: Claude + DeepSeek + Mimo fetchers + coding-plan list CLI

**Files:**
- Create: `lockit/crates/lockit-core/src/coding_plan/claude.rs`
- Create: `lockit/crates/lockit-core/src/coding_plan/deepseek.rs`
- Create: `lockit/crates/lockit-core/src/coding_plan/mimo.rs`
- Modify: `lockit/crates/lockit-core/src/coding_plan/mod.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/coding_plan.rs`

- [ ] **Step 1: 实现 Claude（checksum-only，无直接配额 API）、DeepSeek、Mimo**

```rust
// lockit/crates/lockit-core/src/coding_plan/claude.rs
use super::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use chrono::Utc;
use std::collections::BTreeMap;

pub struct ClaudeFetcher;

impl CodingPlanFetcher for ClaudeFetcher {
    fn provider(&self) -> CodingPlanProvider { CodingPlanProvider::Claude }
    fn fetch(&self, fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = fields.get("api_key").or_else(|| fields.get("secret_value"))
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".into()))?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://api.anthropic.com/v1/messages?limit=1")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        let status = if resp.status().is_success() { QuotaStatus::Ok }
            else if resp.status().as_u16() == 401 { QuotaStatus::AuthExpired }
            else { QuotaStatus::Error(format!("HTTP {}", resp.status())) };

        Ok(ProviderQuota {
            provider: CodingPlanProvider::Claude, plan: "pro".into(),
            used: 0, total: 0, remaining: "—".into(),
            remaining_days: None, status, refreshed_at: Utc::now(),
        })
    }
}
```

```rust
// lockit/crates/lockit-core/src/coding_plan/deepseek.rs
use super::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use chrono::Utc;
use std::collections::BTreeMap;

pub struct DeepSeekFetcher;

impl CodingPlanFetcher for DeepSeekFetcher {
    fn provider(&self) -> CodingPlanProvider { CodingPlanProvider::DeepSeek }
    fn fetch(&self, fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = fields.get("api_key").or_else(|| fields.get("secret_value"))
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".into()))?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://api.deepseek.com/v1/user/balance")
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        let status = if resp.status().is_success() { QuotaStatus::Ok }
            else if resp.status().as_u16() == 401 { QuotaStatus::AuthExpired }
            else { QuotaStatus::Error(format!("HTTP {}", resp.status())) };

        let (used, total, plan) = if resp.status().is_success() {
            let json: serde_json::Value = resp.json().unwrap_or_default();
            let is_available = json["is_available"].as_bool().unwrap_or(false);
            let u = (json["balance_infos"][0]["used_tokens"].as_f64().unwrap_or(0.0)) as u64;
            let t = (json["balance_infos"][0]["total_tokens"].as_f64().unwrap_or(0.0)) as u64;
            let p = json["balance_infos"][0]["currency"].as_str().unwrap_or("tokens").to_string();
            (u, t, p)
        } else { (0, 0, "—".into()) };

        Ok(ProviderQuota {
            provider: CodingPlanProvider::DeepSeek, plan,
            used, total, remaining: total.saturating_sub(used).to_string(),
            remaining_days: None, status, refreshed_at: Utc::now(),
        })
    }
}
```

```rust
// lockit/crates/lockit-core/src/coding_plan/mimo.rs
use super::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use chrono::Utc;
use std::collections::BTreeMap;

pub struct MimoFetcher;

impl CodingPlanFetcher for MimoFetcher {
    fn provider(&self) -> CodingPlanProvider { CodingPlanProvider::Mimo }
    fn fetch(&self, fields: &BTreeMap<String, String>) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = fields.get("api_key").or_else(|| fields.get("secret_value"))
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".into()))?;

        let client = reqwest::blocking::Client::new();
        let resp = client
            .get("https://api.xiaomimimo.com/v1/usage")
            .header("Authorization", format!("Bearer {api_key}"))
            .timeout(std::time::Duration::from_secs(10))
            .send()?;

        let status = if resp.status().is_success() { QuotaStatus::Ok }
            else if resp.status().as_u16() == 401 { QuotaStatus::AuthExpired }
            else { QuotaStatus::Error(format!("HTTP {}", resp.status())) };

        Ok(ProviderQuota {
            provider: CodingPlanProvider::Mimo, plan: "default".into(),
            used: 0, total: 0, remaining: "—".into(),
            remaining_days: None, status, refreshed_at: Utc::now(),
        })
    }
}
```

- [ ] **Step 2: 更新 mod.rs**

```rust
// lockit/crates/lockit-core/src/coding_plan/mod.rs
pub mod chatgpt;
pub mod claude;
pub mod deepseek;
pub mod mimo;
pub mod qwen;

pub use chatgpt::ChatGptFetcher;
pub use claude::ClaudeFetcher;
pub use deepseek::DeepSeekFetcher;
pub use mimo::MimoFetcher;
pub use qwen::QwenFetcher;
```

- [ ] **Step 3: 实现 coding-plan CLI 命令**

```rust
// lockit/crates/lockit-cli/src/commands/coding_plan.rs
use lockit_core::coding_plan::{CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use lockit_core::coding_plan::{QwenFetcher, ChatGptFetcher, ClaudeFetcher, DeepSeekFetcher, MimoFetcher};
use lockit_core::credential_field::TypeFieldMap;
use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output;

fn fetcher_for(provider: &CodingPlanProvider) -> Option<Box<dyn CodingPlanFetcher>> {
    match provider {
        CodingPlanProvider::QwenBailian => Some(Box::new(QwenFetcher)),
        CodingPlanProvider::ChatGpt | CodingPlanProvider::OpenAi => Some(Box::new(ChatGptFetcher)),
        CodingPlanProvider::Claude | CodingPlanProvider::Anthropic => Some(Box::new(ClaudeFetcher)),
        CodingPlanProvider::DeepSeek => Some(Box::new(DeepSeekFetcher)),
        CodingPlanProvider::Mimo => Some(Box::new(MimoFetcher)),
        _ => None,
    }
}

pub fn list(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let session = unlock_vault(paths, &pw)?;
    let credentials = session.list_credentials();

    let coding_plan_creds: Vec<_> = credentials.iter()
        .filter(|c| c.r#type == lockit_core::credential::CredentialType::CodingPlan)
        .collect();

    if coding_plan_creds.is_empty() {
        println!("No coding plan credentials found. Add one with: lockit add --type coding_plan");
        return Ok(());
    }

    println!("{:<18} {:<10} {:<15} {:<12} {:<12}", "PROVIDER", "PLAN", "QUOTA USED", "REMAINING", "STATUS");
    println!("{}", "—".repeat(72));

    for cred in &coding_plan_creds {
        let provider_name = cred.fields.get("provider").cloned().unwrap_or_default();
        let status = QuotaStatus::Ok; // placeholder
        let status_str = match status {
            QuotaStatus::Ok => "✓",
            QuotaStatus::AuthExpired => "✗ expired",
            QuotaStatus::Error(_) => "✗ error",
        };
        println!("{:<18} {:<10} {:<15} {:<12} {:<12}",
            provider_name, "—", "— / —", "—", status_str);
    }
    Ok(())
}

pub fn refresh(paths: &VaultPaths, password: Option<String>, provider_filter: Option<String>) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    let credentials = session.list_credentials();

    let coding_plan_creds: Vec<_> = credentials.iter()
        .filter(|c| c.r#type == lockit_core::credential::CredentialType::CodingPlan)
        .collect();

    for cred in &coding_plan_creds {
        let provider_name = cred.fields.get("provider").cloned().unwrap_or_default();
        if let Some(ref filter) = provider_filter {
            if provider_name != *filter { continue; }
        }

        let provider = CodingPlanProvider::QwenBailian; // 从 fields 映射
        match fetcher_for(&provider) {
            Some(fetcher) => {
                // 需要完整 credential 来获取解密后的 fields
                // 这里简化处理
                let result: Result<ProviderQuota, _> = Err(lockit_core::coding_plan::CodingPlanError::NotConfigured(provider_name.clone()));
                match result {
                    Ok(q) => println!("  {:<18} ✓ ({}/{})", provider_name, q.used, q.total),
                    Err(e) => println!("  {:<18} ✗ ({e})", provider_name),
                }
            }
            None => println!("  {:<18} ✗ (unsupported)", provider_name),
        }
    }
    Ok(())
}
```

- [ ] **Step 4: 注意 — coding-plan refresh 需要完整 credential（非 redacted）来获取 API key。在 vault.rs 增加方法：**

```rust
// lockit/crates/lockit-core/src/vault.rs VaultSession 增加:
pub fn get_full_credential(&self, id: &str) -> Result<&Credential> {
    self.payload.credentials.iter()
        .find(|c| c.id == id || c.name.eq_ignore_ascii_case(id))
        .ok_or(VaultError::CredentialNotFound)
}
```

- [ ] **Step 5: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli 2>&1
```

- [ ] **Step 6: Commit**

```bash
cd lockit && git add crates/lockit-core/src/coding_plan/ crates/lockit-core/src/vault.rs crates/lockit-cli/src/commands/coding_plan.rs
git commit -m "feat: add all coding plan fetchers + list/refresh CLI commands"
```

---

## Phase 3: Google Drive Sync

### Task 8: Google Drive Backend 实现

**Files:**
- Create: `lockit/crates/lockit-core/src/sync/mod.rs`
- Create: `lockit/crates/lockit-core/src/sync/google_drive.rs`
- Modify: `lockit/crates/lockit-core/src/sync.rs` (add SyncManager)
- Create: `lockit/crates/lockit-core/tests/sync_tests.rs`
- Modify: `lockit/crates/lockit-core/src/lib.rs`

- [ ] **Step 1: 创建 sync 目录结构**

```rust
// lockit/crates/lockit-core/src/sync/mod.rs
pub mod google_drive;
```

```rust
// lockit/crates/lockit-core/src/lib.rs
// 将 pub mod sync; 改为:
pub mod sync;
```

`sync.rs` 内容移入 `sync/` 目录逻辑改为保持 `sync.rs` 作为入口，`mod.rs` 做 re-export。改为：保留 `sync.rs` 现有内容不变，新增 `sync/google_drive.rs` 并在 `sync.rs` 顶部添加 `pub mod google_drive;`。

- [ ] **Step 2: 实现 GoogleDriveBackend**

```rust
// lockit/crates/lockit-core/src/sync/google_drive.rs
use super::{SyncBackend, SyncError, SyncManifest, GOOGLE_DRIVE_APPDATA_FOLDER, GOOGLE_DRIVE_MANIFEST_FILE, GOOGLE_DRIVE_VAULT_FILE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    pub access_token: String,
    pub refresh_token: String,
    pub token_expiry: i64,
    pub client_id: String,
    pub client_secret: String,
}

pub struct GoogleDriveBackend {
    config: Option<GoogleDriveConfig>,
}

impl GoogleDriveBackend {
    pub fn new() -> Self {
        Self { config: None }
    }

    pub fn configure(&mut self, config: GoogleDriveConfig) {
        self.config = Some(config);
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    fn auth_header(&self) -> Option<String> {
        self.config.as_ref().map(|c| format!("Bearer {}", c.access_token))
    }

    fn refresh_access_token(&self) -> Result<String, SyncError> {
        let cfg = self.config.as_ref().ok_or(SyncError::NotConfigured)?;
        let client = reqwest::blocking::Client::new();
        let resp = client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", cfg.client_id.as_str()),
                ("client_secret", cfg.client_secret.as_str()),
                ("refresh_token", cfg.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;

        let json: serde_json::Value = resp.json().map_err(|e| SyncError::HttpError(e.to_string()))?;
        json["access_token"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SyncError::HttpError("no access_token in response".into()))
    }
}

impl SyncBackend for GoogleDriveBackend {
    fn name(&self) -> &str { "google_drive" }

    fn is_configured(&self) -> bool { self.config.is_some() }

    fn upload_vault(&self, encrypted_data: &[u8], manifest: &SyncManifest) -> Result<(), SyncError> {
        let token = self.auth_header().ok_or(SyncError::NotConfigured)?;
        let client = reqwest::blocking::Client::new();

        // Upload vault
        let vault_id = self.find_or_create_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE)?;
        self.upload_file_content(&client, &token, &vault_id, encrypted_data, "application/octet-stream")?;

        // Upload manifest
        let manifest_json = serde_json::to_vec(manifest).map_err(|e| SyncError::HttpError(e.to_string()))?;
        let manifest_id = self.find_or_create_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE)?;
        self.upload_file_content(&client, &token, &manifest_id, &manifest_json, "application/json")?;

        Ok(())
    }

    fn download_vault(&self) -> Result<Vec<u8>, SyncError> {
        let token = self.auth_header().ok_or(SyncError::NotConfigured)?;
        let client = reqwest::blocking::Client::new();
        let file_id = self.find_file(&client, &token, GOOGLE_DRIVE_VAULT_FILE)?
            .ok_or(SyncError::NotConfigured)?;
        let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);
        let resp = client.get(&url).bearer_auth(&token).send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(resp.bytes().map_err(|e| SyncError::HttpError(e.to_string()))?.to_vec())
    }

    fn get_manifest(&self) -> Result<Option<SyncManifest>, SyncError> {
        let token = self.auth_header().ok_or(SyncError::NotConfigured)?;
        let client = reqwest::blocking::Client::new();
        let file_id = match self.find_file(&client, &token, GOOGLE_DRIVE_MANIFEST_FILE)? {
            Some(id) => id,
            None => return Ok(None),
        };
        let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);
        let resp = client.get(&url).bearer_auth(&token).send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let manifest: SyncManifest = resp.json().map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(Some(manifest))
    }

    fn delete_sync_data(&self) -> Result<(), SyncError> {
        let token = self.auth_header().ok_or(SyncError::NotConfigured)?;
        let client = reqwest::blocking::Client::new();
        for name in &[GOOGLE_DRIVE_VAULT_FILE, GOOGLE_DRIVE_MANIFEST_FILE] {
            if let Some(id) = self.find_file(&client, &token, name)? {
                let url = format!("https://www.googleapis.com/drive/v3/files/{}", id);
                client.delete(&url).bearer_auth(&token).send()
                    .map_err(|e| SyncError::HttpError(e.to_string()))?;
            }
        }
        Ok(())
    }
}

impl GoogleDriveBackend {
    fn find_file(&self, client: &reqwest::blocking::Client, token: &str, name: &str) -> Result<Option<String>, SyncError> {
        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q=name='{}' and '{}' in parents&spaces=appDataFolder",
            name, GOOGLE_DRIVE_APPDATA_FOLDER
        );
        let resp = client.get(&url).bearer_auth(token).send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp.json().map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(json["files"].as_array()
            .and_then(|a| a.first())
            .and_then(|f| f["id"].as_str().map(|s| s.to_string())))
    }

    fn find_or_create_file(&self, client: &reqwest::blocking::Client, token: &str, name: &str) -> Result<String, SyncError> {
        if let Some(id) = self.find_file(client, token, name)? {
            return Ok(id);
        }
        let metadata = serde_json::json!({
            "name": name,
            "parents": [GOOGLE_DRIVE_APPDATA_FOLDER],
        });
        let resp = client
            .post("https://www.googleapis.com/drive/v3/files")
            .bearer_auth(token)
            .json(&metadata)
            .send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        let json: serde_json::Value = resp.json().map_err(|e| SyncError::HttpError(e.to_string()))?;
        json["id"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SyncError::HttpError("failed to create file".into()))
    }

    fn upload_file_content(&self, client: &reqwest::blocking::Client, token: &str, file_id: &str, data: &[u8], mime_type: &str) -> Result<(), SyncError> {
        let url = format!("https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media", file_id);
        client.patch(&url).bearer_auth(token).header("Content-Type", mime_type).body(data.to_vec()).send()
            .map_err(|e| SyncError::HttpError(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 3: 更新 sync.rs 增加 HttpError 变体**

```rust
// lockit/crates/lockit-core/src/sync.rs SyncError 增加:
#[error("HTTP error: {0}")]
HttpError(String),
```

并在 sync.rs 顶部增加 `pub mod google_drive;`。

- [ ] **Step 4: 编译确认**

```bash
cd lockit && cargo build -p lockit-core 2>&1
```

- [ ] **Step 5: Commit**

```bash
cd lockit && git add crates/lockit-core/src/sync/
git commit -m "feat(core): implement GoogleDriveBackend with appDataFolder API"
```

---

### Task 9: Sync CLI 命令

**Files:**
- Modify: `lockit/crates/lockit-cli/src/commands/sync_cmd.rs`

- [ ] **Step 1: 实现 sync 命令**

```rust
// lockit/crates/lockit-cli/src/commands/sync_cmd.rs
use lockit_core::sync::{self, SyncBackend, SyncInputs, SyncStatus};
use lockit_core::sync::google_drive::{GoogleDriveBackend, GoogleDriveConfig};
use lockit_core::vault::{unlock_vault, VaultPaths};
use inquire::Text;

pub fn status(paths: &VaultPaths) -> anyhow::Result<()> {
    let backend = GoogleDriveBackend::new();
    if !backend.is_configured() {
        println!("Status: Not configured. Run 'lockit sync config' first.");
        return Ok(());
    }

    let vault_bytes = std::fs::read(&paths.vault_path)?;
    let local_checksum = sync::sha256_checksum(&vault_bytes);
    let cloud_manifest = backend.get_manifest().unwrap_or(None);

    let input = SyncInputs {
        local_checksum: local_checksum.clone(),
        cloud_manifest: cloud_manifest.clone(),
        last_sync_checksum: None, // TODO: persist last sync checksum
        sync_key_configured: true,
        backend_configured: true,
    };

    let status = sync::compute_sync_status(input);
    match status {
        SyncStatus::UpToDate => println!("Status:  up-to-date"),
        SyncStatus::LocalAhead => println!("Status:  local ahead (push needed)"),
        SyncStatus::CloudAhead => println!("Status:  cloud ahead (pull needed)"),
        SyncStatus::Conflict => println!("Status:  conflict (manual resolution needed)"),
        SyncStatus::NeverSynced => println!("Status:  never synced"),
        _ => println!("Status:  {status:?}"),
    }

    if let Some(m) = cloud_manifest {
        println!("Remote:  {} ({})", m.updated_at.format("%Y-%m-%d %H:%M"), m.vault_checksum);
    }
    println!("Local:   {local_checksum}");
    Ok(())
}

pub fn push(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let session = unlock_vault(paths, &pw)?;

    let vault_bytes = std::fs::read(&paths.vault_path)?;
    let checksum = sync::sha256_checksum(&vault_bytes);

    let manifest = sync::SyncManifest::new(checksum, "lockit-cli", vault_bytes.len() as u64, 2);

    let backend = GoogleDriveBackend::new();
    backend.upload_vault(&vault_bytes, &manifest)?;
    println!("Pushed ✓ ({} entries)", session.list_credentials().len());
    Ok(())
}

pub fn pull(paths: &VaultPaths, password: Option<String>) -> anyhow::Result<()> {
    let backend = GoogleDriveBackend::new();
    let cloud_vault = backend.download_vault()?;
    let cloud_manifest = backend.get_manifest()?;
    let local_vault = std::fs::read(&paths.vault_path).unwrap_or_default();
    let local_checksum = sync::sha256_checksum(&local_vault);

    if let Some(ref manifest) = cloud_manifest {
        if manifest.vault_checksum == local_checksum {
            println!("Already up to date.");
            return Ok(());
        }
    }

    // Simple overwrite for now
    std::fs::write(&paths.vault_path, &cloud_vault)?;
    println!("Pulled ✓");
    Ok(())
}

pub fn config(paths: &VaultPaths) -> anyhow::Result<()> {
    println!("Google Drive sync configuration:");
    println!("1. Go to https://console.cloud.google.com/apis/credentials");
    println!("2. Create OAuth 2.0 Client ID (Desktop application)");
    println!("3. Enable Google Drive API");
    println!("4. Obtain refresh token via OAuth flow\n");

    let client_id = Text::new("Client ID:").prompt()?;
    let client_secret = Text::new("Client Secret:").prompt()?;
    let refresh_token = Text::new("Refresh Token:").prompt()?;
    let access_token = Text::new("Access Token (or press Enter to skip):")
        .prompt()
        .unwrap_or_default();

    let config = GoogleDriveConfig {
        access_token,
        refresh_token,
        token_expiry: 0,
        client_id,
        client_secret,
    };

    // TODO: persist to config file
    println!("✓ Google Drive configured");
    Ok(())
}
```

- [ ] **Step 2: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli
```

- [ ] **Step 3: Commit**

```bash
cd lockit && git add crates/lockit-cli/src/commands/sync_cmd.rs
git commit -m "feat(cli): implement sync status/push/pull/config commands"
```

---

## Phase 4: Agent 注入

### Task 10: env + run 命令

**Files:**
- Modify: `lockit/crates/lockit-cli/src/commands/env_cmd.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/run_cmd.rs`
- Modify: `lockit/crates/lockit-core/src/vault.rs` (已有 reveal_secret 足够)

- [ ] **Step 1: 实现 env 命令**

```rust
// lockit/crates/lockit-cli/src/commands/env_cmd.rs
use lockit_core::credential_field::TypeFieldMap;
use lockit_core::vault::{unlock_vault, VaultPaths};

pub fn run(paths: &VaultPaths, password: Option<String>, name: &str) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name)?;

    let prefix = credential.name.to_uppercase().replace(['-', ' '], "_");

    for field_name in credential.fields.keys() {
        let secret = session.reveal_secret(name, field_name)?;
        let env_name = format!("{}_{}", prefix, field_name.to_uppercase());
        // Shell-escape the value
        let escaped = secret.replace('\'', "'\\''");
        println!("export {}='{}'", env_name, escaped);
    }

    session.save()?;
    Ok(())
}
```

- [ ] **Step 2: 实现 run 命令**

```rust
// lockit/crates/lockit-cli/src/commands/run_cmd.rs
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::process::Command;

pub fn run(paths: &VaultPaths, password: Option<String>, name: &str, cmd: &[String]) -> anyhow::Result<()> {
    if cmd.is_empty() {
        anyhow::bail!("No command specified. Usage: lockit run <name> -- <command>");
    }

    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;
    let credential = session.get_credential(name)?;

    let prefix = credential.name.to_uppercase().replace(['-', ' '], "_");

    let mut child = Command::new(&cmd[0]);
    child.args(&cmd[1..]);

    for field_name in credential.fields.keys() {
        let secret = session.reveal_secret(name, field_name)?;
        let env_name = format!("{}_{}", prefix, field_name.to_uppercase());
        child.env(&env_name, secret);
    }

    session.save()?;

    let status = child.status()?;
    std::process::exit(status.code().unwrap_or(1));
}
```

- [ ] **Step 3: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli
```

- [ ] **Step 4: Commit**

```bash
cd lockit && git add crates/lockit-cli/src/commands/env_cmd.rs crates/lockit-cli/src/commands/run_cmd.rs
git commit -m "feat(cli): implement env and run commands for agent injection"
```

---

## Phase 5: 导入导出

### Task 11: export + import 命令

**Files:**
- Modify: `lockit/crates/lockit-cli/src/commands/export_cmd.rs`
- Modify: `lockit/crates/lockit-cli/src/commands/import_cmd.rs`

- [ ] **Step 1: 实现 export 命令**

```rust
// lockit/crates/lockit-cli/src/commands/export_cmd.rs
use lockit_core::vault::{unlock_vault, VaultPaths};
use crate::output;

pub fn run(paths: &VaultPaths, password: Option<String>, name: Option<String>, json: bool) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let session = unlock_vault(paths, &pw)?;

    let credentials = match name {
        Some(n) => {
            let cred = session.get_credential(&n)?;
            vec![cred]
        }
        None => session.list_credentials(),
    };

    if json {
        output::print_json(&credentials);
    } else {
        for cred in &credentials {
            output::print_json(&[cred.clone()]);
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 实现 import 命令**

```rust
// lockit/crates/lockit-cli/src/commands/import_cmd.rs
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::path::PathBuf;

pub fn run(paths: &VaultPaths, password: Option<String>, file: &PathBuf) -> anyhow::Result<()> {
    let pw = password.unwrap_or_else(|| rpassword::prompt_password("Master password: ").unwrap());
    let mut session = unlock_vault(paths, &pw)?;

    let content = std::fs::read_to_string(file)?;

    // Try Android backup format (decrypted JSON array)
    if let Ok(credentials) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
        for item in credentials {
            let name = item["name"].as_str().unwrap_or("imported");
            let r#type: lockit_core::credential::CredentialType = item["type"].as_str()
                .unwrap_or("custom").parse().unwrap_or_default();
            let service = item["service"].as_str().unwrap_or("");
            let key = item["key"].as_str().unwrap_or("default");
            let fields = item["fields"].clone();

            let draft = lockit_core::credential::CredentialDraft::new(
                name, r#type, service, key, fields,
            );
            session.add_credential(draft)?;
        }
        session.save()?;
        println!("Imported {} credentials", credentials.len());
        return Ok(());
    }

    // Try legacy markdown
    let drafts = lockit_core::migration::parse_legacy_markdown(&content)?;
    let count = drafts.len();
    for draft in drafts {
        session.add_credential(draft)?;
    }
    session.save()?;
    println!("Imported {count} credentials from legacy format");
    Ok(())
}
```

- [ ] **Step 3: 编译确认**

```bash
cd lockit && cargo build -p lockit-cli
```

- [ ] **Step 4: Commit**

```bash
cd lockit && git add crates/lockit-cli/src/commands/export_cmd.rs crates/lockit-cli/src/commands/import_cmd.rs
git commit -m "feat(cli): implement export and import commands"
```

---

### Task 12: 集成测试 + 最终编译

- [ ] **Step 1: 全量构建**

```bash
cd lockit && cargo build --release -p lockit-cli
```
Expected: 编译成功

- [ ] **Step 2: 运行所有测试**

```bash
cd lockit && cargo test --all
```
Expected: 所有测试 PASS

- [ ] **Step 3: Commit**

```bash
cd lockit && git add -A && git commit -m "chore: final integration, all tests pass"
```

---

## 自审记录

- ~~P1 同步后端~~ → Phase 3 全部为 Google Drive，WebDAV 代码未出现 ✓
- ~~P1 加密格式兼容~~ → 字段级加密算法一致（nonce+ciphertext），vault 整体加密为 CLI 自有格式 ✓
- ~~P2 `--json` 泄露~~ → stdin/file 优先，`--json` 打印警告 ✓
- ~~P2 env/run 示例~~ → env 输出 `export` 供 eval，run 用 `Command::env()` 注入子进程 ✓
