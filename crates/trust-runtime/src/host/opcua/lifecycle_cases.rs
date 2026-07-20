use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use smol_str::SmolStr;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

use super::*;

const TEST_ID: &str = "TEST_OPCUA_CLIENT_LIFECYCLE_TRACE_001";
const CASE_FILE: &str = "verification/cases/protocols/PROTO_OPCUA_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:d5f1740836a29c1a0742f3040e5060a01fc78f07e99870a8c139f27c41191e7d";

#[test]
fn opcua_client_lifecycle_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = LifecycleProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_lifecycle_case);

    let artifact = result.expect("OPC UA lifecycle artifact must be written");
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
        "OPC UA lifecycle failures: {}",
        failed.join("; ")
    );
}

fn run_lifecycle_case(
    case: &CaseRecord,
    probe: &mut LifecycleProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let mut fixture = LifecycleFixture::new(scenario)?;
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no protocol trace", case.id))?;
    let mut mismatches = Vec::new();

    for step in trace {
        fixture.execute(step)?;
        let observed = fixture.observed();
        compare_expected(step, &observed, &mut mismatches)?;
        probe.observed = Some(observed);
    }

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("protocol_trace_passed".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("protocol_trace_mismatch".to_string()),
        })
    }
}

struct LifecycleFixture {
    runtime: crate::Runtime,
    point: OpcUaClientPointConfig,
    bridge: OpcUaClientBridge,
    worker: OpcUaClientWorker<LifecycleTransport>,
}

impl LifecycleFixture {
    fn new(scenario: &str) -> Result<Self, String> {
        let mut runtime = crate::Runtime::new();
        runtime
            .storage_mut()
            .set_global(SmolStr::new("line1_value"), Value::Real(0.0));
        let point = OpcUaClientPointConfig {
            var: SmolStr::new("line1_value"),
            node_id: "ns=2;i=2".to_string(),
            data_type: OpcUaDataType::Float,
            access: OpcUaClientPointAccess::ReadWrite,
        };
        let reference = runtime
            .storage()
            .ref_for_global("line1_value")
            .ok_or_else(|| "line1_value global reference is missing".to_string())?;
        let binding = OpcUaClientBinding {
            point: point.clone(),
            reference,
        };
        let transport = LifecycleTransport {
            recover_succeeds: scenario == "SUBSCRIPTION_TRANSFER_RECOVERY",
            write_error: (scenario == "REJECTED_WRITE_POINT_FAULT_ISOLATION").then(|| {
                OpcUaClientBridgeError::validation(
                    "OPC UA node 'ns=2;i=2' write returned BadUserAccessDenied",
                )
            }),
            ..LifecycleTransport::default()
        };
        let connection = OpcUaClientConnectionConfig {
            name: SmolStr::new("line1"),
            endpoint_url: "opc.tcp://127.0.0.1:4840/trust".to_string(),
            security: OpcUaSecurityProfile {
                policy: OpcUaSecurityPolicy::None,
                mode: OpcUaMessageSecurityMode::None,
                allow_anonymous: true,
            },
            auth: OpcUaClientAuthConfig::Anonymous,
            trust_server_certificate: true,
            poll_interval_ms: 10,
            timeout_ms: 50,
            points: vec![point.clone()],
        };
        let (bridge, worker) =
            OpcUaClientBridge::with_transport(connection, transport, vec![binding])
                .map_err(|error| error.to_string())?;
        Ok(Self {
            runtime,
            point,
            bridge,
            worker,
        })
    }

    fn execute(&mut self, step: &TraceStep) -> Result<(), String> {
        let action = trace_string(&step.stimulus, "action")?;
        let now_ms = trace_u64(&step.stimulus, "now_ms")?;
        match action.as_str() {
            "tick" => self
                .worker
                .tick(now_ms)
                .map_err(|error| error.to_string())?,
            "sample" => {
                let value_milli = trace_i64(&step.stimulus, "value_milli")?;
                self.worker.transport_mut().emit_sample(
                    &self.point,
                    Value::Real(value_milli as f32 / 1_000.0),
                    now_ms,
                )?;
                self.worker
                    .tick(now_ms)
                    .map_err(|error| error.to_string())?;
                self.bridge
                    .apply_inputs(self.runtime.storage_mut(), now_ms.saturating_add(1))
                    .map_err(|error| error.to_string())?;
            }
            "disconnect" => {
                self.worker.transport_mut().emit_connection_status(
                    false,
                    now_ms,
                    "session lost",
                )?;
                self.worker
                    .tick(now_ms)
                    .map_err(|error| error.to_string())?;
            }
            "write" => {
                let value_milli = trace_i64(&step.stimulus, "value_milli")?;
                let reference = self
                    .runtime
                    .storage()
                    .ref_for_global("line1_value")
                    .ok_or_else(|| "line1_value global reference is missing".to_string())?;
                if !self
                    .runtime
                    .storage_mut()
                    .write_by_ref(reference, Value::Real(value_milli as f32 / 1_000.0))
                {
                    return Err("line1_value write failed".to_string());
                }
                self.bridge
                    .capture_outputs(self.runtime.storage_mut(), now_ms)
                    .map_err(|error| error.to_string())?;
                self.worker
                    .tick(now_ms)
                    .map_err(|error| error.to_string())?;
            }
            other => return Err(format!("unreviewed OPC UA trace action {other}")),
        }
        Ok(())
    }

    fn observed(&self) -> serde_json::Value {
        let snapshot = self.bridge.snapshot();
        let storage_milli = match self.runtime.storage().get_global("line1_value") {
            Some(Value::Real(value)) => Some((*value * 1_000.0).round() as i64),
            _ => None,
        };
        let point_status = snapshot
            .point_statuses
            .iter()
            .find(|status| status.var == "line1_value");
        json!({
            "connection_detail": snapshot.detail,
            "connect_count": self.worker.transport().connect_count,
            "disconnect_count": self.worker.transport().disconnect_count,
            "pending": self.bridge.pending_write("line1_value").is_some(),
            "point_detail": point_status.map(|status| status.detail.as_str()),
            "point_state": point_status.map(|status| state_name(status.state)),
            "recover_count": self.worker.transport().recover_count,
            "state": state_name(self.bridge.state()),
            "storage_milli": storage_milli,
            "subscribe_count": self.worker.transport().subscribe_count,
            "write_batches": self.worker.transport().write_batches.len(),
        })
    }
}

#[derive(Default)]
struct LifecycleTransport {
    connect_count: usize,
    subscribe_count: usize,
    recover_count: usize,
    recover_succeeds: bool,
    disconnect_count: usize,
    write_batches: Vec<Vec<(SmolStr, Value)>>,
    write_error: Option<OpcUaClientBridgeError>,
    sink: Option<OpcUaClientEventSink>,
}

impl LifecycleTransport {
    fn emit_sample(
        &mut self,
        point: &OpcUaClientPointConfig,
        value: Value,
        now_ms: u64,
    ) -> Result<(), String> {
        let sink = self
            .sink
            .as_ref()
            .ok_or_else(|| "OPC UA event sink is not connected".to_string())?;
        if !sink.publish_sample(OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: Some(value),
            state: OpcUaClientConnectionState::Connected,
            last_seen_ms: Some(now_ms),
            detail: "case subscription update".to_string(),
        }) {
            return Err("OPC UA sample event queue is full".to_string());
        }
        Ok(())
    }

    fn emit_connection_status(
        &mut self,
        connected: bool,
        now_ms: u64,
        detail: &str,
    ) -> Result<(), String> {
        let sink = self
            .sink
            .as_ref()
            .ok_or_else(|| "OPC UA event sink is not connected".to_string())?;
        if !sink.publish_connection_status(connected, now_ms, detail) {
            return Err("OPC UA connection event queue is full".to_string());
        }
        Ok(())
    }
}

impl OpcUaClientTransport for LifecycleTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError> {
        self.connect_count += 1;
        self.sink = Some(sink);
        Ok(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError> {
        if !points.is_empty() {
            self.subscribe_count += 1;
        }
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError> {
        if let Some(error) = self.write_error.clone() {
            return Err(error);
        }
        self.write_batches.push(
            values
                .iter()
                .map(|(point, value)| (point.var.clone(), value.clone()))
                .collect(),
        );
        Ok(())
    }

    fn recover_after_disconnect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        _read_points: &[OpcUaClientPointConfig],
        sink: OpcUaClientEventSink,
    ) -> Result<Option<OpcUaClientSessionInfo>, OpcUaClientBridgeError> {
        self.recover_count += 1;
        if !self.recover_succeeds {
            return Ok(None);
        }
        self.sink = Some(sink);
        Ok(Some(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: Some(
                "case reconnect transferred subscriptions with send_initial_values=true",
            ),
        }))
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError> {
        self.disconnect_count += 1;
        Ok(())
    }
}

fn compare_expected(
    step: &TraceStep,
    observed: &serde_json::Value,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (key, expected) in &step.expected {
        if let Some(field) = key.strip_suffix("_contains") {
            let needle = expected
                .as_str()
                .ok_or_else(|| format!("{key} must be a string"))?;
            let detail = observed[field].as_str().unwrap_or_default();
            if !detail.contains(needle) {
                mismatches.push(format!(
                    "step {} expected {field} containing {needle:?}, observed {detail:?}",
                    step.sequence
                ));
            }
            continue;
        }
        let expected_json = toml_to_json(expected)?;
        if observed.get(key) != Some(&expected_json) {
            mismatches.push(format!(
                "step {} expected {key}={expected_json}, observed {}",
                step.sequence,
                observed.get(key).unwrap_or(&serde_json::Value::Null)
            ));
        }
    }
    Ok(())
}

fn toml_to_json(value: &toml::Value) -> Result<serde_json::Value, String> {
    match value {
        toml::Value::String(value) => Ok(json!(value)),
        toml::Value::Integer(value) => Ok(json!(value)),
        toml::Value::Boolean(value) => Ok(json!(value)),
        _ => Err(format!("unsupported expected trace value {value:?}")),
    }
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be a string"))
}

fn trace_i64(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i64, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be an integer"))
}

fn trace_u64(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<u64, String> {
    let value = trace_i64(values, key)?;
    u64::try_from(value).map_err(|_| format!("trace field {key} must be non-negative"))
}

fn state_name(state: OpcUaClientConnectionState) -> &'static str {
    match state {
        OpcUaClientConnectionState::Disabled => "disabled",
        OpcUaClientConnectionState::Configured => "configured",
        OpcUaClientConnectionState::Connecting => "connecting",
        OpcUaClientConnectionState::Connected => "connected",
        OpcUaClientConnectionState::Reconnecting => "reconnecting",
        OpcUaClientConnectionState::Stale => "stale",
        OpcUaClientConnectionState::Faulted => "faulted",
    }
}

#[derive(Default)]
struct LifecycleProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for LifecycleProbe {
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
