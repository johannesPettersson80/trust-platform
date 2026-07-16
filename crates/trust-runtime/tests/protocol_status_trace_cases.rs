use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value as JsonValue};
use trust_runtime::ads::diagnostics::AdsConnectionStatusState;
use trust_runtime::ads::AdsConnectionState;
use trust_runtime::connectors::{
    ads_connection_state_status, ads_connection_status_state, opcua_client_status,
    opcua_server_snapshot_status, ConnectorHealth, ConnectorKind, ConnectorPointMetadata,
    ConnectorPointStatus, ConnectorProtocol, ConnectorState, ConnectorStatusBuilder,
    OpcUaServerSnapshotState, PointDirection, PointQuality,
};
use trust_runtime::opcua::OpcUaClientConnectionState;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_CONNECTOR_STATUS_TRUTH_TRACE_001";
const CASE_FILE: &str = "verification/cases/protocols/PROTO_STATUS_TRUTH_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:39e449f0b4495f45a700d9a3ef2b30d5c02a1dd13b7220113bdd08e22b5b3a16";

#[test]
fn connector_status_truth_trace_cases() {
    let workspace = workspace_root();
    let mut probe = StatusProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_status_case)
        .expect("connector-status artifact must be written");
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
        "connector-status trace failures: {}",
        failed.join("; ")
    );
}

fn run_status_case(case: &CaseRecord, probe: &mut StatusProbe) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no status trace", case.id))?;
    let mut mismatches = Vec::new();
    for step in trace {
        let observed = execute_step(step)?;
        compare_expected(step, &observed, &mut mismatches)?;
        probe.observed = Some(observed);
    }
    Ok(case_execution(mismatches, "connector_status"))
}

fn execute_step(step: &TraceStep) -> Result<JsonValue, String> {
    let action = trace_string(&step.stimulus, "action")?;
    match action.as_str() {
        "ads_source" => projection_json(ads_connection_state_status(
            match trace_string(&step.stimulus, "state")?.as_str() {
                "disconnected" => AdsConnectionState::Disconnected,
                "connecting" => AdsConnectionState::Connecting,
                "connected" => AdsConnectionState::Connected,
                "reconnecting" => AdsConnectionState::Reconnecting,
                "faulted" => AdsConnectionState::Faulted,
                state => return Err(format!("unreviewed ADS source state {state}")),
            },
        )),
        "ads_report" => projection_json(ads_connection_status_state(
            match trace_string(&step.stimulus, "state")?.as_str() {
                "connected" => AdsConnectionStatusState::Connected,
                "reconnecting" => AdsConnectionStatusState::Reconnecting,
                "not_ready" => AdsConnectionStatusState::NotReady,
                "faulted" => AdsConnectionStatusState::Faulted,
                "stale" => AdsConnectionStatusState::Stale,
                "disabled" => AdsConnectionStatusState::Disabled,
                "unknown" => AdsConnectionStatusState::Unknown,
                state => return Err(format!("unreviewed ADS report state {state}")),
            },
            trace_usize(&step.stimulus, "degraded_points")?,
        )),
        "opcua_client" => projection_json(opcua_client_status(
            match trace_string(&step.stimulus, "state")?.as_str() {
                "disabled" => OpcUaClientConnectionState::Disabled,
                "configured" => OpcUaClientConnectionState::Configured,
                "connecting" => OpcUaClientConnectionState::Connecting,
                "connected" => OpcUaClientConnectionState::Connected,
                "reconnecting" => OpcUaClientConnectionState::Reconnecting,
                "stale" => OpcUaClientConnectionState::Stale,
                "faulted" => OpcUaClientConnectionState::Faulted,
                state => return Err(format!("unreviewed OPC UA client state {state}")),
            },
            trace_usize(&step.stimulus, "degraded_points")?,
        )),
        "opcua_server" => projection_json(opcua_server_snapshot_status(
            match trace_string(&step.stimulus, "state")?.as_str() {
                "disabled" => OpcUaServerSnapshotState::Disabled,
                "starting" => OpcUaServerSnapshotState::Starting,
                "no_snapshot" => OpcUaServerSnapshotState::NoSnapshot,
                "snapshot_ready" => OpcUaServerSnapshotState::SnapshotReady,
                "faulted" => OpcUaServerSnapshotState::Faulted,
                state => return Err(format!("unreviewed OPC UA server state {state}")),
            },
        )),
        "point_stale" => {
            let report = ConnectorStatusBuilder::new(
                "status-trace",
                ConnectorProtocol::Ads,
                ConnectorKind::SupervisoryClient,
                ConnectorState::Degraded,
                ConnectorHealth::Degraded,
            )
            .points(vec![ConnectorPointStatus {
                metadata: ConnectorPointMetadata {
                    name: "stale_point".to_string(),
                    source: Some("MAIN.Stale".to_string()),
                    data_type: Some("REAL".to_string()),
                    direction: PointDirection::Read,
                },
                quality: PointQuality::Stale,
                last_update_ms: Some(1),
                detail: Some("freshness deadline exceeded".to_string()),
            }])
            .build();
            Ok(json!({
                "state": report.state,
                "health": report.health,
                "point_quality": report.points[0].quality,
                "degraded_points": report.point_counts.degraded,
            }))
        }
        other => Err(format!("unreviewed connector-status action {other}")),
    }
}

fn projection_json(
    projection: trust_runtime::connectors::ConnectorStatusProjection,
) -> Result<JsonValue, String> {
    Ok(json!({"state": projection.state, "health": projection.health}))
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

fn trace_usize(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<usize, String> {
    values
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("trace {key} must be a non-negative integer"))
}

fn case_execution(mismatches: Vec<String>, prefix: &str) -> CaseExecution {
    if mismatches.is_empty() {
        CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some(format!("{prefix}_trace_passed")),
        }
    } else {
        CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some(format!("{prefix}_trace_mismatch")),
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
struct StatusProbe {
    observed: Option<JsonValue>,
    next_snapshot_is_before: bool,
}

impl StateProbe for StatusProbe {
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
