//! Minimal JSON-line harness for deterministic cycle driving.

use std::io::{self, BufRead, Write};

use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[derive(Debug, Deserialize)]
struct Request {
    cmd: String,
    source: Option<String>,
    count: Option<u32>,
    dt_ms: Option<i64>,
    watch: Option<Vec<String>>,
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut harness: Option<TestHarness> = None;

    for line in stdin.lock().lines() {
        let line = line.context("read stdin line")?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle_request(request, &mut harness),
            Err(err) => error_response(format!("invalid request: {err}")),
        };

        writeln!(out, "{}", serde_json::to_string(&response)?)?;
        out.flush()?;
    }

    Ok(())
}

fn handle_request(request: Request, harness: &mut Option<TestHarness>) -> JsonValue {
    match dispatch_request(request, harness) {
        Ok(data) => json!({
            "ok": true,
            "data": data,
        }),
        Err(err) => error_response(err.to_string()),
    }
}

fn dispatch_request(
    request: Request,
    harness: &mut Option<TestHarness>,
) -> anyhow::Result<JsonValue> {
    match request.cmd.as_str() {
        "load" => handle_load(request, harness),
        "cycle" => handle_cycle(request, harness),
        other => Err(anyhow!("unsupported command '{other}'")),
    }
}

fn handle_load(request: Request, harness: &mut Option<TestHarness>) -> anyhow::Result<JsonValue> {
    let source = request
        .source
        .as_deref()
        .ok_or_else(|| anyhow!("load requires 'source'"))?;
    let mut loaded = TestHarness::from_source(source)?;
    let cycle = loaded.cycle();
    if !cycle.errors.is_empty() {
        let rendered = cycle
            .errors
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow!("initial cycle failed: {rendered}"));
    }
    *harness = Some(loaded);
    Ok(json!({
        "cycle_count": 1,
        "elapsed_ms": 0,
    }))
}

fn handle_cycle(request: Request, harness: &mut Option<TestHarness>) -> anyhow::Result<JsonValue> {
    let harness = harness
        .as_mut()
        .ok_or_else(|| anyhow!("cycle requires a loaded program"))?;
    let count = request.count.unwrap_or(1);

    if let Some(dt_ms) = request.dt_ms {
        if dt_ms < 0 {
            return Err(anyhow!("dt_ms must be non-negative"));
        }
    }

    for _ in 0..count {
        if let Some(dt_ms) = request.dt_ms {
            harness.advance_time(Duration::from_millis(dt_ms));
        }
        let cycle = harness.cycle();
        if !cycle.errors.is_empty() {
            let rendered = cycle
                .errors
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!("cycle failed: {rendered}"));
        }
    }

    let values = watched_values(harness, request.watch.as_deref().unwrap_or(&[]));
    Ok(json!({
        "cycle_count": harness.cycle_count(),
        "elapsed_ms": harness.current_time().as_millis(),
        "values": values,
    }))
}

fn watched_values(harness: &TestHarness, watch: &[String]) -> JsonValue {
    let mut values = Map::new();
    for name in watch {
        let value = harness
            .get_output(name)
            .as_ref()
            .map(value_to_json)
            .unwrap_or(JsonValue::Null);
        values.insert(name.clone(), value);
    }
    JsonValue::Object(values)
}

fn value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Bool(v) => json!(v),
        Value::SInt(v) => json!(v),
        Value::Int(v) => json!(v),
        Value::DInt(v) => json!(v),
        Value::LInt(v) => json!(v),
        Value::USInt(v) => json!(v),
        Value::UInt(v) => json!(v),
        Value::UDInt(v) => json!(v),
        Value::ULInt(v) => json!(v),
        Value::Real(v) => json!(v),
        Value::LReal(v) => json!(v),
        Value::Byte(v) => json!(v),
        Value::Word(v) => json!(v),
        Value::DWord(v) => json!(v),
        Value::LWord(v) => json!(v),
        Value::Time(v) => json!({"type": "TIME", "ms": v.as_millis()}),
        Value::LTime(v) => json!({"type": "LTIME", "ms": v.as_millis()}),
        Value::Date(v) => json!({"type": "DATE", "ticks": v.ticks()}),
        Value::LDate(v) => json!({"type": "LDATE", "nanos": v.nanos()}),
        Value::Tod(v) => json!({"type": "TOD", "ticks": v.ticks()}),
        Value::LTod(v) => json!({"type": "LTOD", "nanos": v.nanos()}),
        Value::Dt(v) => json!({"type": "DT", "ticks": v.ticks()}),
        Value::Ldt(v) => json!({"type": "LDT", "nanos": v.nanos()}),
        Value::String(v) => json!(v.to_string()),
        Value::WString(v) => json!(v),
        Value::Char(v) => json!(char::from(*v).to_string()),
        Value::WChar(v) => json!(std::char::from_u32(u32::from(*v))
            .unwrap_or('\u{FFFD}')
            .to_string()),
        Value::Array(array) => JsonValue::Array(array.elements.iter().map(value_to_json).collect()),
        Value::Struct(value) => {
            let mut fields = Map::new();
            for (name, field_value) in &value.fields {
                fields.insert(name.to_string(), value_to_json(field_value));
            }
            json!({
                "type": "STRUCT",
                "type_name": value.type_name.to_string(),
                "fields": fields,
            })
        }
        Value::Enum(value) => json!({
            "type": "ENUM",
            "type_name": value.type_name.to_string(),
            "variant": value.variant_name.to_string(),
            "numeric": value.numeric_value,
        }),
        Value::Reference(reference) => json!({
            "type": "REFERENCE",
            "value": reference.as_ref().map(|entry| format!("{entry:?}")),
        }),
        Value::Instance(id) => json!({
            "type": "INSTANCE",
            "value": id.0,
        }),
        Value::Null => JsonValue::Null,
    }
}

fn error_response(message: String) -> JsonValue {
    json!({
        "ok": false,
        "error": message,
    })
}
