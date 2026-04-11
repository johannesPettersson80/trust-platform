use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value as JsonValue};

fn timer_program() -> &'static str {
    r#"
PROGRAM Main
VAR
    ton_fb : TON;
    q : BOOL;
    et : TIME;
END_VAR
ton_fb(IN := TRUE, PT := T#100MS, Q => q, ET => et);
END_PROGRAM
"#
}

fn run_harness(requests: &[JsonValue]) -> (Vec<JsonValue>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_trust-harness"))
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
            "cmd": "cycle",
            "count": 10,
            "dt_ms": 10,
            "watch": ["q", "et"],
        }),
    ]);

    assert_eq!(responses.len(), 2, "stderr was:\n{stderr}");
    assert_eq!(responses[0]["ok"], json!(true));
    assert_eq!(responses[1]["ok"], json!(true));
    assert_eq!(responses[1]["data"]["values"]["q"], json!(true));
    assert_eq!(
        responses[1]["data"]["values"]["et"],
        json!({"type": "TIME", "ms": 100})
    );
}

#[test]
fn trust_harness_cycle_without_dt_ms_keeps_time_frozen() {
    let (responses, stderr) = run_harness(&[
        json!({
            "cmd": "load",
            "source": timer_program(),
        }),
        json!({
            "cmd": "cycle",
            "count": 10,
            "watch": ["q", "et"],
        }),
    ]);

    assert_eq!(responses.len(), 2, "stderr was:\n{stderr}");
    assert_eq!(responses[0]["ok"], json!(true));
    assert_eq!(responses[1]["ok"], json!(true));
    assert_eq!(responses[1]["data"]["values"]["q"], json!(false));
    assert_eq!(
        responses[1]["data"]["values"]["et"],
        json!({"type": "TIME", "ms": 0})
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
    assert!(
        responses[1]["error"]
            .as_str()
            .expect("error string")
            .contains("dt_ms"),
        "expected dt_ms error, got: {}",
        responses[1]
    );
}
