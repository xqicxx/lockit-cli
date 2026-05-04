use crate::coding_plan::{
    CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus,
};
use crate::credential::find_field_insensitive;
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Duration;

pub struct ClaudeFetcher;

impl CodingPlanFetcher for ClaudeFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::Claude
    }

    fn fetch(
        &self,
        credential_fields: &BTreeMap<String, String>,
    ) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = find_field_insensitive(credential_fields, "api_key")
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".to_string()))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CodingPlanError::ApiError(format!("client build: {}", e)))?;

        let response = match client
            .get("https://api.anthropic.com/v1/messages?limit=1")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                return Err(CodingPlanError::ApiError(format!("request failed: {}", e)));
            }
        };

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status.as_u16() == 403 {
            return Ok(ProviderQuota {
                provider: self.provider(),
                plan: String::new(),
                used: 0,
                total: 0,
                remaining: String::new(),
                remaining_days: None,
                status: QuotaStatus::AuthExpired,
                refreshed_at: Utc::now(),
            });
        }

        // Checksum-only: just verify the key is valid. No quota info available
        // from the public Anthropic API.
        if status.is_success() || status.as_u16() >= 400 {
            Ok(ProviderQuota {
                provider: self.provider(),
                plan: String::from("Claude API"),
                used: 0,
                total: 0,
                remaining: String::from("—"),
                remaining_days: None,
                status: QuotaStatus::Ok,
                refreshed_at: Utc::now(),
            })
        } else {
            Ok(ProviderQuota {
                provider: self.provider(),
                plan: String::new(),
                used: 0,
                total: 0,
                remaining: String::new(),
                remaining_days: None,
                status: QuotaStatus::Error(format!("HTTP {}", status.as_u16())),
                refreshed_at: Utc::now(),
            })
        }
    }
}
