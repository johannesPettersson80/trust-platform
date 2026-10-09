use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "support/modbus.rs"]
mod modbus_support;

use modbus_support::{start_gated_modbus_server, ModbusTestState};
use trust_runtime::io::{IoDriver, ModbusTcpDriver};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_IO_BOUND_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_IO_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:3f7be52d2cee2f095a11f999ff036eb9fa64e20eb34aa1a8b332028352da86df";

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
    let completion_timeout_ms = required_u64(&step.stimulus, "completion_timeout_ms")?;
    let transport_timeout_ms = required_u64(&step.stimulus, "transport_timeout_ms")?;
    if completion_timeout_ms >= transport_timeout_ms {
        return Err("harness completion timeout must precede transport timeout".into());
    }
    let state = Arc::new(Mutex::new(ModbusTestState::with_registers(
        vec![0x1122],
        vec![0u16; 1],
    )));
    let peer = start_gated_modbus_server(state);
    let params: toml::Value = toml::from_str(&format!(
        "address = \"{}\"\nunit_id = 1\ninput_start = 0\noutput_start = 0\ntimeout_ms = {transport_timeout_ms}\non_error = \"warn\"\n", peer.addr
    )).map_err(|error| format!("invalid Modbus fixture parameters: {error}"))?;
    let mut driver = ModbusTcpDriver::from_params(&params)
        .map_err(|error| format!("Modbus fixture setup failed: {error}"))?;
    let operation = operation.to_owned();
    let worker_operation = operation.clone();
    let (completed_tx, completed_rx) = std::sync::mpsc::channel();
    let scan = std::thread::spawn(move || {
        let started = Instant::now();
        let result = match worker_operation.as_str() {
            "read_inputs" => driver.read_inputs(&mut [0u8; 2]),
            "write_outputs" => driver.write_outputs(&[0x12, 0x34]),
            _ => panic!("unreviewed I/O operation"),
        };
        // Retain the driver until after the peer request is observed, so Drop cannot
        // stop a worker that has not yet been scheduled.
        let _ = completed_tx.send((driver, result, started.elapsed()));
    });
    let deadline = Instant::now() + Duration::from_millis(completion_timeout_ms);
    let received = peer.wait_for_request(deadline.saturating_duration_since(Instant::now()));
    let completed = completed_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    let scan_completed = completed.is_ok();
    // No reply has been released on either path. Cleanup releases it even after failure.
    let observed = match (received, completed) {
        (Ok(()), Ok((driver, result, elapsed))) => {
            let degraded = matches!(
                driver.health(),
                trust_runtime::io::IoDriverHealth::Degraded { .. }
            );
            probe.target = Some(serde_json::json!({
                "operation": operation,
                "elapsed_us": elapsed.as_micros(),
                "returned_before_response": result.is_ok() && degraded,
            }));
            result.map_err(|error| error.to_string()).and_then(|()| {
                if degraded {
                    Ok(())
                } else {
                    Err("pending handoff did not report degraded health".into())
                }
            })
        }
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(format!(
            "scan did not return while response was withheld: {error}"
        )),
    };
    drop(peer);
    // A real internal deadlock must fail the finite assertion, not hang cleanup.
    // Successful callbacks have already sent their result and can be joined.
    if scan_completed {
        let _ = scan.join();
    }
    Ok(CaseExecution {
        result: if observed.is_ok() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: observed.err(),
        observed_status: Some("scan_handoff_before_protocol_response".into()),
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
