use crate::credential::CredentialType;

static SERVICE_PRESETS: &[&str] = &[
    "google",
    "github",
    "openai",
    "anthropic",
    "aws",
    "vercel",
    "stripe",
    "netlify",
    "cloudflare",
    "alibaba",
    "tencent",
];

static CODING_PLAN_PROVIDERS: &[&str] = &[
    "openai",
    "chatgpt",
    "anthropic",
    "claude",
    "google",
    "deepseek",
    "moonshot",
    "minimax",
    "glm",
    "qwen",
    "qwen_bailian",
    "xiaomi_mimo",
];

static CODING_PLAN_BASE_URLS: &[&str] = &[
    "https://api.openai.com",
    "https://api.anthropic.com",
    "https://api.deepseek.com",
    "https://dashscope.aliyuncs.com",
];

static GITHUB_TOKEN_TYPES: &[&str] = &["PAT", "SSH", "OAuth", "GitHub App"];
static GITHUB_SCOPES: &[&str] = &["repo", "read:org", "workflow"];
static REGION_PRESETS: &[&str] = &["CN", "US", "JP", "KR", "SG"];
static BANK_PRESETS: &[&str] = &["ICBC", "BOC", "CMB", "CCB", "ABC"];
static EMAIL_SERVICE_PRESETS: &[&str] = &["gmail", "outlook", "qq", "163", "protonmail"];
static KEY_TYPE_PRESETS: &[&str] = &["ed25519", "rsa-4096"];
static WEBHOOK_SERVICE_PRESETS: &[&str] = &["github", "stripe", "vercel"];
static DB_PRESETS: &[&str] = &["postgres", "mysql", "mongo", "redis"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialField {
    pub label: &'static str,
    pub placeholder: &'static str,
    pub required: bool,
    pub secret: bool,
    pub presets: &'static [&'static str],
}

impl CredentialField {
    pub fn is_dropdown(&self) -> bool {
        !self.presets.is_empty()
    }
}

pub fn credential_fields_for(ct: &CredentialType) -> Vec<CredentialField> {
    match ct {
        CredentialType::ApiKey => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. OPENAI_API_KEY",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. openai, anthropic...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "KEY_IDENTIFIER",
                placeholder: "e.g. default, production...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SECRET_VALUE",
                placeholder: "Paste or enter the secret...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::GitHub => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. GITHUB_TOKEN",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "TOKEN_TYPE",
                placeholder: "Select token type",
                required: false,
                secret: false,
                presets: GITHUB_TOKEN_TYPES,
            },
            CredentialField {
                label: "ACCOUNT",
                placeholder: "GitHub username",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "TOKEN_VALUE",
                placeholder: "Paste token or SSH key...",
                required: true,
                secret: true,
                presets: &[],
            },
            CredentialField {
                label: "SCOPE",
                placeholder: "Select scopes",
                required: false,
                secret: false,
                presets: GITHUB_SCOPES,
            },
        ],
        CredentialType::Account => vec![
            CredentialField {
                label: "USERNAME",
                placeholder: "Enter username...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. google, github...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "EMAIL",
                placeholder: "Associated email",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "PASSWORD",
                placeholder: "Enter password...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::CodingPlan => vec![
            CredentialField {
                label: "PROVIDER",
                placeholder: "Select provider",
                required: false,
                secret: false,
                presets: CODING_PLAN_PROVIDERS,
            },
            CredentialField {
                label: "RAW_CURL",
                placeholder: "Paste curl command (auto-extracts)...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "API_KEY",
                placeholder: "Paste your API key here...",
                required: true,
                secret: true,
                presets: &[],
            },
            CredentialField {
                label: "COOKIE",
                placeholder: "Bailian console cookie...",
                required: false,
                secret: true,
                presets: &[],
            },
            CredentialField {
                label: "BASE_URL",
                placeholder: "Select base URL",
                required: true,
                secret: false,
                presets: CODING_PLAN_BASE_URLS,
            },
        ],
        CredentialType::Password => vec![
            CredentialField {
                label: "PASSWORD_LABEL",
                placeholder: "Enter password...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. google, github...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "USERNAME",
                placeholder: "Associated username",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "PASSWORD_VALUE",
                placeholder: "Enter password again...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::Phone => vec![
            CredentialField {
                label: "REGION",
                placeholder: "Select region",
                required: false,
                secret: false,
                presets: REGION_PRESETS,
            },
            CredentialField {
                label: "PHONE_NUMBER",
                placeholder: "138 0000 0000",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "NOTE",
                placeholder: "e.g. delivery, work contact...",
                required: false,
                secret: false,
                presets: &[],
            },
        ],
        CredentialType::BankCard => vec![
            CredentialField {
                label: "CARD_NUMBER",
                placeholder: "Card number...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "BANK",
                placeholder: "e.g. ICBC, BOC...",
                required: false,
                secret: false,
                presets: BANK_PRESETS,
            },
            CredentialField {
                label: "CARDHOLDER",
                placeholder: "Cardholder name...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "CVV_EXPIRY",
                placeholder: "CVV or expiry",
                required: false,
                secret: false,
                presets: &[],
            },
        ],
        CredentialType::Email => vec![
            CredentialField {
                label: "SERVICE",
                placeholder: "Select provider",
                required: true,
                secret: false,
                presets: EMAIL_SERVICE_PRESETS,
            },
            CredentialField {
                label: "EMAIL_PREFIX",
                placeholder: "e.g. john.doe",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "PASSWORD",
                placeholder: "Password or app code...",
                required: true,
                secret: true,
                presets: &[],
            },
            CredentialField {
                label: "REGION",
                placeholder: "Select region...",
                required: false,
                secret: false,
                presets: REGION_PRESETS,
            },
            CredentialField {
                label: "STREET",
                placeholder: "123 Main St",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "CITY",
                placeholder: "New York",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "STATE_ZIP",
                placeholder: "NY 10001",
                required: false,
                secret: false,
                presets: &[],
            },
        ],
        CredentialType::Token => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. JWT_TOKEN",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. my-app...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "KEY_IDENTIFIER",
                placeholder: "e.g. default, production...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "TOKEN_VALUE",
                placeholder: "Paste token...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::SshKey => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. GITHUB_SSH",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. github, aws...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "KEY_IDENTIFIER",
                placeholder: "e.g. ed25519, rsa-4096...",
                required: false,
                secret: false,
                presets: KEY_TYPE_PRESETS,
            },
            CredentialField {
                label: "PRIVATE_KEY",
                placeholder: "Paste private key...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::WebhookSecret => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. GITHUB_WEBHOOK",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. github, stripe...",
                required: false,
                secret: false,
                presets: WEBHOOK_SERVICE_PRESETS,
            },
            CredentialField {
                label: "HEADER_KEY",
                placeholder: "e.g. X-Hub-Signature...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SECRET_VALUE",
                placeholder: "Paste webhook secret...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::OAuthClient => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. GOOGLE_OAUTH",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. google, github...",
                required: false,
                secret: false,
                presets: SERVICE_PRESETS,
            },
            CredentialField {
                label: "CLIENT_ID",
                placeholder: "Enter client ID...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "CLIENT_SECRET",
                placeholder: "Paste client secret...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::AwsCredential => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. AWS_PROD",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. aws, aws-prod...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "ACCESS_KEY",
                placeholder: "Enter access key ID...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SECRET_KEY",
                placeholder: "Paste secret key...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::GpgKey => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. PERSONAL_GPG",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. personal, ci-cd...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "KEY_ID",
                placeholder: "e.g. key fingerprint...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "PRIVATE_KEY",
                placeholder: "Paste GPG private key...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::DatabaseUrl => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. POSTGRES_PROD",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. postgres, mongo...",
                required: false,
                secret: false,
                presets: DB_PRESETS,
            },
            CredentialField {
                label: "KEY_IDENTIFIER",
                placeholder: "e.g. primary, replica...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "CONNECTION_URL",
                placeholder: "Paste connection string...",
                required: true,
                secret: true,
                presets: &[],
            },
        ],
        CredentialType::IdCard => vec![
            CredentialField {
                label: "CARDHOLDER",
                placeholder: "Name on ID...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "ISSUER",
                placeholder: "e.g. government, company...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "ID_NUMBER",
                placeholder: "ID number...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "EXTRA",
                placeholder: "Notes",
                required: false,
                secret: false,
                presets: &[],
            },
        ],
        CredentialType::Note => vec![
            CredentialField {
                label: "TITLE",
                placeholder: "e.g. WiFi Password, Server Info...",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "TAGS",
                placeholder: "e.g. wifi, network, home...",
                required: false,
                secret: false,
                presets: &[],
            },
        ],
        CredentialType::Custom => vec![
            CredentialField {
                label: "NAME",
                placeholder: "e.g. MY_CUSTOM_KEY",
                required: true,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "SERVICE",
                placeholder: "e.g. my-service...",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "KEY",
                placeholder: "custom_key_identifier",
                required: false,
                secret: false,
                presets: &[],
            },
            CredentialField {
                label: "VALUE",
                placeholder: "Paste or enter the secret...",
                required: true,
                secret: false,
                presets: &[],
            },
        ],
    }
}
