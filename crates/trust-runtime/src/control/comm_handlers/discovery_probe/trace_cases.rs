use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value as JsonValue};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

use super::{
    probe_modbus_tcp, probe_mqtt, DiscoveryProbeOutcome, ModbusDiscoveryProbe, ModbusSafeReadProbe,
};

const TEST_ID: &str = "TEST_PROTOCOL_DISCOVERY_CONFIDENCE_TRACE_001";
const CASE_FILE: &str = "verification/cases/protocols/PROTO_DISCOVERY_TRUTH_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:d5e970a36ac73985d6a38e4df1c45323ee723d26796561732a49fea930c9fed8";

#[test]
fn protocol_discovery_confidence_trace_cases() {
    let mut probe = DiscoveryProbe::default();
    let config = RunConfig::new(TEST_ID, workspace_root().join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_discovery_case)
        .expect("protocol-discovery artifact must be written");
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
        "protocol-discovery trace failures: {}",
        failed.join("; ")
    );
}

fn run_discovery_case(
    case: &CaseRecord,
    probe: &mut DiscoveryProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no discovery trace", case.id))?;
    let mut mismatches = Vec::new();
    for step in trace {
        if trace_string(&step.stimulus, "action")? != "probe" {
            return Err(format!("{} uses an unreviewed discovery action", case.id));
        }
        let observed = execute_probe(scenario)?;
        compare_expected(step, &observed, &mut mismatches)?;
        probe.observed = Some(observed);
    }
    Ok(case_execution(mismatches))
}

fn execute_probe(scenario: &str) -> Result<JsonValue, String> {
    match scenario {
        "MODBUS_DEVICE_ID_CONFIRMED" => {
            let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
            let addr = listener.local_addr().map_err(|error| error.to_string())?;
            let handle = thread::spawn(move || -> Result<(), String> {
                let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
                let mut request = [0u8; 11];
                stream
                    .read_exact(&mut request)
                    .map_err(|error| error.to_string())?;
                if request[7..11] != [0x2b, 0x0e, 0x01, 0x00] {
                    return Err(format!("unexpected Modbus device-id request: {request:?}"));
                }
                stream
                    .write_all(&[
                        0x00, 0x01, 0x00, 0x00, 0x00, 0x08, 0x01, 0x2b, 0x0e, 0x01, 0x01, 0x00,
                        0x00, 0x00,
                    ])
                    .map_err(|error| error.to_string())
            });
            let outcome = probe_modbus_tcp(
                addr,
                Duration::from_millis(250),
                ModbusDiscoveryProbe {
                    unit_id: 1,
                    safe_read: None,
                },
            )
            .ok_or_else(|| "Modbus device-id probe returned no outcome".to_string())?;
            join_server(handle)?;
            Ok(outcome_json(outcome, false, false))
        }
        "MODBUS_SAFE_READ_CONFIRMED" => {
            let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
            let addr = listener.local_addr().map_err(|error| error.to_string())?;
            let handle = thread::spawn(move || -> Result<(), String> {
                let (mut first, _) = listener.accept().map_err(|error| error.to_string())?;
                let mut device_id = [0u8; 11];
                first
                    .read_exact(&mut device_id)
                    .map_err(|error| error.to_string())?;
                drop(first);
                let (mut second, _) = listener.accept().map_err(|error| error.to_string())?;
                let mut safe_read = [0u8; 12];
                second
                    .read_exact(&mut safe_read)
                    .map_err(|error| error.to_string())?;
                if safe_read[7..12] != [0x03, 0x01, 0x90, 0x00, 0x01] {
                    return Err(format!(
                        "unexpected Modbus safe-read request: {safe_read:?}"
                    ));
                }
                second
                    .write_all(&[0x00, 0x02, 0x00, 0x00, 0x00, 0x05, 0x11, 0x03, 0x02, 0, 0])
                    .map_err(|error| error.to_string())
            });
            let outcome = probe_modbus_tcp(
                addr,
                Duration::from_millis(250),
                ModbusDiscoveryProbe {
                    unit_id: 17,
                    safe_read: Some(ModbusSafeReadProbe {
                        address: 400,
                        quantity: 1,
                    }),
                },
            )
            .ok_or_else(|| "Modbus safe-read probe returned no outcome".to_string())?;
            join_server(handle)?;
            Ok(outcome_json(outcome, false, false))
        }
        "MODBUS_TCP_ONLY" => {
            let (addr, handle) = tcp_only_server()?;
            let outcome = probe_modbus_tcp(
                addr,
                Duration::from_millis(250),
                ModbusDiscoveryProbe {
                    unit_id: 1,
                    safe_read: None,
                },
            )
            .ok_or_else(|| "Modbus TCP-only probe returned no outcome".to_string())?;
            join_server(handle)?;
            Ok(outcome_json(outcome, false, false))
        }
        "MQTT_ACCEPTED_CONNACK" => mqtt_probe_scenario(0),
        "MQTT_AUTH_REJECTED" => mqtt_probe_scenario(5),
        "MQTT_GENERIC_REJECTED" => mqtt_probe_scenario(2),
        "MQTT_TCP_ONLY" => {
            let (addr, handle) = tcp_only_server()?;
            let outcome = probe_mqtt(addr, Duration::from_millis(250))
                .ok_or_else(|| "MQTT TCP-only probe returned no outcome".to_string())?;
            join_server(handle)?;
            Ok(outcome_json(outcome, false, false))
        }
        other => Err(format!("unreviewed discovery scenario {other}")),
    }
}

fn mqtt_probe_scenario(code: u8) -> Result<JsonValue, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || -> Result<(), String> {
        let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
        let mut fixed = [0u8; 2];
        stream
            .read_exact(&mut fixed)
            .map_err(|error| error.to_string())?;
        if fixed[0] != 0x10 {
            return Err(format!("unexpected MQTT packet type: {fixed:?}"));
        }
        let mut body = vec![0u8; usize::from(fixed[1])];
        stream
            .read_exact(&mut body)
            .map_err(|error| error.to_string())?;
        let clean_session = body.get(7).is_some_and(|flags| flags & 0x02 != 0);
        stream
            .write_all(&[0x20, 0x02, 0x00, code])
            .map_err(|error| error.to_string())?;
        let mut disconnect = [0u8; 2];
        stream
            .read_exact(&mut disconnect)
            .map_err(|error| error.to_string())?;
        tx.send((clean_session, disconnect == [0xe0, 0x00]))
            .map_err(|error| error.to_string())
    });
    let outcome = probe_mqtt(addr, Duration::from_millis(250))
        .ok_or_else(|| "MQTT probe returned no outcome".to_string())?;
    let (clean_session, disconnect_seen) = rx.recv().map_err(|error| error.to_string())?;
    join_server(handle)?;
    Ok(outcome_json(outcome, clean_session, disconnect_seen))
}

type ServerHandle = thread::JoinHandle<Result<(), String>>;

fn tcp_only_server() -> Result<(std::net::SocketAddr, ServerHandle), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    let handle = thread::spawn(move || -> Result<(), String> {
        let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
        let mut buffer = [0u8; 256];
        let _ = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        Ok(())
    });
    Ok((addr, handle))
}

fn join_server(handle: ServerHandle) -> Result<(), String> {
    handle
        .join()
        .map_err(|_| "protocol probe server panicked".to_string())?
}

fn outcome_json(
    outcome: DiscoveryProbeOutcome,
    clean_session: bool,
    disconnect_seen: bool,
) -> JsonValue {
    json!({
        "auth_required": outcome.auth_required,
        "clean_session": clean_session,
        "confidence": outcome.confidence,
        "disconnect_seen": disconnect_seen,
        "source": outcome.source,
        "warning": !outcome.warnings.is_empty(),
    })
}

fn compare_expected(
    step: &TraceStep,
    observed: &JsonValue,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (key, expected) in &step.expected {
        let expected = serde_json::to_value(expected).map_err(|error| error.to_string())?;
        let actual = observed.get(key).cloned().unwrap_or(JsonValue::Null);
        if actual != expected {
            mismatches.push(format!(
                "step {} {key}: expected {expected}, observed {actual}",
                step.sequence
            ));
        }
    }
    Ok(())
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace {key} must be a string"))
}

fn case_execution(mismatches: Vec<String>) -> CaseExecution {
    if mismatches.is_empty() {
        CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("discovery_trace_passed".to_string()),
        }
    } else {
        CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("discovery_trace_mismatch".to_string()),
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf()
}

#[derive(Default)]
struct DiscoveryProbe {
    observed: Option<JsonValue>,
    next_snapshot_is_before: bool,
}

impl StateProbe for DiscoveryProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_before {
            self.observed = None;
        }
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
