//! Credential-channel contracts, moved from trust-runtime
//! (`control/tests/control_request_boundary_contract.rs`) with the classification.

use serde_json::json;

use super::*;

fn control_request(kind: &str, params: Option<serde_json::Value>) -> ControlRequest {
    ControlRequest {
        id: 7,
        r#type: kind.into(),
        params,
        auth: None,
        request_id: None,
    }
}

#[test]
fn credential_channel_classification_covers_complete_trust_partition() {
    for client in [
        None,
        Some("unix"),
        Some("loopback"),
        Some("127.0.0.1:1"),
        Some("127.255.255.255:65535"),
        Some("[::1]:1"),
    ] {
        assert_eq!(
            credential_channel_for_client(client),
            "trusted_same_host",
            "client={client:?}"
        );
    }
    for client in [
        Some(""),
        Some("local"),
        Some("localhost:1"),
        Some("192.0.2.1:1"),
        Some("[2001:db8::1]:1"),
        Some("127.0.0.1"),
        Some("tcp://127.0.0.1:1"),
        Some("UNIX"),
        Some(" unix"),
    ] {
        assert_eq!(
            credential_channel_for_client(client),
            "untrusted_remote_plain_tcp",
            "client={client:?}"
        );
    }
}

#[test]
fn credential_channel_is_injected_for_exact_communication_operations() {
    for kind in [
        "comm.apply",
        "comm.test",
        "comm.browse_symbols",
        "ads.import_symbols.apply",
    ] {
        let mut request = control_request(kind, None);
        stamp_credential_channel_param(&mut request, Some("unix"));
        assert_eq!(
            request
                .params
                .as_ref()
                .and_then(|params| params.get("credential_channel")),
            Some(&json!("trusted_same_host")),
            "kind={kind}"
        );
    }
}

#[test]
fn credential_channel_replaces_caller_supplied_value() {
    let mut request = control_request(
        "comm.apply",
        Some(json!({
            "protocol": "modbus_tcp",
            "credential_channel": "caller-controlled"
        })),
    );
    stamp_credential_channel_param(&mut request, Some("192.0.2.1:9000"));

    let params = request.params.expect("params");
    assert_eq!(params["protocol"], json!("modbus_tcp"));
    assert_eq!(
        params["credential_channel"],
        json!("untrusted_remote_plain_tcp")
    );
}

#[test]
fn credential_channel_does_not_rewrite_nonobject_params() {
    for params in [json!(null), json!([]), json!("params"), json!(7)] {
        let mut request = control_request("comm.test", Some(params.clone()));
        stamp_credential_channel_param(&mut request, Some("unix"));
        assert_eq!(request.params, Some(params));
    }
}

#[test]
fn credential_channel_is_not_injected_into_other_operations() {
    for kind in [
        "status",
        "ads.import_symbols",
        "comm.schema",
        "comm.discover",
        "io.write",
    ] {
        let original = Some(json!({"credential_channel": "caller"}));
        let mut request = control_request(kind, original.clone());
        stamp_credential_channel_param(&mut request, Some("unix"));
        assert_eq!(request.params, original, "kind={kind}");
    }
}
