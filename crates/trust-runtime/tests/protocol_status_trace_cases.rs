use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value as JsonValue};
use trust_runtime::ads::diagnostics::AdsConnectionStatusState;
use trust_runtime::ads::AdsConnectionState;
use trust_runtime::connectors::{
    ads_connection_state_status, ads_connection_status_state, opcua_client_status,
    opcua_server_snapshot_status, ConnectorHealth, ConnectorKind, ConnectorPointMetadata,
    ConnectorPointStatus, ConnectorProtocol, ConnectorState, ConnectorStatusBuilder,
    DiscoveryConfidence, OpcUaServerSnapshotState, PointDirection, PointQuality,
};
use trust_runtime::opcua::OpcUaClientConnectionState;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_CONNECTOR_STATUS_TRUTH_TRACE_001";
const CASE_FILE: &str = "verification/cases/protocols/PROTO_STATUS_TRUTH_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:621664ed187d39e300c8f42e8638ac814fc90ef3cd4780c7358e87c0eff7e179";
const UI_TEST_ID: &str = "TEST_UI_CONNECTOR_STATUS_TRACE_001";
const UI_CASE_FILE: &str = "verification/cases/hmi_ui/UI_STATUS_001.toml";
const UI_CASE_FILE_DIGEST: &str =
    "sha256:c2a73e3b0ab74c82ca3cc0ee27f1f8b79f0c4852e4b2bbf480dbf38d2add09af";

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

#[test]
fn ui_connector_status_trace_cases() {
    let workspace = workspace_root();
    let mut probe = StatusProbe::default();
    let config = RunConfig::new(
        UI_TEST_ID,
        workspace.join(UI_CASE_FILE),
        UI_CASE_FILE_DIGEST,
    );
    let artifact = run_case_file(&config, &mut probe, run_ui_status_case)
        .expect("UI connector-status artifact must be written");
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
        "UI status trace failures: {}",
        failed.join("; ")
    );
}

fn run_ui_status_case(case: &CaseRecord, probe: &mut StatusProbe) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (observed, mismatch) = match scenario {
        "CANONICAL_STATUS_VOCABULARY" => {
            let states = [
                ConnectorState::Disabled,
                ConnectorState::Configured,
                ConnectorState::Starting,
                ConnectorState::Ready,
                ConnectorState::Degraded,
                ConnectorState::Reconnecting,
                ConnectorState::Stale,
                ConnectorState::NotReady,
                ConnectorState::Faulted,
            ];
            let health = [
                ConnectorHealth::Ok,
                ConnectorHealth::Degraded,
                ConnectorHealth::Faulted,
                ConnectorHealth::Unknown,
            ];
            let value = json!({"states": states, "health": health});
            let expected = json!({
                "states": ["disabled", "configured", "starting", "ready", "degraded", "reconnecting", "stale", "not_ready", "faulted"],
                "health": ["ok", "degraded", "faulted", "unknown"],
            });
            (
                value.clone(),
                (value != expected).then(|| format!("{value}")),
            )
        }
        "FRESHNESS_AND_DETAIL_PRESERVATION" => {
            let report = ConnectorStatusBuilder::new(
                "ui-status",
                ConnectorProtocol::Ads,
                ConnectorKind::SupervisoryClient,
                ConnectorState::Degraded,
                ConnectorHealth::Degraded,
            )
            .confidence(DiscoveryConfidence::Likely)
            .freshness_ms(0)
            .last_error("read timeout")
            .points(vec![ConnectorPointStatus {
                metadata: ConnectorPointMetadata {
                    name: "stale".to_string(),
                    source: None,
                    data_type: Some("REAL".to_string()),
                    direction: PointDirection::Read,
                },
                quality: PointQuality::Stale,
                last_update_ms: Some(1),
                detail: Some("freshness deadline exceeded".to_string()),
            }])
            .build();
            let configured = ConnectorStatusBuilder::new(
                "configured",
                ConnectorProtocol::Ads,
                ConnectorKind::SupervisoryClient,
                ConnectorState::Configured,
                ConnectorHealth::Unknown,
            )
            .build();
            let value = json!({
                "state": report.state,
                "health": report.health,
                "freshness_ms": report.freshness_ms,
                "last_error": report.last_error,
                "point_quality": report.points[0].quality,
                "missing_freshness_state": configured.state,
            });
            let passed = report.freshness_ms == Some(0)
                && report.last_error.as_deref() == Some("read timeout")
                && report.points[0].quality == PointQuality::Stale
                && configured.freshness_ms.is_none()
                && configured.state == ConnectorState::Configured;
            (
                value,
                (!passed).then(|| "status detail was not preserved".to_string()),
            )
        }
        "UNKNOWN_STATUS_IDENTIFIER" => {
            let rejected = serde_json::from_str::<ConnectorState>("\"invented_healthy\"").is_err();
            let value = json!({"accepted": !rejected, "rendered_healthy": false, "diagnostic_visible": rejected});
            (
                value,
                (!rejected).then(|| "unknown state was accepted".to_string()),
            )
        }
        other => return Err(format!("unreviewed UI status scenario {other}")),
    };
    probe.observed = Some(observed);
    Ok(CaseExecution {
        result: if mismatch.is_none() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: mismatch,
        observed_status: Some("ui_status_contract_checked".to_string()),
    })
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
