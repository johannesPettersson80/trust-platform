use std::io::Write;
use std::process::{Command, ExitStatus, Stdio};

use serde_json::{json, Value as JsonValue};

fn timer_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    ton_in : BOOL;
    ton_fb : TON;
    q : BOOL;
    et : TIME;
END_VAR
ton_fb(IN := ton_in, PT := T#100MS, Q => q, ET => et);
END_PROGRAM
"#
}

fn latch_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    start : BOOL;
    latched : BOOL;
END_VAR
IF start THEN
    latched := TRUE;
END_IF;
END_PROGRAM
"#
}

fn retained_program(initial: i16) -> String {
    format!(
        r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    counter : INT := INT#{initial};
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

PROGRAM Main
END_PROGRAM
"#
    )
}

fn access_and_direct_io_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    input : BOOL;
    output : BOOL;
    accessible : INT;
END_VAR
output := input;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_ACCESS
    RemoteValue : P1.accessible : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#
}

fn runtime_cycle_error_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
result := 1 / 0;
END_PROGRAM
"#
}

fn run_harness(requests: &[JsonValue]) -> (Vec<JsonValue>, String) {
    run_harness_with_args(&[], requests)
}

fn run_harness_with_args(args: &[&str], requests: &[JsonValue]) -> (Vec<JsonValue>, String) {
    let lines = requests
        .iter()
        .map(|request| serde_json::to_string(request).expect("encode request"))
        .collect::<Vec<_>>();
    let (status, responses, stderr) = run_harness_lines(args, &[], &lines);
    assert!(
        status.success(),
        "expected trust-harness success, stderr was:\n{stderr}"
    );
    (responses, stderr)
}

fn run_harness_lines(
    args: &[&str],
    envs: &[(&str, &str)],
    lines: &[String],
) -> (ExitStatus, Vec<JsonValue>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-harness"))
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-harness");

    let mut stdin = child.stdin.take().expect("harness stdin");
    for line in lines {
        writeln!(stdin, "{line}").expect("write request");
    }
    drop(stdin);

    let output = child.wait_with_output().expect("wait for trust-harness");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let responses = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<JsonValue>(line).expect("decode response"))
        .collect();
    (
        output.status,
        responses,
        String::from_utf8(output.stderr).expect("stderr utf-8"),
    )
}

#[test]
fn trust_harness_cycle_dt_ms_advances_virtual_time() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": timer_program(),
        }),
        json!({
            "cmd": "set_input",
            "name": "ton_in",
            "value": { "type": "BOOL", "value": true },
        }),
        json!({
            "cmd": "cycle",
            "count": 10,
            "dt_ms": 10,
            "watch": ["q", "et"],
        }),
    ]);

    assert_eq!(responses.len(), 3, "stderr was:\n{stderr}");
    assert_eq!(responses[0]["ok"], json!(true));
    assert_eq!(responses[0]["protocol_version"], json!(2));
    assert_eq!(responses[1]["ok"], json!(true));
    assert_eq!(responses[2]["ok"], json!(true));
    assert_eq!(
        responses[2]["data"]["values"]["q"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": true}})
    );
    assert_eq!(
        responses[2]["data"]["values"]["et"],
        json!({"status": "ok", "value": {"type": "TIME", "nanos": 100_000_000}})
    );
}

#[test]
fn trust_harness_set_input_then_get_output_roundtrips() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": latch_program(),
        }),
        json!({
            "cmd": "set_input",
            "name": "start",
            "value": { "type": "BOOL", "value": true },
        }),
        json!({
            "cmd": "cycle",
            "count": 1,
            "watch": ["latched"],
        }),
        json!({
            "cmd": "get_output",
            "name": "latched",
        }),
    ]);

    assert_eq!(responses.len(), 4, "stderr was:\n{stderr}");
    assert_eq!(
        responses[2]["data"]["values"]["latched"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": true}})
    );
    assert_eq!(
        responses[3]["data"]["value"],
        json!({"type": "BOOL", "value": true})
    );
}

#[test]
fn trust_harness_protocol_version_1_keeps_legacy_watch_shape() {
    let (responses, stderr) = run_harness_with_args(
        &["--protocol-version", "1"],
        &[
            json!({
                "cmd": "load",
                "source": latch_program(),
            }),
            json!({
                "cmd": "cycle",
                "count": 1,
                "watch": ["latched"],
            }),
        ],
    );

    assert_eq!(responses.len(), 2, "stderr was:\n{stderr}");
    assert_eq!(responses[1]["protocol_version"], json!(1));
    assert_eq!(
        responses[1]["data"]["values"]["latched"],
        json!({"type": "BOOL", "value": false})
    );
}

#[test]
fn trust_harness_advance_time_then_cycle_exposes_timer_progress() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": timer_program(),
        }),
        json!({
            "cmd": "set_input",
            "name": "ton_in",
            "value": { "type": "BOOL", "value": true },
        }),
        json!({
            "cmd": "advance_time",
            "duration_ms": 25,
        }),
        json!({
            "cmd": "cycle",
            "watch": ["q", "et"],
        }),
    ]);

    assert_eq!(responses.len(), 4, "stderr was:\n{stderr}");
    assert_eq!(responses[2]["data"]["elapsed_ms"], json!(25));
    assert_eq!(
        responses[3]["data"]["values"]["q"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": false}})
    );
    assert_eq!(
        responses[3]["data"]["values"]["et"],
        json!({"status": "ok", "value": {"type": "TIME", "nanos": 25_000_000}})
    );
}

#[test]
fn trust_harness_run_until_supports_success_and_bounded_timeout() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": timer_program(),
        }),
        json!({
            "cmd": "set_input",
            "name": "ton_in",
            "value": { "type": "BOOL", "value": true },
        }),
        json!({
            "cmd": "run_until",
            "name": "q",
            "equals": { "type": "BOOL", "value": true },
            "dt_ms": 25,
            "max_cycles": 5,
            "watch": ["q", "et"],
        }),
        json!({
            "cmd": "run_until",
            "name": "q",
            "equals": { "type": "BOOL", "value": false },
            "max_cycles": 2,
        }),
    ]);

    assert_eq!(responses.len(), 4, "stderr was:\n{stderr}");
    assert_eq!(responses[2]["ok"], json!(true));
    assert_eq!(responses[2]["data"]["cycles_ran"], json!(4));
    assert_eq!(
        responses[2]["data"]["matched_value"],
        json!({"type": "BOOL", "value": true})
    );
    assert_eq!(responses[3]["ok"], json!(false));
    assert_eq!(responses[3]["error"]["kind"], json!("run_until_timeout"));
    assert_eq!(responses[3]["error"]["data"]["max_cycles"], json!(2));
}

#[test]
fn trust_harness_reload_preserves_retain_state() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": retained_program(1),
        }),
        json!({
            "cmd": "set_input",
            "name": "counter",
            "value": { "type": "INT", "value": 7 },
        }),
        json!({
            "cmd": "get_output",
            "name": "counter",
        }),
        json!({
            "cmd": "reload",
            "source": retained_program(99),
        }),
        json!({
            "cmd": "get_output",
            "name": "counter",
        }),
    ]);

    assert_eq!(responses.len(), 5, "stderr was:\n{stderr}");
    assert_eq!(
        responses[2]["data"]["value"],
        json!({"type": "INT", "value": 7})
    );
    assert_eq!(responses[3]["ok"], json!(true));
    assert_eq!(
        responses[4]["data"]["value"],
        json!({"type": "INT", "value": 7})
    );
}

#[test]
fn trust_harness_failed_load_and_reload_preserve_the_live_session() {
    let (responses, stderr) = run_harness(&[
        json!({"cmd": "load", "source": latch_program()}),
        json!({
            "cmd": "set_input",
            "name": "start",
            "value": {"type": "BOOL", "value": true},
        }),
        json!({"cmd": "cycle"}),
        json!({
            "cmd": "load",
            "source": timer_program(),
            "sources": [],
        }),
        json!({"cmd": "get_output", "name": "latched"}),
        json!({
            "cmd": "load",
            "source": timer_program(),
            "sources": ["THIS IS NOT STRUCTURED TEXT"],
        }),
        json!({"cmd": "get_output", "name": "latched"}),
        json!({"cmd": "reload", "source": "THIS IS NOT STRUCTURED TEXT"}),
        json!({"cmd": "get_output", "name": "latched"}),
    ]);

    assert_eq!(responses.len(), 9, "stderr was:\n{stderr}");
    assert_eq!(responses[3]["error"]["kind"], json!("invalid_argument"));
    assert_eq!(responses[5]["error"]["kind"], json!("compile_error"));
    assert_eq!(responses[7]["error"]["kind"], json!("compile_error"));
    for index in [4, 6, 8] {
        assert_eq!(
            responses[index]["data"]["value"],
            json!({"type": "BOOL", "value": true}),
            "failed source replacement changed the live session at response {index}"
        );
    }
}

#[test]
fn trust_harness_time_alias_precedence_and_prechecked_run_until_are_deterministic() {
    let (responses, stderr) = run_harness(&[
        json!({"cmd": "load", "source": latch_program()}),
        json!({"cmd": "advance_time", "duration_ms": 5, "dt_ms": 99}),
        json!({
            "cmd": "run_until",
            "name": "latched",
            "equals": {"type": "BOOL", "value": false},
            "dt_ms": 10,
            "max_cycles": 0,
            "watch": ["latched"],
        }),
        json!({"cmd": "advance_time", "dt_ms": 3}),
        json!({"cmd": "snapshot", "watch": ["latched"]}),
    ]);

    assert_eq!(responses.len(), 5, "stderr was:\n{stderr}");
    assert_eq!(responses[1]["data"]["cycle_count"], json!(1));
    assert_eq!(responses[1]["data"]["elapsed_ms"], json!(5));
    assert_eq!(responses[2]["data"]["cycles_ran"], json!(0));
    assert_eq!(responses[2]["data"]["cycle_count"], json!(1));
    assert_eq!(responses[2]["data"]["elapsed_ms"], json!(5));
    assert_eq!(responses[3]["data"]["cycle_count"], json!(1));
    assert_eq!(responses[3]["data"]["elapsed_ms"], json!(8));
    assert_eq!(responses[4]["data"]["cycle_count"], json!(1));
    assert_eq!(responses[4]["data"]["elapsed_ms"], json!(8));
}

#[test]
fn trust_harness_error_kinds_separate_structure_values_runtime_and_boundaries() {
    let (responses, stderr) = run_harness(&[
        json!({"cmd": "load", "source": latch_program()}),
        json!({"cmd": "set_input", "value": {"type": "BOOL", "value": true}}),
        json!({
            "cmd": "set_input",
            "name": "start",
            "value": {"type": "BOOL", "value": 1},
        }),
        json!({"cmd": "restart", "mode": "invalid"}),
        json!({
            "cmd": "set_direct_input",
            "address": "not-an-io-address",
            "value": {"type": "BOOL", "value": true},
        }),
        json!({"cmd": "get_output", "name": "missing"}),
    ]);

    assert_eq!(responses.len(), 6, "stderr was:\n{stderr}");
    let kinds = responses[1..]
        .iter()
        .map(|response| response["error"]["kind"].as_str().expect("error kind"))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "invalid_request",
            "invalid_argument",
            "invalid_request",
            "runtime_error",
            "unresolved_name",
        ]
    );
    assert!(responses
        .iter()
        .all(|response| response["protocol_version"] == json!(2)));
}

#[test]
fn trust_harness_rejects_negative_dt_ms() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": timer_program(),
        }),
        json!({
            "cmd": "cycle",
            "count": 1,
            "dt_ms": -1,
        }),
    ]);

    assert_eq!(responses.len(), 2, "stderr was:\n{stderr}");
    assert_eq!(responses[0]["ok"], json!(true));
    assert_eq!(responses[1]["ok"], json!(false));
    assert_eq!(responses[1]["error"]["kind"], json!("invalid_argument"));
    assert!(
        responses[1]["error"]["message"]
            .as_str()
            .expect("error string")
            .contains("dt_ms"),
        "expected dt_ms error, got: {}",
        responses[1]
    );
}

#[test]
fn trust_harness_reports_line_errors_and_continues_the_session() {
    let lines = vec![
        "{not json".to_string(),
        json!({"cmd": "cycle"}).to_string(),
        json!({"cmd": "unsupported"}).to_string(),
        json!({"cmd": "load", "source": "THIS IS NOT STRUCTURED TEXT"}).to_string(),
        json!({"cmd": "load", "source": runtime_cycle_error_program()}).to_string(),
        json!({"cmd": "load", "source": latch_program()}).to_string(),
        json!({"cmd": "set_input", "name": "start"}).to_string(),
        json!({"cmd": "snapshot", "watch": ["latched", "missing"]}).to_string(),
    ];
    let (status, responses, stderr) = run_harness_lines(&[], &[], &lines);

    assert!(status.success(), "stderr was:\n{stderr}");
    assert_eq!(responses.len(), lines.len(), "stderr was:\n{stderr}");
    assert_eq!(responses[0]["error"]["kind"], json!("invalid_request"));
    assert_eq!(responses[1]["error"]["kind"], json!("not_loaded"));
    assert_eq!(responses[2]["error"]["kind"], json!("invalid_request"));
    assert_eq!(responses[3]["error"]["kind"], json!("compile_error"));
    assert_eq!(responses[4]["error"]["kind"], json!("runtime_cycle_error"));
    assert!(responses[4]["error"]["data"]["errors"].is_array());
    assert_eq!(responses[5]["ok"], json!(true));
    assert_eq!(responses[6]["error"]["kind"], json!("invalid_request"));
    assert_eq!(responses[7]["ok"], json!(true));
    assert_eq!(
        responses[7]["data"]["values"]["latched"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": false}})
    );
    assert_eq!(
        responses[7]["data"]["values"]["missing"],
        json!({
            "status": "error",
            "code": "unresolved_name",
            "message": "boundary path 'missing' did not resolve to a declared value",
            "path": "missing",
            "candidates": [],
        })
    );
    assert!(responses
        .iter()
        .all(|response| response["protocol_version"] == json!(2)));
}

#[test]
fn trust_harness_protocol_v1_promotes_watch_errors_to_request_errors() {
    let (responses, stderr) = run_harness_with_args(
        &["--protocol-version=1"],
        &[
            json!({"cmd": "load", "source": latch_program()}),
            json!({"cmd": "snapshot", "watch": ["latched", "missing"]}),
        ],
    );

    assert_eq!(responses.len(), 2, "stderr was:\n{stderr}");
    assert_eq!(responses[1]["ok"], json!(false));
    assert_eq!(responses[1]["protocol_version"], json!(1));
    assert_eq!(responses[1]["error"]["kind"], json!("unresolved_name"));
    assert_eq!(responses[1]["error"]["data"]["path"], json!("missing"));
    assert_eq!(responses[1]["error"]["data"]["candidates"], json!([]));
}

#[test]
fn trust_harness_var_access_and_direct_io_commands_roundtrip() {
    let (responses, stderr) = run_harness(&[
        json!({"cmd": "load", "source": access_and_direct_io_program()}),
        json!({"cmd": "bind_direct", "name": "input", "address": "%IX0.0"}),
        json!({"cmd": "bind_direct", "name": "output", "address": "%QX0.0"}),
        json!({
            "cmd": "set_direct_input",
            "address": "%IX0.0",
            "value": {"type": "BOOL", "value": true},
        }),
        json!({
            "cmd": "set_access",
            "name": "RemoteValue",
            "value": {"type": "INT", "value": 42},
        }),
        json!({"cmd": "cycle"}),
        json!({"cmd": "get_direct_output", "address": "%QX0.0"}),
        json!({"cmd": "get_access", "name": "RemoteValue"}),
    ]);

    assert_eq!(responses.len(), 8, "stderr was:\n{stderr}");
    assert!(responses
        .iter()
        .all(|response| response["ok"] == json!(true)));
    assert_eq!(
        responses[6]["data"]["value"],
        json!({"type": "BOOL", "value": true})
    );
    assert_eq!(
        responses[7]["data"]["value"],
        json!({"type": "INT", "value": 42})
    );
}

#[test]
fn trust_harness_snapshot_is_passive_and_restart_modes_are_explicit() {
    let (responses, stderr) = run_harness(&[
        json!({"cmd": "load", "source": retained_program(1)}),
        json!({
            "cmd": "set_input",
            "name": "counter",
            "value": {"type": "INT", "value": 7},
        }),
        json!({"cmd": "snapshot", "watch": ["counter"]}),
        json!({"cmd": "snapshot", "watch": ["counter"]}),
        json!({"cmd": "restart", "mode": "WARM"}),
        json!({"cmd": "get_output", "name": "counter"}),
        json!({"cmd": "restart", "mode": "cold"}),
        json!({"cmd": "get_output", "name": "counter"}),
    ]);

    assert_eq!(responses.len(), 8, "stderr was:\n{stderr}");
    assert_eq!(
        responses[2]["data"]["cycle_count"],
        responses[3]["data"]["cycle_count"]
    );
    assert_eq!(responses[4]["data"]["mode"], json!("warm"));
    assert_eq!(
        responses[5]["data"]["value"],
        json!({"type": "INT", "value": 7})
    );
    assert_eq!(responses[6]["data"]["mode"], json!("cold"));
    assert_eq!(
        responses[7]["data"]["value"],
        json!({"type": "INT", "value": 1})
    );
}

#[test]
fn trust_harness_cli_protocol_selection_overrides_environment_and_rejects_bad_arguments() {
    let not_loaded = vec![json!({"cmd": "snapshot"}).to_string()];
    let (status, responses, stderr) = run_harness_lines(
        &["--protocol-version=2"],
        &[("TRUST_HARNESS_PROTOCOL_VERSION", "1")],
        &not_loaded,
    );
    assert!(status.success(), "stderr was:\n{stderr}");
    assert_eq!(responses[0]["protocol_version"], json!(2));

    let (status, responses, stderr) = run_harness_lines(&["--protocol-version=3"], &[], &[]);
    assert!(!status.success());
    assert!(responses.is_empty());
    assert!(stderr.contains("unsupported protocol version '3'"));
}
