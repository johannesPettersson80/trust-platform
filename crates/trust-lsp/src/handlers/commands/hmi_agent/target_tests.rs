//! The simulator target check (specification 43, AT-08). Port of the target assertions of
//! `editors/vscode/src/test/suite/hmi-agent-completion.test.ts` ("simulator journeys refuse physical
//! mode and nonlocal endpoints without fallback"), with the local forms they leave out.

use serde_json::{json, Value};

use super::{check_target, is_local_simulator};

/// A target in simulation, or not, with a web address and a control endpoint.
fn target(simulator: Value, url: &str, endpoint: &str) -> Value {
    json!({"simulator": simulator, "url": url, "endpoint": endpoint, "label": "Fixture"})
}

/// Catches: a local simulator being refused, which would leave the agent without simulator
/// journeys.
#[test]
fn a_local_simulator_is_accepted() {
    for (url, endpoint) in [
        ("http://127.0.0.1:8080", "tcp://127.0.0.1:9100"),
        ("http://localhost:8080", "tcp://localhost:9100"),
        ("http://[::1]:8080", "tcp://[::1]:9100"),
        (
            "http://LOCALHOST:8080/hmi",
            "unix:///run/trust/control.sock",
        ),
        ("https://127.0.0.1", r"npipe:\\.\pipe\trust-control"),
    ] {
        assert!(
            is_local_simulator(&target(json!(true), url, endpoint)),
            "{url} {endpoint}"
        );
    }
}

/// Catches: a physical runtime being driven by an agent journey because it listens on this
/// computer.
#[test]
fn a_runtime_that_is_not_a_simulator_is_refused() {
    for simulator in [json!(false), json!("true"), json!(1), Value::Null] {
        assert!(
            !is_local_simulator(&target(
                simulator.clone(),
                "http://127.0.0.1:8080",
                "tcp://127.0.0.1:9100"
            )),
            "{simulator}"
        );
    }
    assert!(!is_local_simulator(
        &json!({"url": "http://127.0.0.1:8080", "endpoint": "tcp://127.0.0.1:9100"})
    ));
}

/// Catches: a remote or forwarded runtime being driven as if it were local, by its web address or
/// by its control endpoint.
#[test]
fn a_remote_web_address_or_control_endpoint_is_refused() {
    for (url, endpoint) in [
        ("http://192.168.1.2:8080", "tcp://192.168.1.2:9100"),
        ("http://127.0.0.1:8080", "tcp://192.168.1.2:9100"),
        ("http://192.168.1.2:8080", "tcp://127.0.0.1:9100"),
        ("http://127.0.0.1.example.com:8080", "tcp://127.0.0.1:9100"),
        ("http://127.0.0.1@example.com:8080", "tcp://127.0.0.1:9100"),
        ("http://127.0.0.1:8080", "tcp://127.0.0.1.example.com:9100"),
        ("http://127.0.0.1:8080", "tcp://localhost.example.com:9100"),
        ("http://127.0.0.1:8080", "tcp://127.0.0.1"),
        ("http://127.0.0.1:8080", "TCP://127.0.0.1:9100"),
        ("http://127.0.0.1:8080", "ssh://127.0.0.1:9100"),
        ("http://127.0.0.1:8080", ""),
        ("not a url", "tcp://127.0.0.1:9100"),
        ("", "tcp://127.0.0.1:9100"),
    ] {
        assert!(
            !is_local_simulator(&target(json!(true), url, endpoint)),
            "{url} {endpoint}"
        );
    }
}

/// Catches: the command answering in another form, or accepting a missing target.
#[test]
fn the_command_answers_whether_its_argument_is_a_local_simulator() {
    let local = target(json!(true), "http://127.0.0.1:8080", "tcp://127.0.0.1:9100");
    assert_eq!(check_target(&[local]), json!({"local": true}));
    let remote = target(
        json!(true),
        "http://192.168.1.2:8080",
        "tcp://192.168.1.2:9100",
    );
    assert_eq!(check_target(&[remote]), json!({"local": false}));
    assert_eq!(check_target(&[]), json!({"local": false}));
    assert_eq!(check_target(&[Value::Null]), json!({"local": false}));
    assert_eq!(
        check_target(&[json!("tcp://127.0.0.1:9100")]),
        json!({"local": false})
    );
}
