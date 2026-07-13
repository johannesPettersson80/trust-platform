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

#[test]
fn keeps_distinct_local_runtimes_that_share_one_windows_ip() {
    let mut results = Vec::new();
    for (name, ams_net_id) in [
        ("UmRT_Default", "10.20.30.40.1.1"),
        ("Local ADS runtime", "10.20.30.41.1.1"),
    ] {
        push_unique(
            &mut results,
            DiscoveryResult {
                target: ObservedAdsIdentity {
                    name: Some(name.to_string()),
                    ip: "127.0.0.1".to_string(),
                    ams_net_id: ams_net_id.to_string(),
                    preferred_ams_port: Some(851),
                    responding_ads_ports: vec![851],
                    tc_version: None,
                },
                source: DiscoverySource::LocalRouter,
            },
        );
    }

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].target.ams_net_id, "10.20.30.40.1.1");
    assert_eq!(results[1].target.ams_net_id, "10.20.30.41.1.1");
}

#[test]
fn udp_identity_and_native_port_proof_merge_without_claiming_udp_851() {
    let identity = |source, preferred_ams_port, responding_ads_ports| DiscoveryResult {
        target: ObservedAdsIdentity {
            name: Some("Local controller".to_string()),
            ip: "127.0.0.1".to_string(),
            ams_net_id: "10.20.30.40.1.1".to_string(),
            preferred_ams_port,
            responding_ads_ports,
            tc_version: None,
        },
        source,
    };
    let mut results = Vec::new();
    push_unique(
        &mut results,
        identity(DiscoverySource::DirectedIdentify, None, Vec::new()),
    );
    push_unique(
        &mut results,
        identity(DiscoverySource::LocalRouter, Some(501), vec![501, 301]),
    );

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].target.preferred_ams_port, Some(501));
    assert_eq!(results[0].target.responding_ads_ports, vec![501, 301]);
    assert!(!results[0].target.responding_ads_ports.contains(&851));
}
