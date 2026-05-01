use crate::credential::CredentialType;

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
                CredentialFieldDef { label: "PROVIDER".into(), placeholder: "Select provider".into(), required: false, is_dropdown: true, presets: vec!["openai".into(), "chatgpt".into(), "anthropic".into(), "claude".into(), "google".into(), "deepseek".into(), "moonshot".into(), "minimax".into(), "glm".into(), "qwen".into(), "qwen_bailian".into(), "xiaomi_mimo".into()] },
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
