use std::collections::BTreeMap;
use std::env;

use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const AUTHORIZATION_TEST_ID: &str = "TEST_CONTROL_AUTHORIZATION_TRACE_001";
const AUTHORIZATION_CASE_FILE: &str =
    "verification/cases/control_security/SEC_AUTHZ_001.toml";
const AUTHORIZATION_CASE_FILE_DIGEST: &str =
    "sha256:8357b5118a28f7d76ca0937f41c8c529719cd95bdcd018dc54c1a4561a35c4ed";

const AUTHORIZATION_SOURCE: &str = r#"
PROGRAM Main
VAR
    output_bit AT %QX0.0 : BOOL := FALSE;
END_VAR
output_bit := FALSE;
END_PROGRAM
"#;

#[test]
fn control_authorization_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("authorization runner must enter workspace root");

    let mut probe = AuthorizationProbe::default();
    let config = RunConfig::new(
        AUTHORIZATION_TEST_ID,
        AUTHORIZATION_CASE_FILE,
        AUTHORIZATION_CASE_FILE_DIGEST,
    );
    let result = run_case_file(&config, &mut probe, run_authorization_case);

    env::set_current_dir(original_dir).expect("authorization runner must restore current directory");
    let artifact = result.expect("authorization case artifact must be written");
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
        "authorization trace failures: {}",
        failed.join("; ")
    );
}

fn run_authorization_case(
    case: &CaseRecord,
    probe: &mut AuthorizationProbe,
) -> Result<CaseExecution, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }

    let step = &trace[0];
    let scenario = trace_string(&step.stimulus, "scenario")?;
    let observed = execute_authorization_scenario(&scenario)?;
    let mut mismatches = Vec::new();
    compare_expected(step, &observed, &mut mismatches)?;
    probe.observed = Some(serde_json::json!({ "trace": [observed] }));

    Ok(CaseExecution {
        result: if mismatches.is_empty() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!mismatches.is_empty()).then(|| mismatches.join("; ")),
        observed_status: Some(if mismatches.is_empty() {
            "authorization_contract_matched".to_string()
        } else {
            "authorization_contract_mismatch".to_string()
        }),
    })
}

fn execute_authorization_scenario(
    scenario: &str,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let mut state = hmi_test_state(AUTHORIZATION_SOURCE);
    state.auth_token = Arc::new(Mutex::new(Some(SmolStr::new("admin-token"))));
    state.control_requires_auth = true;
    state.debug_enabled.store(true, Ordering::Relaxed);

    let pairing_path = pairing_file(&format!("authorization-trace-{scenario}"));
    let store = Arc::new(PairingStore::load(pairing_path.clone()));
    state.pairing = Some(store.clone());
    let viewer_token = claim_role(&store, AccessRole::Viewer)?;
    let operator_token = claim_role(&store, AccessRole::Operator)?;
    let engineer_token = claim_role(&store, AccessRole::Engineer)?;

    let response = match scenario {
        "viewer_force_denied" => force_request(1, viewer_token.as_str(), &state),
        "operator_force_denied" => force_request(2, operator_token.as_str(), &state),
        "engineer_debug_activation_denied" => {
            state.debug_enabled.store(false, Ordering::Relaxed);
            handle_request_value(
                json!({
                    "id": 3,
                    "type": "config.set",
                    "auth": engineer_token,
                    "params": { "control.debug_enabled": true }
                }),
                &state,
                None,
            )
        }
        "admin_debug_activation_allowed" => {
            state.debug_enabled.store(false, Ordering::Relaxed);
            handle_request_value(
                json!({
                    "id": 4,
                    "type": "config.set",
                    "auth": "admin-token",
                    "params": { "control.debug_enabled": true }
                }),
                &state,
                None,
            )
        }
        "engineer_force_allowed" => force_request(5, engineer_token.as_str(), &state),
        "viewer_release_denied" => {
            let setup = force_request(6, engineer_token.as_str(), &state);
            if !setup.ok {
                return Err(format!("engineer force setup failed: {:?}", setup.error));
            }
            handle_request_value(
                json!({
                    "id": 7,
                    "type": "io.unforce",
                    "auth": viewer_token,
                    "params": { "address": "%QX0.0" }
                }),
                &state,
                None,
            )
        }
        "engineer_release_allowed" => {
            let setup = force_request(8, engineer_token.as_str(), &state);
            if !setup.ok {
                return Err(format!("engineer force setup failed: {:?}", setup.error));
            }
            handle_request_value(
                json!({
                    "id": 9,
                    "type": "io.unforce",
                    "auth": engineer_token,
                    "params": { "address": "%QX0.0" }
                }),
                &state,
                None,
            )
        }
        "unclassified_engineer_denied" => handle_request_value(
            json!({
                "id": 10,
                "type": "future.unclassified.operation",
                "auth": engineer_token,
                "params": {}
            }),
            &state,
            None,
        ),
        other => return Err(format!("unreviewed authorization scenario {other}")),
    };

    let observed = BTreeMap::from([
        ("ok".to_string(), json!(response.ok)),
        (
            "error_code".to_string(),
            json!(response.error_code.as_deref().unwrap_or("none")),
        ),
        (
            "debug_enabled".to_string(),
            json!(state.debug_enabled.load(Ordering::Relaxed)),
        ),
        (
            "forced_count".to_string(),
            json!(state.debug.forced_snapshot().io.len()),
        ),
    ]);
    let _ = fs::remove_file(pairing_path);
    Ok(observed)
}

fn claim_role(store: &PairingStore, role: AccessRole) -> Result<String, String> {
    let code = store.start_pairing();
    store
        .claim(&code.code, Some(role))
        .map(|token| token.to_string())
        .ok_or_else(|| format!("failed to claim {} token", role.as_str()))
}

fn force_request(id: u64, token: &str, state: &ControlState) -> ControlResponse {
    handle_request_value(
        json!({
            "id": id,
            "type": "io.force",
            "auth": token,
            "params": { "address": "%QX0.0", "value": "TRUE" }
        }),
        state,
        None,
    )
}

fn compare_expected(
    step: &TraceStep,
    observed: &BTreeMap<String, serde_json::Value>,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (field, expected) in &step.expected {
        let expected = toml_to_json(expected)?;
        if observed.get(field) != Some(&expected) {
            mismatches.push(format!(
                "step {} {field} expected {expected}, observed {}",
                step.sequence,
                observed
                    .get(field)
                    .map_or_else(|| "missing".to_string(), ToString::to_string)
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
        other => Err(format!("unsupported expected trace value {other:?}")),
    }
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be text"))
}

#[derive(Default)]
struct AuthorizationProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for AuthorizationProbe {
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
