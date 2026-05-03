use super::*;

#[test]
fn registry_profile_covers_required_endpoints() {
    let profile = registry_api_profile();
    assert_eq!(profile.api_version, "v1");
    assert!(profile
        .endpoints
        .iter()
        .any(|endpoint| endpoint.path == "/v1/packages/{name}/{version}"));
    assert!(profile
        .endpoints
        .iter()
        .any(|endpoint| endpoint.path == "/v1/packages/{name}/{version}/verify"));
    assert!(profile
        .metadata_model
        .package_fields
        .iter()
        .any(|field| field == "package_sha256"));
}
