use crate::coding_plan::{
    find_field, CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota,
    QuotaStatus,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Duration;

pub struct QwenFetcher;

impl CodingPlanFetcher for QwenFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::QwenBailian
    }

    fn fetch(
        &self,
        credential_fields: &BTreeMap<String, String>,
    ) -> Result<ProviderQuota, CodingPlanError> {
        let base_url = find_field(credential_fields, "base_url")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "https://dashscope.aliyuncs.com".to_string());
        let api_key = find_field(credential_fields, "api_key")
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".to_string()))?
            .to_string();
        let cookie = find_field(credential_fields, "cookie")
            .unwrap_or("")
            .to_string();

        let url = format!("{}/api/v1/usage/overview", base_url.trim_end_matches('/'));

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CodingPlanError::ApiError(format!("client build: {}", e)))?;

        let response = {
            let mut request = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json");

            if !cookie.is_empty() {
                request = request.header("Cookie", cookie.as_str());
            }

            match request.send() {
                Ok(r) => r,
                Err(e) => {
                    return Err(CodingPlanError::ApiError(format!(
                        "request failed: {}",
                        e
                    )));
                }
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

        if !status.is_success() {
            return Ok(ProviderQuota {
                provider: self.provider(),
                plan: String::new(),
                used: 0,
                total: 0,
                remaining: String::new(),
                remaining_days: None,
                status: QuotaStatus::Error(format!("HTTP {}", status.as_u16())),
                refreshed_at: Utc::now(),
            });
        }

        let body: serde_json::Value = response
            .json()
            .map_err(|e| CodingPlanError::ParseError(format!("json parse: {}", e)))?;

        let used = body
            .get("data")
            .and_then(|d: &serde_json::Value| d.get("used_tokens"))
            .or_else(|| body.get("used"))
            .or_else(|| body.get("used_tokens"))
            .and_then(|v: &serde_json::Value| v.as_u64())
            .unwrap_or(0);

        let total = body
            .get("data")
            .and_then(|d: &serde_json::Value| d.get("total_tokens"))
            .or_else(|| body.get("total"))
            .or_else(|| body.get("total_tokens"))
            .or_else(|| body.get("quota"))
            .and_then(|v: &serde_json::Value| v.as_u64())
            .unwrap_or(0);

        let remaining = if total > 0 && total > used {
            (total - used).to_string()
        } else if total == 0 {
            String::from("—")
        } else {
            String::from("0")
        };

        Ok(ProviderQuota {
            provider: self.provider(),
            plan: String::from("Qwen"),
            used,
            total,
            remaining,
            remaining_days: None,
            status: QuotaStatus::Ok,
            refreshed_at: Utc::now(),
        })
    }
}
