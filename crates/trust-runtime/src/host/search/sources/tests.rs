use serde_json::json;
use smol_str::SmolStr;

use super::adapters::{confidence, host_part};
use super::catalog::default_subnets;
use crate::ads::onboarding::RuntimeAddressCandidate;
use crate::connectors::DiscoveryConfidence;

fn interface(nic: &str, ip: &str) -> RuntimeAddressCandidate {
    RuntimeAddressCandidate {
        ip: ip.to_string(),
        nic: Some(nic.to_string()),
        prefix_len: Some(24),
        broadcast: None,
    }
}

#[test]
fn search_confidence_is_the_closed_set() {
    assert_eq!(confidence("confirmed"), DiscoveryConfidence::Confirmed);
    assert_eq!(confidence("likely"), DiscoveryConfidence::Likely);
    assert_eq!(
        confidence("port_reachable"),
        DiscoveryConfidence::PortReachable
    );
    assert_eq!(confidence("unavailable"), DiscoveryConfidence::Unavailable);
    // `comm.discover` says `observed` for an ADS identify reply, a GetEndpoints answer, an
    // EtherCAT read, a GPIO listing and an mDNS answer; the search reports each as confirmed.
    assert_eq!(confidence("observed"), DiscoveryConfidence::Confirmed);
    for word in ["", "Confirmed", "healthy", "port-reachable"] {
        assert_eq!(
            confidence(word),
            DiscoveryConfidence::Unavailable,
            "word={word:?}"
        );
    }
    for value in [
        DiscoveryConfidence::Confirmed,
        DiscoveryConfidence::Likely,
        DiscoveryConfidence::PortReachable,
        DiscoveryConfidence::Unavailable,
    ] {
        let word = serde_json::to_value(value).expect("serialize");
        assert!(
            [
                json!("confirmed"),
                json!("likely"),
                json!("port_reachable"),
                json!("unavailable")
            ]
            .contains(&word),
            "{word}"
        );
    }
}

#[test]
fn default_subnet_follows_the_decided_rule() {
    let interfaces = [
        interface("lo", "127.0.0.1"),
        interface("eth0", "192.168.1.20"),
        interface("eth0", "192.168.1.21"),
        interface("wlan0", "10.0.5.7"),
        interface("docker0", "172.17.0.1"),
        interface("eth1", "8.8.8.8"),
        interface("eth2", "169.254.10.3"),
        interface("eth0", "fe80::1"),
    ];
    assert_eq!(
        default_subnets(&interfaces, &[]),
        vec!["192.168.1.0/24", "10.0.5.0/24", "172.17.0.0/24"]
    );
    assert_eq!(
        default_subnets(&interfaces, &[SmolStr::new("wlan0")]),
        vec!["10.0.5.0/24"]
    );
    assert!(default_subnets(&interfaces, &[SmolStr::new("eth1")]).is_empty());
}

#[test]
fn known_address_host_drops_only_its_port() {
    assert_eq!(host_part("10.0.0.9:502"), "10.0.0.9");
    assert_eq!(host_part("10.0.0.9"), "10.0.0.9");
    assert_eq!(host_part("plc.local:1883"), "plc.local");
    assert_eq!(host_part("[fe80::1]:4840"), "fe80::1");
    assert_eq!(host_part("[fe80::1]"), "fe80::1");
    assert_eq!(host_part("fe80::1"), "fe80::1");
}
