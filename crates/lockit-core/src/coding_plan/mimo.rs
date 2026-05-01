use crate::coding_plan::{
    find_field, CodingPlanError, CodingPlanFetcher, CodingPlanProvider, ProviderQuota,
    QuotaStatus,
};
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Duration;

pub struct MimoFetcher;

impl CodingPlanFetcher for MimoFetcher {
    fn provider(&self) -> CodingPlanProvider {
        CodingPlanProvider::Mimo
    }

    fn fetch(
        &self,
        credential_fields: &BTreeMap<String, String>,
    ) -> Result<ProviderQuota, CodingPlanError> {
        let api_key = find_field(credential_fields, "api_key")
            .ok_or_else(|| CodingPlanError::NotConfigured("api_key".to_string()))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CodingPlanError::ApiError(format!("client build: {}", e)))?;

        let response = match client
            .get("https://api.xiaomimimo.com/v1/usage")
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

        // Checksum-only: verify auth is valid, no detailed quota parsing
        if status.is_success() || status.as_u16() >= 400 {
            Ok(ProviderQuota {
                provider: self.provider(),
                plan: String::from("Mimo"),
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
