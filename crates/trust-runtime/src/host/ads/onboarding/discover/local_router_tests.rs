use super::*;
use crate::ads::onboarding::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

#[test]
fn preserves_local_router_identity_provenance() {
    let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::LocalRouter);
    let results = discover_targets(&mut wire, &DiscoveryRequest::manual("127.0.0.1"))
        .expect("local router discovery");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::LocalRouter);
}
