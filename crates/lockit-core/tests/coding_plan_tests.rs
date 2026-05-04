use lockit_core::coding_plan::{CodingPlanProvider, ProviderQuota, QuotaStatus};

#[test]
fn test_quota_usage_pct() {
    let quota = ProviderQuota {
        provider: CodingPlanProvider::QwenBailian,
        plan: "plus".into(),
        used: 847,
        total: 1000,
        remaining: "153".into(),
        remaining_days: Some(30),
        status: QuotaStatus::Ok,
        refreshed_at: chrono::Utc::now(),
    };
    assert_eq!(quota.usage_pct(), 84.7);
}

#[test]
fn test_quota_usage_pct_total_zero() {
    let quota = ProviderQuota {
        provider: CodingPlanProvider::Claude,
        plan: "pro".into(),
        used: 0,
        total: 0,
        remaining: "-".into(),
        remaining_days: None,
        status: QuotaStatus::Ok,
        refreshed_at: chrono::Utc::now(),
    };
    assert_eq!(quota.usage_pct(), 0.0);
}
