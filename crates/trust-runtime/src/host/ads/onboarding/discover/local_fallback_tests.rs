use super::*;
use crate::ads::onboarding::{MockAdsOnboardingScenario, MockAdsOnboardingWire};

#[test]
fn failed_same_host_identity_does_not_block_lan_broadcast_results() {
    let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::DirectedIdentifyBlocked);
    let request = DiscoveryRequest {
        target: Some("127.0.0.1".to_string()),
        directed_targets: Vec::new(),
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: true,
        broadcast_targets: vec!["192.168.77.255".to_string()],
        timeout_ms: Some(3_000),
    };

    let results = discover_targets(&mut wire, &request)
        .expect("LAN broadcast discovery survives an unavailable local AMS Router");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::DirectedBroadcast);
    assert_eq!(results[0].target.ams_net_id, "5.23.91.12.1.1");
}

#[test]
fn failed_same_host_identity_falls_back_to_directed_interface_identify() {
    let mut wire = MockAdsOnboardingWire::new(MockAdsOnboardingScenario::LoopbackIdentifyBlocked);
    let request = DiscoveryRequest {
        target: Some("127.0.0.1".to_string()),
        directed_targets: vec!["192.168.77.11".to_string()],
        target_ams_net_id: None,
        ams_port: Some(851),
        target_name: None,
        include_broadcast: true,
        broadcast_targets: Vec::new(),
        timeout_ms: Some(3_000),
    };

    let results = discover_targets(&mut wire, &request)
        .expect("interface-directed identify survives an unavailable loopback AMS Router");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, DiscoverySource::DirectedIdentify);
    assert_eq!(results[0].target.ams_net_id, "5.23.91.12.1.1");
}
