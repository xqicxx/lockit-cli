use crate::coding_plan::{CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota, QuotaStatus};
use crate::credential::find_field_insensitive;
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Duration;

pub struct DeepSeekFetcher;

impl CodingPlanFetcher for DeepSeekFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::DeepSeek
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
            .get("https://api.deepseek.com/v1/user/balance")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                return Err(CodingPlanError::ApiError(format!(
                    "request failed: {}",
                    e
                )));
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

        // Parse balance_infos array (DeepSeek returns balance breakdown)
        let (used, total): (u64, u64) = if let Some(infos) =
            body.get("balance_infos").and_then(|v: &serde_json::Value| v.as_array())
        {
            // Each entry has a type field (e.g. "used", "total") and amount/total_tokens
            let find_amount = |typ: &str| -> Option<f64> {
                infos
                    .iter()
                    .find(|item: &&serde_json::Value| {
                        item.get("type")
                            .and_then(|t: &serde_json::Value| t.as_str())
                            .map(|s: &str| s.eq_ignore_ascii_case(typ))
                            .unwrap_or(false)
                    })
                    .and_then(|item: &serde_json::Value| {
                        item.get("total_tokens")
                            .or_else(|| item.get("amount"))
                            .or_else(|| item.get("value"))
                    })
                    .and_then(|v: &serde_json::Value| v.as_f64())
            };

            let u = find_amount("used").unwrap_or(0.0) as u64;
            let t = find_amount("total").unwrap_or(0.0) as u64;
            (u, t)
        } else {
            // Fallback: try direct fields
            let u = body
                .get("used_tokens")
                .or_else(|| body.get("used"))
                .and_then(|v: &serde_json::Value| v.as_f64())
                .unwrap_or(0.0) as u64;
            let t = body
                .get("total_tokens")
                .or_else(|| body.get("total"))
                .and_then(|v: &serde_json::Value| v.as_f64())
                .unwrap_or(0.0) as u64;
            (u, t)
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
            plan: String::from("DeepSeek"),
            used,
            total,
            remaining,
            remaining_days: None,
            status: QuotaStatus::Ok,
            refreshed_at: Utc::now(),
        })
    }
}
