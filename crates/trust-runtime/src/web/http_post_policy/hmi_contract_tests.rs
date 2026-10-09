//! Specification 39, §21.5 item 3: the cross-site check of the HMI write routes. It is the policy
//! of the other write routes with a JSON body required, the connection's own scheme, any client
//! allowed without `Origin`, and every refusal answered 403.

use std::net::SocketAddr;

use serde_json::Value;
use tiny_http::{Header, Method, Request, TestRequest};

use super::*;

fn request(remote: &str, https: bool, headers: &[(&str, &str)]) -> Request {
    let mut request = TestRequest::new()
        .with_method(Method::Post)
        .with_remote_addr(remote.parse::<SocketAddr>().expect("socket"))
        .with_body("{}");
    if https {
        request = request.with_https();
    }
    for (name, value) in headers {
        request = request.with_header(
            Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header"),
        );
    }
    request.into()
}

/// The refusal's status and JSON body.
fn refusal(remote: &str, https: bool, headers: &[(&str, &str)]) -> (StatusCode, Value) {
    let response = hmi_post_policy_check(&request(remote, https, headers)).expect_err("a refusal");
    let status = response.status_code();
    let body = serde_json::from_slice(response.into_reader().get_ref()).expect("JSON refusal");
    (status, body)
}

fn admitted(remote: &str, https: bool, headers: &[(&str, &str)]) -> bool {
    hmi_post_policy_check(&request(remote, https, headers)).is_ok()
}

#[test]
fn a_same_origin_json_request_is_admitted() {
    assert!(admitted(
        "192.0.2.10:4100",
        false,
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example:8080"),
            ("Host", "runtime.example:8080"),
        ],
    ));
    assert!(admitted(
        "192.0.2.10:4100",
        true,
        &[
            ("Content-Type", "application/json; charset=utf-8"),
            ("Origin", "https://runtime.example"),
            ("Host", "runtime.example"),
        ],
    ));
}

#[test]
fn a_json_request_without_origin_is_admitted_from_any_client() {
    // The editor's Live mode and Deploy send no Origin, also to a runtime on another machine.
    assert!(admitted(
        "192.0.2.10:4100",
        false,
        &[("Content-Type", "application/json")],
    ));
    assert!(admitted(
        "127.0.0.1:4100",
        false,
        &[("Content-Type", "application/json")],
    ));
}

#[test]
fn a_cross_origin_request_is_refused_with_403() {
    for (https, origin) in [
        (false, "http://attacker.example"),
        (false, "https://runtime.example"),
        (true, "http://runtime.example"),
        (false, "http://runtime.example.attacker.invalid"),
        (false, "null"),
    ] {
        let (status, body) = refusal(
            "192.0.2.10:4100",
            https,
            &[
                ("Content-Type", "application/json"),
                ("Origin", origin),
                ("Host", "runtime.example"),
            ],
        );
        assert_eq!(status, StatusCode(403), "{origin}: {body}");
        assert_eq!(body["ok"], json!(false), "{origin}: {body}");
        assert_eq!(body["denial_code"], json!("permission_denied"), "{body}");
    }
}

#[test]
fn a_request_that_is_not_json_is_refused_with_403() {
    for headers in [
        vec![("Content-Type", "text/plain")],
        vec![("Content-Type", "application/x-www-form-urlencoded")],
        vec![("Content-Type", "multipart/form-data; boundary=x")],
        vec![("Content-Type", "application/jsonx")],
        vec![],
    ] {
        let (status, body) = refusal("127.0.0.1:4100", false, &headers);
        assert_eq!(status, StatusCode(403), "{headers:?}: {body}");
        assert_eq!(
            body["error"],
            json!("Content-Type must be application/json"),
            "{body}"
        );
    }
}

#[test]
fn an_origin_without_a_host_to_compare_is_refused_with_403() {
    let (status, _) = refusal(
        "127.0.0.1:4100",
        false,
        &[
            ("Content-Type", "application/json"),
            ("Origin", "http://runtime.example"),
        ],
    );
    assert_eq!(status, StatusCode(403));
}

#[test]
fn the_other_write_routes_keep_their_policy() {
    // Only the HMI routes admit a client without Origin and answer every refusal 403.
    let missing_origin = api_post_policy_check(
        &request(
            "192.0.2.10:4100",
            false,
            &[("Content-Type", "application/json")],
        ),
        false,
        true,
    )
    .expect_err("a non-loopback client needs Origin");
    assert_eq!(missing_origin.status_code(), StatusCode(403));
    let not_json = api_post_policy_check(
        &request("127.0.0.1:4100", false, &[("Content-Type", "text/plain")]),
        false,
        true,
    )
    .expect_err("not JSON");
    assert_eq!(not_json.status_code(), StatusCode(415));
}
