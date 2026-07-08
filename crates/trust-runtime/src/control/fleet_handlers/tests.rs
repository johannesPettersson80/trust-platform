use serde_json::json;
use smol_str::SmolStr;

use super::{ads_server_params, host_name_from_sources, normalized_host_name};
use crate::ads::server::{AdsServerClientConfig, AdsServerRuntimeConfig, AdsServerSourcePin};

#[test]
fn host_name_uses_os_hostname_before_literal_fallback() {
    assert_eq!(
        host_name_from_sources(None, None, Some("raspberrypi")),
        "raspberrypi"
    );
    assert_eq!(
        host_name_from_sources(Some(""), Some("  "), Some("raspberrypi")),
        "raspberrypi"
    );
    assert_eq!(host_name_from_sources(None, None, None), "local-host");
}

#[test]
fn host_name_normalization_trims_whitespace_and_trailing_dot() {
    assert_eq!(
        normalized_host_name(" raspberrypi.local. "),
        Some("raspberrypi.local".to_string())
    );
    assert_eq!(normalized_host_name("  "), None);
}

#[test]
fn ads_server_params_humanize_allowed_clients_without_raw_pin_objects() {
    let config = AdsServerRuntimeConfig {
        clients: vec![
            AdsServerClientConfig {
                ams_net_id: trust_ads_core::AmsNetId::new("127.0.0.1.1.100"),
                source: AdsServerSourcePin::Ip(SmolStr::new("127.0.0.1")),
            },
            AdsServerClientConfig {
                ams_net_id: trust_ads_core::AmsNetId::new("5.23.91.12.1.1"),
                source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
            },
        ],
        ..AdsServerRuntimeConfig::default()
    };

    let params = ads_server_params(&config);
    assert_eq!(
        params.get("clients_summary"),
        Some(&json!([
            "127.0.0.1.1.100 (from 127.0.0.1)",
            "5.23.91.12.1.1 (from 127.0.0.0/8)",
        ]))
    );
    assert!(
        params.get("clients").is_none(),
        "topology params must not send editable raw ADS client objects"
    );

    let payload = params.to_string();
    assert!(!payload.contains("source_ip"), "{payload}");
    assert!(!payload.contains("source_cidr"), "{payload}");
}
