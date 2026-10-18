use serde_json::json;

use super::*;

#[test]
fn sanitize_id_retains_only_closed_ascii_vocabulary() {
    for (input, expected) in [
        ("alpha-1_beta.two", "alpha-1_beta.two"),
        ("a:b/c d", "a_b_c_d"),
        ("åäö", "___"),
        ("", ""),
    ] {
        assert_eq!(sanitize_id(input), expected);
    }
}

#[test]
fn candidate_serialization_omits_empty_warning_list() {
    let value = serde_json::to_value(DiscoverCandidate {
        id: "mqtt:127.0.0.1_1883".to_string(),
        label: "MQTT broker 127.0.0.1:1883".to_string(),
        source: "tcp_connect",
        confidence: "port_reachable",
        params: json!({ "broker": "127.0.0.1:1883" }),
        warnings: Vec::new(),
    })
    .expect("candidate serialization");
    assert!(value.get("warnings").is_none());
    assert_eq!(value["source"], "tcp_connect");
    assert_eq!(value["confidence"], "port_reachable");
}
