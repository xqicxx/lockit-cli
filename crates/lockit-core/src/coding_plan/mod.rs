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
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64) * 100.0
        }
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
}

pub trait CodingPlanFetcher {
    fn provider(&self) -> CodingPlanProvider;
    fn fetch(
        &self,
        credential_fields: &BTreeMap<String, String>,
    ) -> Result<ProviderQuota, CodingPlanError>;
}
