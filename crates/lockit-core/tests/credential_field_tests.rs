use lockit_core::credential::CredentialType;
use lockit_core::credential_field::TypeFieldMap;

#[test]
fn test_api_key_has_four_fields() {
    let fields = TypeFieldMap::fields_for(&CredentialType::ApiKey);
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].label, "NAME");
    assert_eq!(fields[1].label, "SERVICE");
    assert!(fields[1].is_dropdown);
    assert!(!fields[1].presets.is_empty());
    assert_eq!(fields[2].label, "KEY_IDENTIFIER");
    assert_eq!(fields[3].label, "SECRET_VALUE");
}

#[test]
fn test_coding_plan_required_fields() {
    let indices = CredentialType::CodingPlan.required_field_indices();
    assert_eq!(indices, vec![2, 4]); // API_KEY, BASE_URL
}

#[test]
fn test_every_type_has_fields() {
    for ct in CredentialType::all() {
        let fields = TypeFieldMap::fields_for(&ct);
        assert!(!fields.is_empty(), "{} has no fields", ct.name());
    }
}

#[test]
fn test_preset_values_exist_for_dropdowns() {
    let fields = TypeFieldMap::fields_for(&CredentialType::CodingPlan);
    let provider_field = &fields[0];
    assert!(provider_field.is_dropdown);
    assert!(provider_field.presets.contains(&"openai".to_string()));
    assert!(provider_field.presets.contains(&"anthropic".to_string()));
}
