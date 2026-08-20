use std::net::SocketAddr;

use tiny_http::{Header, Request, TestRequest};

use super::*;

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn request(headers: &[(&str, &str)]) -> Request {
    let mut request = TestRequest::new()
        .with_remote_addr("127.0.0.1:4100".parse::<SocketAddr>().expect("socket"));
    for (name, value) in headers {
        request = request.with_header(header(name, value));
    }
    request.into()
}

#[test]
fn header_lookup_is_ascii_case_insensitive_and_trims_value() {
    let request = request(&[("x-CuStOm-HeAdEr", "  exact value \t")]);
    assert_eq!(
        header_value(&request, "X-Custom-Header").as_deref(),
        Some("exact value")
    );
}

#[test]
fn header_lookup_returns_first_matching_header() {
    let request = request(&[("X-Test", "first"), ("x-test", "second")]);
    assert_eq!(header_value(&request, "X-Test").as_deref(), Some("first"));
}

#[test]
fn absent_header_returns_none() {
    assert_eq!(header_value(&request(&[]), "X-Missing"), None);
}

#[test]
fn web_url_projects_http_and_https() {
    assert_eq!(
        format_web_url("runtime.example:8080", false),
        "http://runtime.example:8080"
    );
    assert_eq!(
        format_web_url("runtime.example:8443", true),
        "https://runtime.example:8443"
    );
}

#[test]
fn web_url_maps_ipv4_wildcard_to_localhost() {
    assert_eq!(
        format_web_url("0.0.0.0:8080", false),
        "http://localhost:8080"
    );
}

#[test]
fn web_url_preserves_ipv4_authority() {
    assert_eq!(
        format_web_url("127.0.0.1:8080", false),
        "http://127.0.0.1:8080"
    );
}

#[test]
fn web_url_brackets_ipv6_authority() {
    assert_eq!(format_web_url("[::1]:8080", false), "http://[::1]:8080");
}

#[test]
fn limit_parser_accepts_first_exact_unsigned_decimal_value() {
    assert_eq!(parse_limit("/events?limit=42"), Some(42));
    assert_eq!(parse_limit("/events?x=1&limit=0&limit=9"), Some(0));
    assert_eq!(parse_limit("/events?unlimited=7&limit=8"), Some(8));
}

#[test]
fn limit_parser_rejects_missing_empty_signed_or_overflow_value() {
    for url in [
        "/events",
        "/events?",
        "/events?limit=",
        "/events?limit=-1",
        "/events?limit=+1",
        "/events?limit=18446744073709551616",
    ] {
        assert_eq!(parse_limit(url), None, "{url}");
    }
}

#[test]
fn query_value_returns_first_exact_key_and_empty_value() {
    assert_eq!(
        query_value("/api?target=first&target=second", "target").as_deref(),
        Some("first")
    );
    assert_eq!(query_value("/api?target", "target").as_deref(), Some(""));
    assert_eq!(query_value("/api?other=x", "target"), None);
}

#[test]
fn url_decoder_handles_plus_ascii_and_utf8_percent_octets() {
    assert_eq!(decode_url_component("hello+world"), "hello world");
    assert_eq!(decode_url_component("a%2Fb%3Fc"), "a/b?c");
    assert_eq!(decode_url_component("%C3%A5%C3%A4%C3%B6"), "åäö");
    assert_eq!(
        query_value("/api?name=hello+%C3%A5", "name").as_deref(),
        Some("hello å")
    );
}

#[test]
fn url_decoder_is_hex_case_insensitive() {
    assert_eq!(decode_url_component("%2f%2F"), "//");
}

#[test]
fn malformed_percent_escapes_are_preserved_literally() {
    for input in ["%", "%2", "%zz", "a%2zb", "a%b"] {
        assert_eq!(decode_url_component(input), input, "{input}");
    }
}

#[test]
fn invalid_utf8_percent_octets_use_lossy_replacement() {
    assert_eq!(decode_url_component("%FF"), "\u{fffd}");
}

#[test]
fn rollout_action_parser_accepts_exact_two_component_route() {
    assert_eq!(
        parse_runtime_cloud_rollout_action("/api/runtime-cloud/rollouts/roll-17/pause"),
        Some(("roll-17".to_string(), "pause".to_string()))
    );
}

#[test]
fn rollout_action_parser_trims_captured_components() {
    assert_eq!(
        parse_runtime_cloud_rollout_action("/api/runtime-cloud/rollouts/ roll-17 / pause "),
        Some(("roll-17".to_string(), "pause".to_string()))
    );
}

#[test]
fn rollout_action_parser_rejects_empty_extra_or_variant_routes() {
    for path in [
        "/api/runtime-cloud/rollouts/",
        "/api/runtime-cloud/rollouts/id",
        "/api/runtime-cloud/rollouts/id/",
        "/api/runtime-cloud/rollouts//pause",
        "/api/runtime-cloud/rollouts/id/pause/extra",
        "/API/runtime-cloud/rollouts/id/pause",
        "prefix/api/runtime-cloud/rollouts/id/pause",
    ] {
        assert_eq!(parse_runtime_cloud_rollout_action(path), None, "{path}");
    }
}

#[test]
fn probe_success_prefers_plc_name_and_preserves_state() {
    assert_eq!(
        parse_probe_response(r#"{"ok":true,"result":{"plc_name":"Line A","state":"running"}}"#),
        json!({"ok": true, "name": "Line A", "state": "running"})
    );
}

#[test]
fn probe_success_falls_back_to_resource_then_defaults() {
    assert_eq!(
        parse_probe_response(r#"{"ok":true,"result":{"resource":"Main"}}"#),
        json!({"ok": true, "name": "Main", "state": "online"})
    );
    assert_eq!(
        parse_probe_response(r#"{"ok":true}"#),
        json!({"ok": true, "name": "PLC", "state": "online"})
    );
}

#[test]
fn probe_failure_preserves_string_error() {
    assert_eq!(
        parse_probe_response(r#"{"ok":false,"error":"permission denied"}"#),
        json!({"ok": false, "error": "permission denied"})
    );
}

#[test]
fn invalid_or_incomplete_probe_payload_is_unreachable() {
    for text in [
        "",
        "not-json",
        "{}",
        r#"{"ok":false}"#,
        r#"{"ok":"true"}"#,
        r#"{"ok":false,"error":17}"#,
    ] {
        assert_eq!(
            parse_probe_response(text),
            json!({"ok": false, "error": "unreachable"}),
            "{text}"
        );
    }
}

#[test]
fn qr_renderer_returns_complete_svg() {
    let svg = render_qr_svg("trust://pair/example").expect("QR SVG");
    assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.contains("viewBox="));
}

#[test]
fn qr_renderer_rejects_payload_beyond_encoder_capacity() {
    let oversized = "x".repeat(100_000);
    let error = render_qr_svg(&oversized).expect_err("oversized QR payload");
    assert!(error.to_string().contains("qr:"));
}

#[test]
fn wall_clock_helpers_use_millisecond_and_nanosecond_units() {
    let before_ms = now_ms();
    let observed_ns = now_ns();
    let after_ms = now_ms();
    assert!(observed_ns >= (before_ms as u128 * 1_000_000) as u64);
    assert!(observed_ns <= ((after_ms.saturating_add(1)) as u128 * 1_000_000) as u64);
}
