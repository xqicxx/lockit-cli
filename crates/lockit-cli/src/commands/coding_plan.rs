use anyhow::Context;
use lockit_core::coding_plan::{
    ChatGptFetcher, ClaudeFetcher, CodingPlanFetcher, CodingPlanProvider, DeepSeekFetcher,
    MimoFetcher, QwenFetcher,
};
use lockit_core::credential::CredentialType;
use lockit_core::vault::{unlock_vault, VaultPaths};
use std::collections::BTreeMap;
use tabled::Tabled;

use crate::output;

/// Create the appropriate fetcher for a given provider.
fn create_fetcher(provider: &CodingPlanProvider) -> Option<Box<dyn CodingPlanFetcher>> {
    match provider {
        CodingPlanProvider::QwenBailian => Some(Box::new(QwenFetcher)),
        CodingPlanProvider::ChatGpt => Some(Box::new(ChatGptFetcher)),
        CodingPlanProvider::Claude => Some(Box::new(ClaudeFetcher)),
        CodingPlanProvider::DeepSeek => Some(Box::new(DeepSeekFetcher)),
        CodingPlanProvider::Mimo => Some(Box::new(MimoFetcher)),
        // Providers without fetchers yet: OpenAi, Anthropic, Google, Moonshot, MiniMax, Glm
        _ => None,
    }
}

// ---- list command ----

#[derive(Tabled)]
struct CodingPlanRow {
    #[tabled(rename = "PROVIDER")]
    provider: String,
    #[tabled(rename = "PLAN")]
    plan: String,
    #[tabled(rename = "QUOTA USED")]
    quota_used: String,
    #[tabled(rename = "REMAINING")]
    remaining: String,
    #[tabled(rename = "STATUS")]
    status: String,
}

pub fn list(paths: &VaultPaths) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    let session = unlock_vault(paths, &pw)?;

    let credentials = session.list_credentials();
    let coding_plan_creds: Vec<_> = credentials
        .iter()
        .filter(|c| c.r#type == CredentialType::CodingPlan)
        .collect();

    if coding_plan_creds.is_empty() {
        println!("(no coding plan credentials)");
        return Ok(());
    }

    let rows: Vec<CodingPlanRow> = coding_plan_creds
        .iter()
        .map(|c| {
            let provider = c
                .fields
                .get("provider")
                .cloned()
                .unwrap_or_else(|| String::from("—"));
            CodingPlanRow {
                provider,
                plan: String::from("—"),
                quota_used: String::from("—"),
                remaining: String::from("—"),
                status: String::from("—"),
            }
        })
        .collect();

    let mut table = tabled::Table::new(rows);
    table.with(tabled::settings::Style::modern_rounded());
    println!("{table}");

    Ok(())
}

// ---- refresh command ----

pub fn refresh(
    paths: &VaultPaths,
    provider_filter: Option<String>,
) -> anyhow::Result<()> {
    let pw = crate::utils::vault_key();
    let mut session = unlock_vault(paths, &pw)?;

    // Collect immutable info first (so we can borrow mutably later for reveal_secret)
    struct CpInfo {
        id: String,
        name: String,
    }

    let cp_infos: Vec<CpInfo> = {
        let credentials = session.list_credentials();
        credentials
            .iter()
            .filter(|c| c.r#type == CredentialType::CodingPlan)
            .filter(|c| {
                if let Some(ref filter) = provider_filter {
                    let prov = c.fields.get("provider").map(|s| s.as_str()).unwrap_or("");
                    prov.to_ascii_lowercase()
                        .contains(&filter.to_ascii_lowercase())
                } else {
                    true
                }
            })
            .map(|c| CpInfo {
                id: c.id.clone(),
                name: c.name.clone(),
            })
            .collect()
    };

    if cp_infos.is_empty() {
        println!("(no coding plan credentials to refresh)");
        return Ok(());
    }

    let mut errors: Vec<String> = Vec::new();

    for info in &cp_infos {
        let api_key = match session.reveal_secret(&info.id, "api_key") {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}: could not read API_KEY: {}", info.name, e));
                continue;
            }
        };

        let provider_field = session
            .reveal_secret(&info.id, "provider")
            .unwrap_or_default();
        let cookie = session
            .reveal_secret(&info.id, "cookie")
            .unwrap_or_default();
        let base_url = session
            .reveal_secret(&info.id, "base_url")
            .unwrap_or_default();

        // Resolve provider
        let provider = match CodingPlanProvider::from_field_value(&provider_field) {
            Some(p) => p,
            None => {
                output::error(&format!(
                    "{}: unknown provider '{}'",
                    info.name, provider_field
                ));
                continue;
            }
        };

        // Create fetcher
        let fetcher = match create_fetcher(&provider) {
            Some(f) => f,
            None => {
                output::error(&format!(
                    "{}: no fetcher available for provider '{}'",
                    info.name,
                    provider.display_name()
                ));
                continue;
            }
        };

        // Build fields map for the fetcher
        let mut fields_map = BTreeMap::new();
        fields_map.insert("provider".to_string(), provider_field.clone());
        fields_map.insert("api_key".to_string(), api_key);
        fields_map.insert("cookie".to_string(), cookie);
        fields_map.insert("base_url".to_string(), base_url);

        // Fetch and display
        match fetcher.fetch(&fields_map) {
            Ok(quota) => {
                let status_str = match &quota.status {
                    lockit_core::coding_plan::QuotaStatus::Ok => "ok".to_string(),
                    lockit_core::coding_plan::QuotaStatus::AuthExpired => {
                        "auth expired".to_string()
                    }
                    lockit_core::coding_plan::QuotaStatus::Error(e) => format!("error: {}", e),
                };
                let used_str = if quota.total > 0 {
                    quota.used.to_string()
                } else {
                    String::from("—")
                };
                output::success(&format!(
                    "{}: plan={} used={} remaining={} status={}",
                    info.name,
                    if quota.plan.is_empty() {
                        "—"
                    } else {
                        &quota.plan
                    },
                    used_str,
                    quota.remaining,
                    status_str,
                ));
            }
            Err(e) => {
                output::error(&format!("{}: {}", info.name, e));
            }
        }
    }

    // Save audit events from reveal_secret calls
    session.save().context("save vault after refresh")?;

    if !errors.is_empty() {
        eprintln!("\n{} provider(s) failed:", errors.len());
        for err in &errors {
            eprintln!("  - {err}");
        }
    }

    Ok(())
}
