use std::io::Write;
use std::process::{Command, Stdio};

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

fn run_harness(requests: &[JsonValue]) -> (Vec<JsonValue>, String) {
    run_harness_with_args(&[], requests)
}

fn run_harness_with_args(args: &[&str], requests: &[JsonValue]) -> (Vec<JsonValue>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-harness"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-harness");

    let mut stdin = child.stdin.take().expect("harness stdin");
    for request in requests {
        writeln!(
            stdin,
            "{}",
            serde_json::to_string(request).expect("encode request")
        )
        .expect("write request");
    }
    drop(stdin);

    let output = child.wait_with_output().expect("wait for trust-harness");
    assert!(
        output.status.success(),
        "expected trust-harness success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let responses = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<JsonValue>(line).expect("decode response"))
        .collect();
    (
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
