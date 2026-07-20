use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "support/modbus.rs"]
mod modbus_support;

use modbus_support::{start_delayed_modbus_server, ModbusTestState};
use trust_runtime::io::{IoDriver, ModbusTcpDriver};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_IO_BOUND_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_IO_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:5651bab7a19964e2f3e037947628d2fd32c8b6a737fea523b53a5ba0aead009b";

#[test]
fn runtime_io_bound_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = IoBoundProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_io_case)
        .expect("I/O bound case artifact must be written");
    let failed = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            format!(
                "{}: {}",
                case.id,
                case.observed_error.as_deref().unwrap_or("not passed")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "I/O bound failures: {}",
        failed.join("; ")
    );
}

fn run_io_case(case: &CaseRecord, probe: &mut IoBoundProbe) -> Result<CaseExecution, String> {
    let step = case
        .trace
        .as_deref()
        .and_then(|trace| trace.first())
        .ok_or_else(|| format!("{} requires one trace step", case.id))?;
    let operation = required_string(&step.stimulus, "operation")?;
    let delay_ms = required_u64(&step.stimulus, "delay_ms")?;
    let bound_ms = required_u64(&step.stimulus, "bound_ms")?;
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let addr = start_delayed_modbus_server(state, Duration::from_millis(delay_ms));
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{addr}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = 1000\non_error = \"warn\"\n"
    ))
    .map_err(|error| format!("invalid Modbus fixture parameters: {error}"))?;
    let mut driver = ModbusTcpDriver::from_params(&params)
        .map_err(|error| format!("Modbus fixture setup failed: {error}"))?;

    let started = Instant::now();
    match operation {
        "read_inputs" => driver
            .read_inputs(&mut [0u8; 2])
            .map_err(|error| format!("read_inputs failed: {error}"))?,
        "write_outputs" => driver
            .write_outputs(&[0x12, 0x34])
            .map_err(|error| format!("write_outputs failed: {error}"))?,
        other => return Err(format!("unreviewed I/O operation {other}")),
    }
    let elapsed = started.elapsed();
    let passed = elapsed < Duration::from_millis(bound_ms);
    probe.target = Some(serde_json::json!({
        "bound_ms": bound_ms,
        "elapsed_us": elapsed.as_micros(),
        "operation": operation,
        "within_bound": passed,
    }));
    Ok(CaseExecution {
        result: if passed {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!passed)
            .then(|| format!("{operation} took {elapsed:?}, exceeding the {bound_ms} ms bound")),
        observed_status: Some(
            if passed {
                "scan_handoff_bounded"
            } else {
                "scan_handoff_blocked"
            }
            .to_string(),
        ),
    })
}

fn required_string<'a>(
    values: &'a BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<&'a str, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("trace field {key} must be a string"))
}

fn required_u64(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<u64, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| format!("trace field {key} must be a non-negative integer"))
}

#[derive(Default)]
struct IoBoundProbe {
    target: Option<serde_json::Value>,
    next_snapshot_is_after: bool,
}

impl StateProbe for IoBoundProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_after {
            self.target = None;
        }
        self.next_snapshot_is_after = !self.next_snapshot_is_after;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.target.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
