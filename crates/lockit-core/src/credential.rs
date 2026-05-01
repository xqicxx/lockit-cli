use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    ApiKey,
    GitHub,
    Account,
    CodingPlan,
    Password,
    Phone,
    BankCard,
    Email,
    Token,
    SshKey,
    WebhookSecret,
    OAuthClient,
    AwsCredential,
    GpgKey,
    DatabaseUrl,
    IdCard,
    Note,
    Custom,
}

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

    pub fn description(&self) -> &'static str {
        match self {
            Self::ApiKey => "Store API keys for AI services, cloud providers, or any REST API",
            Self::GitHub => "Store GitHub credentials for private repos, CI/CD, and agent git operations",
            Self::Account => "Store login credentials for websites or apps",
            Self::CodingPlan => "Store coding agent plan tokens",
            Self::Password => "Store standalone passwords (WiFi, shared, etc.)",
            Self::Phone => "Store phone numbers with country codes",
            Self::BankCard => "Store bank card details for payments",
            Self::Email => "Store email accounts with passwords and regions",
            Self::Token => "Store bearer tokens, session tokens, or auth tokens",
            Self::SshKey => "Store SSH private keys for server access",
            Self::WebhookSecret => "Store webhook signing secrets",
            Self::OAuthClient => "Store OAuth2 client ID and secret",
            Self::AwsCredential => "Store AWS access keys for cloud operations",
            Self::GpgKey => "Store GPG signing keys for commits or package signing",
            Self::DatabaseUrl => "Store database connection strings",
            Self::IdCard => "Store ID card information",
            Self::Note => "Store freeform notes like WiFi passwords, server info",
            Self::Custom => "Generic key-value store for any credential type",
        }
    }

    pub fn required_field_indices(&self) -> Vec<usize> {
        crate::credential_field::TypeFieldMap::fields_for(self)
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.required.then_some(i))
            .collect()
    }
}

impl fmt::Display for CredentialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
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
        };
        write!(f, "{value}")
    }
}

impl FromStr for CredentialType {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let normalized = input.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "api_key" | "apikey" => Ok(Self::ApiKey),
            "github" => Ok(Self::GitHub),
            "account" => Ok(Self::Account),
            "coding_plan" | "codingplan" => Ok(Self::CodingPlan),
            "password" | "passwd" | "pwd" => Ok(Self::Password),
            "phone" => Ok(Self::Phone),
            "bank_card" | "bankcard" | "card" => Ok(Self::BankCard),
            "email" | "email_account" => Ok(Self::Email),
            "token" | "bearer_token" => Ok(Self::Token),
            "ssh_key" | "sshkey" => Ok(Self::SshKey),
            "webhook_secret" | "webhook" => Ok(Self::WebhookSecret),
            "oauth_client" | "oauth" => Ok(Self::OAuthClient),
            "aws_credential" | "aws" => Ok(Self::AwsCredential),
            "gpg_key" | "gpg" => Ok(Self::GpgKey),
            "database_url" | "db_url" => Ok(Self::DatabaseUrl),
            "id_card" | "idcard" => Ok(Self::IdCard),
            "note" => Ok(Self::Note),
            "custom" => Ok(Self::Custom),
            other => Err(format!("unknown credential type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub r#type: CredentialType,
    pub service: String,
    pub key: String,
    pub fields: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RedactedCredential {
    pub id: String,
    pub name: String,
    pub r#type: CredentialType,
    pub service: String,
    pub key: String,
    pub fields: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialDraft {
    pub name: String,
    pub r#type: CredentialType,
    pub service: String,
    pub key: String,
    pub fields: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
    pub tags: Vec<String>,
}

impl CredentialDraft {
    pub fn new(
        name: impl Into<String>,
        r#type: CredentialType,
        service: impl Into<String>,
        key: impl Into<String>,
        fields: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            r#type,
            service: service.into(),
            key: key.into(),
            fields: value_to_string_map(fields),
            metadata: BTreeMap::new(),
            tags: Vec::new(),
        }
    }

    pub fn with_metadata(mut self, metadata: BTreeMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn into_credential(self) -> Credential {
        let now = Utc::now();
        Credential {
            id: Uuid::new_v4().to_string(),
            name: self.name,
            r#type: self.r#type,
            service: self.service,
            key: self.key,
            fields: self.fields,
            metadata: self.metadata,
            tags: self.tags,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Credential {
    pub fn redacted(&self) -> RedactedCredential {
        RedactedCredential {
            id: self.id.clone(),
            name: self.name.clone(),
            r#type: self.r#type.clone(),
            service: self.service.clone(),
            key: self.key.clone(),
            fields: self
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), redact_secret(value)))
                .collect(),
            metadata: self.metadata.clone(),
            tags: self.tags.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let query = normalize(query);
        if query.is_empty() {
            return true;
        }

        let haystack = [
            self.name.as_str(),
            self.service.as_str(),
            self.key.as_str(),
            &self.r#type.to_string(),
            &self.tags.join(" "),
            &self.fields.values().cloned().collect::<Vec<_>>().join(" "),
            &self.metadata.values().cloned().collect::<Vec<_>>().join(" "),
        ]
        .join(" ");
        normalize(&haystack).contains(&query)
    }
}

pub fn redact_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    match chars.len() {
        0 => String::new(),
        1..=4 => "•".repeat(chars.len()),
        5..=8 => format!("{}••{}", chars[0], chars[chars.len() - 1]),
        _ => {
            let prefix: String = chars.iter().take(4).collect();
            let suffix: String = chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            format!("{prefix}••••{suffix}")
        }
    }
}

pub fn value_to_string_map(value: serde_json::Value) -> BTreeMap<String, String> {
    match value {
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| {
                let value = match value {
                    serde_json::Value::String(value) => value,
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                };
                (key, value)
            })
            .collect(),
        serde_json::Value::String(value) => BTreeMap::from([("value".to_string(), value)]),
        other => BTreeMap::from([("value".to_string(), other.to_string())]),
    }
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
