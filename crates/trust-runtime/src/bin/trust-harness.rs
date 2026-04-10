//! Lightweight CLI harness for IEC-MCP integration.
//!
//! Reads ST source from a file, executes N cycles with TestHarness,
//! and outputs results as JSON via stdout. Inputs/outputs via JSON protocol
//! on stdin/stdout (one JSON per line).
//!
//! Protocol:
//!   Request:  {"cmd": "load", "source": "PROGRAM Main..."}
//!   Request:  {"cmd": "cycle", "count": 1, "inputs": {"var": "TRUE"}, "watch": ["var1","var2"]}
//!   Request:  {"cmd": "state", "watch": ["var1","var2"]}
//!   Response: {"ok": true, "data": {...}}

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[derive(Deserialize)]
struct Request {
    cmd: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    inputs: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    watch: Option<Vec<String>>,
    /// Scan time in milliseconds — advances virtual time per cycle (for TON/TOF/TP)
    #[serde(default)]
    dt_ms: Option<i64>,
}

#[derive(Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut harness: Option<TestHarness> = None;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(err) => {
                let resp = Response {
                    ok: false,
                    data: None,
                    error: Some(format!("invalid JSON: {err}")),
                };
                let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
                let _ = out.flush();
                continue;
            }
        };

        let resp = match request.cmd.as_str() {
            "load" => handle_load(&request, &mut harness),
            "cycle" => handle_cycle(&request, &mut harness),
            "state" => handle_state(&request, &harness),
            "set" => handle_set(&request, &mut harness),
            _ => Response {
                ok: false,
                data: None,
                error: Some(format!("unknown command: {}", request.cmd)),
            },
        };

        let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
        let _ = out.flush();
    }
}

fn handle_load(req: &Request, harness: &mut Option<TestHarness>) -> Response {
    let source = match &req.source {
        Some(s) => s,
        None => {
            return Response {
                ok: false,
                data: None,
                error: Some("missing 'source' field".into()),
            }
        }
    };

    match TestHarness::from_source(source) {
        Ok(h) => {
            *harness = Some(h);
            Response {
                ok: true,
                data: Some(json!({"message": "program loaded"})),
                error: None,
            }
        }
        Err(err) => Response {
            ok: false,
            data: None,
            error: Some(format!("compile error: {err}")),
        },
    }
}

fn handle_cycle(req: &Request, harness: &mut Option<TestHarness>) -> Response {
    let h = match harness.as_mut() {
        Some(h) => h,
        None => {
            return Response {
                ok: false,
                data: None,
                error: Some("no program loaded".into()),
            }
        }
    };

    // Apply inputs before cycle
    if let Some(inputs) = &req.inputs {
        for (name, value_str) in inputs {
            let value = parse_value_str(value_str);
            h.set_input(name, value);
        }
    }

    // Execute cycles, advancing virtual time if dt_ms is provided
    let count = req.count.unwrap_or(1);
    let dt = req.dt_ms.map(Duration::from_millis);
    for _ in 0..count {
        // Advance time BEFORE cycle so TON sees elapsed time during execution
        if let Some(dt) = dt {
            h.advance_time(dt);
        }
        h.cycle();
    }

    // Read watched variables
    let values = read_watch(h, req.watch.as_deref());

    Response {
        ok: true,
        data: Some(json!({
            "cycles": count,
            "values": values,
        })),
        error: None,
    }
}

fn handle_state(req: &Request, harness: &Option<TestHarness>) -> Response {
    let h = match harness.as_ref() {
        Some(h) => h,
        None => {
            return Response {
                ok: false,
                data: None,
                error: Some("no program loaded".into()),
            }
        }
    };

    let values = read_watch_ref(h, req.watch.as_deref());

    Response {
        ok: true,
        data: Some(json!({"values": values})),
        error: None,
    }
}

fn handle_set(req: &Request, harness: &mut Option<TestHarness>) -> Response {
    let h = match harness.as_mut() {
        Some(h) => h,
        None => {
            return Response {
                ok: false,
                data: None,
                error: Some("no program loaded".into()),
            }
        }
    };

    if let Some(inputs) = &req.inputs {
        for (name, value_str) in inputs {
            let value = parse_value_str(value_str);
            h.set_input(name, value);
        }
    }

    Response {
        ok: true,
        data: Some(json!({"message": "values set"})),
        error: None,
    }
}

fn read_watch(h: &TestHarness, watch: Option<&[String]>) -> serde_json::Map<String, JsonValue> {
    let mut values = serde_json::Map::new();
    if let Some(vars) = watch {
        for name in vars {
            if let Some(val) = h.get_output(name) {
                values.insert(name.clone(), value_to_json(&val));
            }
        }
    }
    values
}

fn read_watch_ref(h: &TestHarness, watch: Option<&[String]>) -> serde_json::Map<String, JsonValue> {
    read_watch(h, watch)
}

fn value_to_json(val: &Value) -> JsonValue {
    match val {
        Value::Bool(b) => json!(*b),
        Value::SInt(n) => json!(*n),
        Value::Int(n) => json!(*n),
        Value::DInt(n) => json!(*n),
        Value::LInt(n) => json!(*n),
        Value::USInt(n) => json!(*n),
        Value::UInt(n) => json!(*n),
        Value::UDInt(n) => json!(*n),
        Value::ULInt(n) => json!(*n),
        Value::Real(n) => json!(*n),
        Value::LReal(n) => json!(*n),
        _ => json!(format!("{val:?}")),
    }
}

fn parse_value_str(s: &str) -> Value {
    match s.to_uppercase().as_str() {
        "TRUE" => Value::Bool(true),
        "FALSE" => Value::Bool(false),
        _ => {
            if let Ok(n) = s.parse::<i16>() {
                Value::Int(n)
            } else if let Ok(n) = s.parse::<i32>() {
                Value::DInt(n)
            } else if let Ok(n) = s.parse::<f32>() {
                Value::Real(n)
            } else {
                Value::Bool(false)
            }
        }
    }
}
