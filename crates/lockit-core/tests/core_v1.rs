use lockit_core::credential::{CredentialDraft, CredentialType};
use lockit_core::crypto::{decrypt_vault_bytes, encrypt_vault_bytes, CryptoParams};
use lockit_core::migration::parse_legacy_markdown;
use lockit_core::sync::{
    compute_sync_status, plan_smart_sync, sha256_checksum, SmartSyncPlan, SyncCheckpoint,
    SyncCrypto, SyncInputs, SyncManifest, SyncStatus,
};
use lockit_core::vault::{init_vault, unlock_vault, VaultPaths};
use tempfile::tempdir;

fn paths() -> (tempfile::TempDir, VaultPaths) {
    let dir = tempdir().unwrap();
    let paths = VaultPaths::new(dir.path().join("vault.enc"));
    (dir, paths)
}

#[test]
fn credential_type_roundtrips_and_redacts_secret_values() {
    assert_eq!(
        "api-key".parse::<CredentialType>().unwrap(),
        CredentialType::ApiKey
    );
    assert_eq!(
        "GITHUB".parse::<CredentialType>().unwrap(),
        CredentialType::GitHub
    );
    assert_eq!(CredentialType::SshKey.to_string(), "ssh_key");

    let draft = CredentialDraft::new(
        "OPENAI_API_KEY",
        CredentialType::ApiKey,
        "openai",
        "default",
        serde_json::json!({ "value": "sk-test-abcdef123456" }),
    );
    let credential = draft.into_credential();
    let redacted = credential.redacted();

    assert_eq!(redacted.name, "OPENAI_API_KEY");
    assert_eq!(redacted.fields.get("value").unwrap(), "sk-t••••3456");
    assert!(credential.matches_query("openai"));
    assert!(credential.matches_query("api key"));
}

#[test]
fn crypto_roundtrips_rejects_wrong_password_and_corruption() {
    let payload = br#"{"schema_version":2,"credentials":[]}"#;
    let params = CryptoParams::default_for_new_vault();
    let encrypted = encrypt_vault_bytes(payload, "correct horse battery staple", &params).unwrap();

    let decrypted = decrypt_vault_bytes(&encrypted, "correct horse battery staple").unwrap();
    assert_eq!(decrypted, payload);

    assert!(decrypt_vault_bytes(&encrypted, "wrong password").is_err());

    let mut corrupted = encrypted.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0xFF;
    assert!(decrypt_vault_bytes(&corrupted, "correct horse battery staple").is_err());
}

#[test]
fn vault_lifecycle_returns_redacted_list_and_reveals_only_explicitly() {
    let (_dir, paths) = paths();
    init_vault(&paths, "master-password").unwrap();

    let mut session = unlock_vault(&paths, "master-password").unwrap();
    let id = session
        .add_credential(CredentialDraft::new(
            "GITHUB_PAT",
            CredentialType::Token,
            "github",
            "default",
            serde_json::json!({ "value": "ghp_abcdefghijklmnopqrstuvwxyz" }),
        ))
        .unwrap();
    session.save().unwrap();

    let list = session.list_credentials();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].fields.get("value").unwrap(), "ghp_••••wxyz");
    assert_eq!(
        session.reveal_secret(&id, "value").unwrap(),
        "ghp_abcdefghijklmnopqrstuvwxyz"
    );
    assert_eq!(session.search_credentials("git").len(), 1);

    session.lock();
    assert!(session.reveal_secret(&id, "value").is_err());
    assert!(unlock_vault(&paths, "wrong-password").is_err());
}

#[test]
fn legacy_markdown_imports_into_structured_drafts() {
    let markdown = r#"# Lockit Credentials

| Name | Type | Service | Key | Value | Metadata | Created | Updated |
|------|------|---------|-----|-------|----------|---------|--------|
| OPENAI_API_KEY | api_key | openai | default | sk-test | {} | 2026-04-28T00:00:00Z | 2026-04-28T00:00:00Z |
| SSH_DEPLOY | ssh_key | prod | ed25519 | private-key | {"note":"deploy"} | 2026-04-28T00:00:00Z | 2026-04-28T00:00:00Z |
"#;

    let drafts = parse_legacy_markdown(markdown).unwrap();
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].r#type, CredentialType::ApiKey);
    assert_eq!(drafts[0].fields.get("value").unwrap(), "sk-test");
    assert_eq!(drafts[1].metadata.get("note").unwrap(), "deploy");
}

#[test]
fn sync_manifest_checksum_status_and_sync_crypto_are_compatible() {
    let checksum_a = sha256_checksum(b"local");
    let checksum_b = sha256_checksum(b"cloud");
    assert!(checksum_a.starts_with("sha256:"));

    let manifest = SyncManifest::new(checksum_a.clone(), "mac-mini", 128, 2);
    let json = serde_json::to_string(&manifest).unwrap();
    let parsed: SyncManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.vault_checksum, checksum_a);
    assert_eq!(parsed.schema_version, 2);

    assert_eq!(
        compute_sync_status(SyncInputs {
            local_checksum: checksum_b.clone(),
            cloud_manifest: Some(parsed.clone()),
            checkpoint: Some(SyncCheckpoint {
                local_checksum: checksum_a.clone(),
                cloud_checksum: parsed.vault_checksum.clone(),
            }),
            sync_key_configured: true,
            backend_configured: true,
        }),
        SyncStatus::LocalAhead
    );

    assert_eq!(
        compute_sync_status(SyncInputs {
            local_checksum: parsed.vault_checksum.clone(),
            cloud_manifest: Some(parsed),
            checkpoint: Some(SyncCheckpoint {
                local_checksum: checksum_b,
                cloud_checksum: checksum_a,
            }),
            sync_key_configured: true,
            backend_configured: true,
        }),
        SyncStatus::UpToDate
    );

    let key = SyncCrypto::generate_key();
    let encoded = SyncCrypto::encode_key(&key);
    assert_eq!(SyncCrypto::decode_key(&encoded).unwrap(), key);
    let blob = SyncCrypto::encrypt(b"vault bytes", &key).unwrap();
    assert_eq!(SyncCrypto::decrypt(&blob, &key).unwrap(), b"vault bytes");
}

#[test]
fn smart_sync_plan_matches_android_conflict_semantics() {
    let cloud_manifest = SyncManifest::new(sha256_checksum(b"cloud-v1"), "android", 64, 2);
    let checkpoint = SyncCheckpoint {
        local_checksum: sha256_checksum(b"local-v1"),
        cloud_checksum: cloud_manifest.vault_checksum.clone(),
    };

    assert_eq!(
        plan_smart_sync(SyncInputs {
            local_checksum: checkpoint.local_checksum.clone(),
            cloud_manifest: None,
            checkpoint: None,
            sync_key_configured: true,
            backend_configured: true,
        }),
        SmartSyncPlan::Push
    );

    assert_eq!(
        plan_smart_sync(SyncInputs {
            local_checksum: sha256_checksum(b"local-v2"),
            cloud_manifest: Some(cloud_manifest.clone()),
            checkpoint: Some(checkpoint.clone()),
            sync_key_configured: true,
            backend_configured: true,
        }),
        SmartSyncPlan::Push
    );

    let changed_cloud = SyncManifest::new(sha256_checksum(b"cloud-v2"), "android", 80, 2);
    assert_eq!(
        plan_smart_sync(SyncInputs {
            local_checksum: checkpoint.local_checksum.clone(),
            cloud_manifest: Some(changed_cloud.clone()),
            checkpoint: Some(checkpoint.clone()),
            sync_key_configured: true,
            backend_configured: true,
        }),
        SmartSyncPlan::Pull
    );

    assert_eq!(
        plan_smart_sync(SyncInputs {
            local_checksum: sha256_checksum(b"local-v2"),
            cloud_manifest: Some(changed_cloud),
            checkpoint: Some(checkpoint),
            sync_key_configured: true,
            backend_configured: true,
        }),
        SmartSyncPlan::Conflict
    );
}
