use super::*;

#[test]
fn error_classifier_maps_common_failures_to_remediation_and_actions() {
    let route = classify_onboarding_failure(OnboardingFailureKind::RouteMissing);
    assert_eq!(route.next_action.kind, NextActionKind::AddRoute);
    assert!(route.remediation.contains("static ADS route"));
    assert!(route.blocks_production_ready);

    let secure = classify_onboarding_failure(OnboardingFailureKind::SecureRequired);
    assert_eq!(secure.next_action.kind, NextActionKind::UseSecure);
    assert!(secure.explanation.contains("Secure ADS"));

    let local_router = classify_onboarding_failure(OnboardingFailureKind::LocalRouterUnavailable);
    assert_eq!(local_router.next_action.kind, NextActionKind::PickTarget);
    assert!(local_router.explanation.contains("this computer"));
    assert!(local_router.remediation.contains("local ADS router"));
    assert!(!local_router
        .remediation
        .to_ascii_lowercase()
        .contains("firewall"));

    let fingerprint = classify_ads_error_code(1861).expect("known ADS code");
    assert_eq!(fingerprint.kind, OnboardingFailureKind::Fingerprint1861);
    assert_eq!(
        fingerprint.ads_error,
        Some(AdsErrorInfo::new(1861, "ADSERR_DEVICE_INVALIDCONTEXT"))
    );
}
