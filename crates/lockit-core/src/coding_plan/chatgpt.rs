use crate::coding_plan::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use crate::credential::find_field_insensitive;
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Duration;

pub struct ChatGptFetcher;

impl CodingPlanFetcher for ChatGptFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::ChatGpt
    }

    fn fetch(
        &self,
        credential_fields: &BTreeMap<String, String>,
    ) -> Result<ProviderQuota, CodingPlanError> {
        let base_url = find_field_insensitive(credential_fields, "base_url")
            .unwrap_or("https://api.openai.com");
        let api_key = find_field_insensitive(credential_fields, "api_key")
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".to_string()))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CodingPlanError::ApiError(format!("client build: {}", e)))?;

        let base = base_url.trim_end_matches('/');

        // Fetch subscription for plan name (10s timeout)
        let sub_url = format!("{}/v1/dashboard/billing/subscription", base);
        let plan_name: String = match client
            .get(&sub_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
        {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                    || resp.status().as_u16() == 403
                {
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
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>() {
                        Ok(json) => json
                            .get("plan")
                            .and_then(|p: &serde_json::Value| p.get("title"))
                            .or_else(|| {
                                json.get("account_plan")
                                    .and_then(|p: &serde_json::Value| p.get("title"))
                            })
                            .or_else(|| json.get("plan_name"))
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .unwrap_or("—")
                            .to_string(),
                        Err(_) => String::from("—"),
                    }
                } else {
                    String::from("—")
                }
            }
            Err(_) => String::from("—"),
        };

        // Fetch usage (10s timeout)
        let usage_url = format!("{}/v1/dashboard/billing/usage", base);
        let (used, total): (u64, u64) = match client
            .get(&usage_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
        {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                    || resp.status().as_u16() == 403
                {
                    return Ok(ProviderQuota {
                        provider: self.provider(),
                        plan: plan_name,
                        used: 0,
                        total: 0,
                        remaining: String::new(),
                        remaining_days: None,
                        status: QuotaStatus::AuthExpired,
                        refreshed_at: Utc::now(),
                    });
                }
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>() {
                        Ok(json) => {
                            let u = json
                                .get("total_usage")
                                .or_else(|| json.get("used"))
                                .and_then(|v: &serde_json::Value| v.as_f64())
                                .unwrap_or(0.0) as u64;
                            let t = json
                                .get("hard_limit_usd")
                                .or_else(|| json.get("limit"))
                                .or_else(|| json.get("total"))
                                .and_then(|v: &serde_json::Value| v.as_f64())
                                .unwrap_or(0.0) as u64;
                            (u, t)
                        }
                        Err(_) => (0, 0),
                    }
                } else {
                    (0, 0)
                }
            }
            Err(_) => (0, 0),
        };

        let remaining = if total > 0 && total > used {
            (total - used).to_string()
        } else if total == 0 {
            String::from("—")
        } else {
            String::from("0")
        };

        Ok(ProviderQuota {
            provider: self.provider(),
            plan: plan_name,
            used,
            total,
            remaining,
            remaining_days: None,
            status: QuotaStatus::Ok,
            refreshed_at: Utc::now(),
        })
    }
}
