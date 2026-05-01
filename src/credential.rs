use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CredentialType {
    #[serde(rename = "api_key")]
    ApiKey,
    #[serde(rename = "cookie")]
    Cookie,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "password")]
    Password,
    #[serde(rename = "custom")]
    Custom,
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialType::ApiKey => write!(f, "api_key"),
            CredentialType::Cookie => write!(f, "cookie"),
            CredentialType::Token => write!(f, "token"),
            CredentialType::Password => write!(f, "password"),
            CredentialType::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for CredentialType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "api_key" | "apikey" | "api-key" => Ok(CredentialType::ApiKey),
            "cookie" => Ok(CredentialType::Cookie),
            "token" => Ok(CredentialType::Token),
            "password" | "passwd" | "pwd" => Ok(CredentialType::Password),
            "custom" => Ok(CredentialType::Custom),
            _ => Err(format!("Unknown credential type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,
    pub r#type: CredentialType,
    pub service: String,
    pub key: String,
    pub value: String,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Credential {
    pub fn new(name: String, r#type: CredentialType, service: String, key: String, value: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            r#type,
            service,
            key,
            value,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }
}
