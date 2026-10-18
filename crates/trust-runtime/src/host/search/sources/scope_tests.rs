use std::net::Ipv4Addr;
use std::time::Duration;

use super::*;

fn scope() -> DiscoverScope {
    DiscoverScope::default()
}

#[test]
fn timeout_defaults_and_clamps_to_closed_bounds() {
    assert_eq!(timeout(&scope()), Duration::from_millis(DEFAULT_TIMEOUT_MS));
    for (requested, expected) in [
        (0, 1),
        (1, 1),
        (150, 150),
        (MAX_TIMEOUT_MS, MAX_TIMEOUT_MS),
        (MAX_TIMEOUT_MS + 1, MAX_TIMEOUT_MS),
        (u64::MAX, MAX_TIMEOUT_MS),
    ] {
        let scope = DiscoverScope {
            timeout_ms: Some(requested),
            ..scope()
        };
        assert_eq!(
            timeout(&scope),
            Duration::from_millis(expected),
            "timeout {requested}"
        );
    }
}

#[test]
fn ipv4_cidr_parser_trims_and_accepts_closed_prefix_domain() {
    for (cidr, expected_ip, expected_prefix) in [
        ("10.20.30.40/24", Ipv4Addr::new(10, 20, 30, 40), 24),
        (" 10.20.30.40/31 ", Ipv4Addr::new(10, 20, 30, 40), 31),
        ("192.168.1.7/32", Ipv4Addr::new(192, 168, 1, 7), 32),
        ("0.0.0.0/0", Ipv4Addr::UNSPECIFIED, 0),
    ] {
        assert_eq!(
            parse_ipv4_cidr(cidr).expect("valid CIDR"),
            (expected_ip, expected_prefix)
        );
    }
}

#[test]
fn ipv4_cidr_parser_rejects_malformed_forms() {
    for cidr in [
        "",
        "10.0.0.1",
        "10.0.0.1/",
        "/24",
        "10.0.0.1/not-a-prefix",
        "10.0.0.1/33",
        "300.0.0.1/24",
        "10.0.0.1/24/extra",
        "::1/128",
    ] {
        assert!(
            parse_ipv4_cidr(cidr).is_err(),
            "malformed CIDR accepted: {cidr}"
        );
    }
}

#[test]
fn cidr_enumeration_rejects_ranges_broader_than_slash_24() {
    for cidr in ["0.0.0.0/0", "10.0.0.0/8", "10.0.0.0/16", "10.0.0.0/23"] {
        let error = ipv4_targets_for_cidr(cidr).expect_err("broad CIDR accepted");
        assert!(error.contains("/24 or tighter"), "{error}");
    }
}

#[test]
fn cidr_enumeration_is_canonical_ascending_and_bounded() {
    let cases = [
        (
            "10.0.0.42/24",
            254,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(10, 0, 0, 254),
        ),
        (
            "10.0.0.130/25",
            126,
            Ipv4Addr::new(10, 0, 0, 129),
            Ipv4Addr::new(10, 0, 0, 254),
        ),
        (
            "10.0.0.6/30",
            2,
            Ipv4Addr::new(10, 0, 0, 5),
            Ipv4Addr::new(10, 0, 0, 6),
        ),
        (
            "10.0.0.6/31",
            2,
            Ipv4Addr::new(10, 0, 0, 6),
            Ipv4Addr::new(10, 0, 0, 7),
        ),
        (
            "10.0.0.6/32",
            1,
            Ipv4Addr::new(10, 0, 0, 6),
            Ipv4Addr::new(10, 0, 0, 6),
        ),
    ];
    for (cidr, count, first, last) in cases {
        let targets = ipv4_targets_for_cidr(cidr).expect("CIDR targets");
        assert_eq!(targets.len(), count, "{cidr}");
        assert_eq!(targets.first(), Some(&first), "{cidr}");
        assert_eq!(targets.last(), Some(&last), "{cidr}");
        assert!(
            targets.windows(2).all(|pair| pair[0] < pair[1]),
            "{cidr} is not strictly ascending"
        );
    }
}
