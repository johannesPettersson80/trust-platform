use std::net::SocketAddr;

use serde_json::Value;
use tiny_http::{Header, Method, Request, TestRequest};

use super::*;

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn request(remote: &str, headers: &[(&str, &str)], body: &'static str) -> Request {
    let mut request = TestRequest::new()
        .with_method(Method::Post)
        .with_remote_addr(remote.parse::<SocketAddr>().expect("socket"))
        .with_body(body);
    for (name, value) in headers {
        request = request.with_header(header(name, value));
    }
    request.into()
}

fn response_json(response: Response<std::io::Cursor<Vec<u8>>>) -> Value {
    serde_json::from_slice(response.into_reader().get_ref()).expect("JSON response")
}

fn assert_json_content_type(response: &Response<std::io::Cursor<Vec<u8>>>) {
    assert!(response.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header
                .value
                .as_str()
                .eq_ignore_ascii_case("application/json")
    }));
}

fn accepted_policy(remote: &str, headers: &[(&str, &str)], tls: bool, require_json: bool) -> bool {
    api_post_policy_check(&request(remote, headers, ""), tls, require_json).is_ok()
}

#[test]
fn json_body_at_the_exact_byte_limit_is_parsed() {
    let mut request = request("127.0.0.1:4100", &[], "1234");
    let value: Value = read_json_body(&mut request, 4).expect("exact limit");
    assert_eq!(value, json!(1234));
}

#[test]
fn json_body_one_byte_over_the_limit_is_too_large() {
    let mut request = request("127.0.0.1:4100", &[], "1234");
    assert_eq!(
        read_json_body::<Value>(&mut request, 3),
        Err(JsonBodyError::TooLarge)
    );
}

#[test]
fn malformed_json_is_distinct_from_an_oversized_body() {
    let mut request = request("127.0.0.1:4100", &[], "{nope");
    assert_eq!(
        read_json_body::<Value>(&mut request, 32),
        Err(JsonBodyError::InvalidJson)
    );
}

#[test]
fn empty_json_body_is_invalid_json() {
    let mut request = request("127.0.0.1:4100", &[], "");
    assert_eq!(
        read_json_body::<Value>(&mut request, 32),
        Err(JsonBodyError::InvalidJson)
    );
}

#[test]
fn nested_json_shape_is_preserved() {
    let mut request = request("127.0.0.1:4100", &[], r#"{"a":[true,null,3]}"#);
    let value: Value = read_json_body(&mut request, 64).expect("nested JSON");
    assert_eq!(value, json!({"a": [true, null, 3]}));
}

#[test]
fn maximum_usize_limit_does_not_overflow() {
    let mut request = request("127.0.0.1:4100", &[], "null");
    let value: Value = read_json_body(&mut request, usize::MAX).expect("overflow-safe limit");
    assert_eq!(value, Value::Null);
}

#[test]
fn required_json_content_type_accepts_exact_media_type() {
    assert!(accepted_policy(
        "127.0.0.1:4100",
        &[("Content-Type", "application/json")],
        false,
        true
    ));
}

#[test]
fn required_json_content_type_is_ascii_case_insensitive() {
    assert!(accepted_policy(
        "127.0.0.1:4100",
        &[("content-type", "Application/JSON")],
        false,
        true
    ));
}

#[test]
fn required_json_content_type_accepts_media_type_parameters() {
    assert!(accepted_policy(
        "127.0.0.1:4100",
        &[("Content-Type", "application/json; charset=utf-8")],
        false,
        true
    ));
}

#[test]
fn required_json_content_type_rejects_missing_header() {
    let response = api_post_policy_check(&request("127.0.0.1:4100", &[], ""), false, true)
        .expect_err("missing media type");
    assert_eq!(response.status_code(), StatusCode(415));
    assert_json_content_type(&response);
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "contract_violation",
            "error": "Content-Type must be application/json"
        })
    );
}

#[test]
fn required_json_content_type_rejects_unrelated_media_type() {
    assert!(!accepted_policy(
        "127.0.0.1:4100",
        &[("Content-Type", "text/plain")],
        false,
        true
    ));
}

#[test]
fn required_json_content_type_rejects_prefix_and_suffix_lookalikes() {
    for value in ["application/jsonx", "application/json-patch+json"] {
        let response = api_post_policy_check(
            &request("127.0.0.1:4100", &[("Content-Type", value)], ""),
            false,
            true,
        )
        .expect_err("lookalike media type");
        assert_eq!(response.status_code(), StatusCode(415), "{value}");
    }
}

#[test]
fn optional_json_policy_does_not_require_content_type() {
    assert!(accepted_policy("127.0.0.1:4100", &[], false, false));
}

#[test]
fn missing_origin_is_allowed_for_ipv4_loopback() {
    assert!(accepted_policy(
        "127.0.0.1:4100",
        &[("Content-Type", "application/json")],
        false,
        true
    ));
}

#[test]
fn missing_origin_is_allowed_for_ipv6_loopback() {
    assert!(accepted_policy(
        "[::1]:4100",
        &[("Content-Type", "application/json")],
        false,
        true
    ));
}

#[test]
fn missing_origin_is_denied_for_non_loopback() {
    let response = api_post_policy_check(
        &request(
            "192.0.2.10:4100",
            &[("Content-Type", "application/json")],
            "",
        ),
        false,
        true,
    )
    .expect_err("non-loopback requires Origin");
    assert_eq!(response.status_code(), StatusCode(403));
    assert_json_content_type(&response);
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "permission_denied",
            "error": "missing Origin header"
        })
    );
}

#[test]
fn matching_http_origin_is_allowed() {
    assert!(accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example:8080"),
            ("Host", "runtime.example:8080"),
        ],
        false,
        true
    ));
}

#[test]
fn matching_https_origin_is_allowed_when_tls_is_enabled() {
    assert!(accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "https://runtime.example"),
            ("Host", "runtime.example"),
        ],
        true,
        true
    ));
}

#[test]
fn origin_comparison_normalizes_ascii_case_and_one_trailing_slash() {
    assert!(accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "HTTP://RUNTIME.EXAMPLE:8080/"),
            ("Host", "runtime.example:8080"),
        ],
        false,
        true
    ));
}

#[test]
fn opaque_null_origin_is_denied() {
    let response = api_post_policy_check(
        &request(
            "127.0.0.1:4100",
            &[
                ("Content-Type", "application/json"),
                ("Origin", "null"),
                ("Host", "runtime.example"),
            ],
            "",
        ),
        false,
        true,
    )
    .expect_err("opaque origin");
    assert_eq!(response.status_code(), StatusCode(403));
    assert_eq!(response_json(response)["denial_code"], "permission_denied");
}

#[test]
fn supplied_origin_requires_host() {
    let response = api_post_policy_check(
        &request(
            "127.0.0.1:4100",
            &[
                ("Content-Type", "application/json"),
                ("Origin", "http://runtime.example"),
            ],
            "",
        ),
        false,
        true,
    )
    .expect_err("missing Host");
    assert_eq!(response.status_code(), StatusCode(400));
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "contract_violation",
            "error": "missing Host header"
        })
    );
}

#[test]
fn mismatched_host_is_denied() {
    assert!(!accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://attacker.example"),
            ("Host", "runtime.example"),
        ],
        false,
        true
    ));
}

#[test]
fn mismatched_scheme_is_denied() {
    assert!(!accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "https://runtime.example"),
            ("Host", "runtime.example"),
        ],
        false,
        true
    ));
}

#[test]
fn mismatched_port_is_denied() {
    assert!(!accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example:9000"),
            ("Host", "runtime.example:8080"),
        ],
        false,
        true
    ));
}

#[test]
fn origin_prefix_attack_is_denied() {
    assert!(!accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example.attacker.invalid"),
            ("Host", "runtime.example"),
        ],
        false,
        true
    ));
}

#[test]
fn origin_with_a_path_is_denied() {
    assert!(!accepted_policy(
        "192.0.2.10:4100",
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example/control"),
            ("Host", "runtime.example"),
        ],
        false,
        true
    ));
}

#[test]
fn invalid_body_response_has_stable_contract() {
    let response = json_body_error_response(JsonBodyError::InvalidBody);
    assert_eq!(response.status_code(), StatusCode(400));
    assert_json_content_type(&response);
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "contract_violation",
            "error": "invalid body"
        })
    );
}

#[test]
fn oversized_body_response_has_stable_contract() {
    let response = json_body_error_response(JsonBodyError::TooLarge);
    assert_eq!(response.status_code(), StatusCode(413));
    assert_json_content_type(&response);
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "contract_violation",
            "error": "request body exceeds maximum size"
        })
    );
}

#[test]
fn invalid_json_response_has_stable_contract() {
    let response = json_body_error_response(JsonBodyError::InvalidJson);
    assert_eq!(response.status_code(), StatusCode(400));
    assert_json_content_type(&response);
    assert_eq!(
        response_json(response),
        json!({
            "ok": false,
            "denial_code": "contract_violation",
            "error": "invalid json"
        })
    );
}
