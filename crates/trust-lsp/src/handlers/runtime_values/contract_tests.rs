use super::*;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

enum ScriptResponse {
    Json(Value),
    Raw(&'static str),
    Close,
}

fn start_server(responses: Vec<ScriptResponse>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind scripted control server");
    let address = listener.local_addr().expect("scripted server address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control client");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set scripted server timeout");
        let reader_stream = stream.try_clone().expect("clone scripted stream");
        let mut reader = BufReader::new(reader_stream);
        let mut requests = Vec::new();
        for response in responses {
            let mut request = String::new();
            match reader.read_line(&mut request) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            requests.push(serde_json::from_str(&request).expect("valid control request"));
            match response {
                ScriptResponse::Json(value) => {
                    writeln!(stream, "{value}").expect("write scripted JSON response");
                    stream.flush().expect("flush scripted JSON response");
                }
                ScriptResponse::Raw(line) => {
                    writeln!(stream, "{line}").expect("write scripted raw response");
                    stream.flush().expect("flush scripted raw response");
                }
                ScriptResponse::Close => break,
            }
        }
        requests
    });
    (format!("tcp://{address}"), handle)
}

fn ok(result: Value) -> ScriptResponse {
    ScriptResponse::Json(json!({ "ok": true, "result": result }))
}

fn failed() -> ScriptResponse {
    ScriptResponse::Json(json!({ "ok": false, "result": null }))
}

fn scopes(entries: &[(&str, u32)]) -> ScriptResponse {
    ok(json!({
        "scopes": entries
            .iter()
            .map(|(name, reference)| {
                json!({ "name": name, "variablesReference": reference })
            })
            .collect::<Vec<_>>()
    }))
}

fn variables(entries: &[(&str, &str)]) -> ScriptResponse {
    ok(json!({
        "variables": entries
            .iter()
            .map(|(name, value)| json!({ "name": name, "value": value }))
            .collect::<Vec<_>>()
    }))
}

fn instances(entries: &[(&str, u32)]) -> ScriptResponse {
    ok(json!({
        "variables": entries
            .iter()
            .map(|(name, reference)| {
                json!({ "name": name, "variablesReference": reference })
            })
            .collect::<Vec<_>>()
    }))
}

fn fetch(
    responses: Vec<ScriptResponse>,
    auth: Option<&str>,
    frame_id: u32,
    hints: &[&str],
) -> (Option<RuntimeInlineValues>, Vec<Value>) {
    let (endpoint, handle) = start_server(responses);
    let hints = hints
        .iter()
        .map(|hint| SmolStr::new(*hint))
        .collect::<Vec<_>>();
    let values = fetch_runtime_inline_values(&endpoint, auth, frame_id, &hints);
    let requests = handle.join().expect("join scripted control server");
    (values, requests)
}

fn tcp_address(endpoint: ControlEndpoint) -> String {
    match endpoint {
        ControlEndpoint::Tcp(address) => address,
        #[cfg(unix)]
        ControlEndpoint::Unix(_) => panic!("expected TCP endpoint"),
    }
}

#[test]
fn tcp_endpoint_requires_exact_scheme_and_nonempty_address() {
    assert_eq!(
        tcp_address(ControlEndpoint::parse("tcp://127.0.0.1:9000").unwrap()),
        "127.0.0.1:9000"
    );
    assert!(ControlEndpoint::parse("TCP://127.0.0.1:9000").is_none());
    assert!(ControlEndpoint::parse(" tcp://127.0.0.1:9000").is_none());
    assert!(ControlEndpoint::parse("tcp://").is_none());
}

#[cfg(unix)]
#[test]
fn unix_endpoint_requires_nonempty_absolute_path() {
    match ControlEndpoint::parse("unix:///tmp/trust-runtime.sock").unwrap() {
        ControlEndpoint::Unix(path) => {
            assert_eq!(path, std::path::PathBuf::from("/tmp/trust-runtime.sock"));
        }
        ControlEndpoint::Tcp(_) => panic!("expected Unix endpoint"),
    }
    assert!(ControlEndpoint::parse("unix://").is_none());
    assert!(ControlEndpoint::parse("unix://relative.sock").is_none());
}

#[test]
fn numeric_socket_addresses_resolve_without_rewriting() {
    assert_eq!(
        resolve_socket_addr("127.0.0.1:9000"),
        Some("127.0.0.1:9000".parse().unwrap())
    );
    assert_eq!(
        resolve_socket_addr("[::1]:9000"),
        Some("[::1]:9000".parse().unwrap())
    );
    assert!(resolve_socket_addr("127.0.0.1").is_none());
}

#[test]
fn scopes_request_has_first_id_exact_type_frame_and_auth() {
    let (values, requests) = fetch(vec![scopes(&[])], Some("token"), 42, &[]);
    assert!(values.is_some());
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0],
        json!({
            "id": 1,
            "type": "debug.scopes",
            "params": { "frame_id": 42 },
            "auth": "token"
        })
    );
}

#[test]
fn absent_auth_is_omitted_from_every_request() {
    let (values, requests) = fetch(
        vec![scopes(&[("locals", 10)]), variables(&[("x", "1")])],
        None,
        1,
        &[],
    );
    assert!(values.is_some());
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.get("auth").is_none()));
}

#[test]
fn blank_auth_is_omitted_instead_of_sent_as_credential() {
    let (values, requests) = fetch(vec![scopes(&[])], Some("   "), 1, &[]);
    assert!(values.is_some());
    assert!(requests[0].get("auth").is_none());
}

#[test]
fn request_ids_increment_and_variable_parameters_are_exact() {
    let (_, requests) = fetch(
        vec![
            scopes(&[("locals", 10), ("globals", 20), ("retain", 30)]),
            variables(&[]),
            variables(&[]),
            variables(&[]),
        ],
        None,
        7,
        &[],
    );
    assert_eq!(
        requests
            .iter()
            .map(|request| request["id"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(requests[1]["type"], "debug.variables");
    assert_eq!(requests[1]["params"], json!({ "variables_reference": 10 }));
    assert_eq!(requests[2]["params"], json!({ "variables_reference": 20 }));
    assert_eq!(requests[3]["params"], json!({ "variables_reference": 30 }));
}

#[test]
fn locals_globals_and_retain_are_kept_separate() {
    let (values, _) = fetch(
        vec![
            scopes(&[("locals", 10), ("globals", 20), ("retain", 30)]),
            variables(&[("local", "1")]),
            variables(&[("global", "2")]),
            variables(&[("saved", "3")]),
        ],
        None,
        1,
        &[],
    );
    let values = values.expect("runtime values");
    assert_eq!(values.locals.get("local").map(String::as_str), Some("1"));
    assert_eq!(values.globals.get("global").map(String::as_str), Some("2"));
    assert_eq!(values.retain.get("saved").map(String::as_str), Some("3"));
    assert!(!values.locals.contains_key("global"));
}

#[test]
fn known_scope_names_are_trimmed_and_case_insensitive() {
    let (values, _) = fetch(
        vec![
            scopes(&[(" LoCaLs ", 10), (" GLOBALS ", 20), ("retain", 30)]),
            variables(&[("local", "1")]),
            variables(&[("global", "2")]),
            variables(&[("saved", "3")]),
        ],
        None,
        1,
        &[],
    );
    let values = values.expect("runtime values");
    assert_eq!(values.locals.len(), 1);
    assert_eq!(values.globals.len(), 1);
    assert_eq!(values.retain.len(), 1);
}

#[test]
fn unknown_scopes_are_ignored_without_extra_requests() {
    let (values, requests) = fetch(vec![scopes(&[("registers", 99)])], None, 1, &[]);
    let values = values.expect("runtime values");
    assert!(values.locals.is_empty());
    assert!(values.globals.is_empty());
    assert!(values.retain.is_empty());
    assert_eq!(requests.len(), 1);
}

#[test]
fn duplicate_known_scope_is_ambiguous_and_rejects_snapshot() {
    let (values, requests) = fetch(
        vec![scopes(&[("locals", 10), ("LOCALS", 11)])],
        None,
        1,
        &[],
    );
    assert!(values.is_none());
    assert_eq!(requests.len(), 1);
}

#[test]
fn zero_known_scope_reference_rejects_snapshot() {
    let (values, requests) = fetch(vec![scopes(&[("locals", 0)])], None, 1, &[]);
    assert!(values.is_none());
    assert_eq!(requests.len(), 1);
}

#[test]
fn advertised_scope_failure_rejects_entire_snapshot() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("locals", 10), ("globals", 20)]),
            variables(&[("local", "1")]),
            failed(),
        ],
        None,
        1,
        &[],
    );
    assert!(values.is_none());
    assert_eq!(requests.len(), 3);
}

#[test]
fn malformed_variables_result_rejects_entire_snapshot() {
    let (values, _) = fetch(
        vec![
            scopes(&[("locals", 10)]),
            ok(json!({ "variables": [{ "name": "x", "value": 1 }] })),
        ],
        None,
        1,
        &[],
    );
    assert!(values.is_none());
}

#[test]
fn variable_names_are_trimmed_blank_dropped_and_first_duplicate_wins() {
    let (values, _) = fetch(
        vec![
            scopes(&[("locals", 10)]),
            variables(&[(" x ", "first"), ("", "blank"), ("x", "second")]),
        ],
        None,
        1,
        &[],
    );
    let locals = values.expect("runtime values").locals;
    assert_eq!(locals.len(), 1);
    assert_eq!(locals.get("x").map(String::as_str), Some("first"));
}

#[test]
fn exact_qualified_instance_prefix_wins_by_hint_order() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("Other#1", 41), ("Pkg.Controller#7", 42)]),
            variables(&[("speed", "12")]),
        ],
        None,
        1,
        &["Pkg.Controller", "Other"],
    );
    assert_eq!(
        values
            .expect("runtime values")
            .locals
            .get("speed")
            .map(String::as_str),
        Some("12")
    );
    assert_eq!(requests[2]["params"], json!({ "variables_reference": 42 }));
}

#[test]
fn exact_instance_type_name_match_is_case_insensitive() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("CONTROLLER", 41), ("Other#1", 42)]),
            variables(&[("speed", "12")]),
        ],
        None,
        1,
        &["controller"],
    );
    assert!(values.expect("runtime values").locals.contains_key("speed"));
    assert_eq!(requests[2]["params"], json!({ "variables_reference": 41 }));
}

#[test]
fn unique_unqualified_base_name_selects_qualified_instance() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("Vendor.Controller#1", 41), ("Vendor.Other#1", 42)]),
            variables(&[("speed", "12")]),
        ],
        None,
        1,
        &["App.Controller"],
    );
    assert!(values.expect("runtime values").locals.contains_key("speed"));
    assert_eq!(requests[2]["params"], json!({ "variables_reference": 41 }));
}

#[test]
fn ambiguous_unqualified_base_name_selects_no_instance() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("VendorA.Controller#1", 41), ("VendorB.Controller#2", 42)]),
        ],
        None,
        1,
        &["App.Controller"],
    );
    assert!(values.expect("runtime values").locals.is_empty());
    assert_eq!(requests.len(), 2);
}

#[test]
fn sole_instance_is_fallback_when_no_hint_matches() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("Only#1", 41)]),
            variables(&[("speed", "12")]),
        ],
        None,
        1,
        &["Unmatched"],
    );
    assert!(values.expect("runtime values").locals.contains_key("speed"));
    assert_eq!(requests[2]["params"], json!({ "variables_reference": 41 }));
}

#[test]
fn multiple_unmatched_instances_are_ignored() {
    let (values, requests) = fetch(
        vec![
            scopes(&[("instances", 40)]),
            instances(&[("One#1", 41), ("Two#1", 42)]),
        ],
        None,
        1,
        &["Unmatched"],
    );
    assert!(values.expect("runtime values").locals.is_empty());
    assert_eq!(requests.len(), 2);
}

#[test]
fn explicit_locals_win_when_instance_values_are_merged() {
    let (values, _) = fetch(
        vec![
            scopes(&[("locals", 10), ("instances", 40)]),
            variables(&[("shared", "local"), ("local_only", "1")]),
            instances(&[("Controller#1", 41)]),
            variables(&[("shared", "instance"), ("instance_only", "2")]),
        ],
        None,
        1,
        &["Controller"],
    );
    let locals = values.expect("runtime values").locals;
    assert_eq!(locals.get("shared").map(String::as_str), Some("local"));
    assert_eq!(locals.get("local_only").map(String::as_str), Some("1"));
    assert_eq!(locals.get("instance_only").map(String::as_str), Some("2"));
}

#[test]
fn failed_scopes_response_rejects_snapshot() {
    let (values, _) = fetch(vec![failed()], None, 1, &[]);
    assert!(values.is_none());
}

#[test]
fn missing_response_result_rejects_snapshot() {
    let (values, _) = fetch(
        vec![ScriptResponse::Json(json!({ "ok": true }))],
        None,
        1,
        &[],
    );
    assert!(values.is_none());
}

#[test]
fn malformed_response_rejects_snapshot() {
    let (values, _) = fetch(vec![ScriptResponse::Raw("{not-json")], None, 1, &[]);
    assert!(values.is_none());
}

#[test]
fn closed_response_stream_rejects_snapshot() {
    let (values, _) = fetch(vec![ScriptResponse::Close], None, 1, &[]);
    assert!(values.is_none());
}

#[test]
fn malformed_scopes_shape_rejects_snapshot() {
    let (values, _) = fetch(
        vec![ok(json!({ "scopes": [{ "name": "locals" }] }))],
        None,
        1,
        &[],
    );
    assert!(values.is_none());
}
