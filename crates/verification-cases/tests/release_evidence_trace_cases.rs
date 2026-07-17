use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value as JsonValue};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

#[test]
fn release_platform_matrix_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_PLATFORM_MATRIX_TRACE_001",
        "verification/cases/release/RELEASE_PLATFORM_MATRIX_001.toml",
        "sha256:fa8499223637516b9eb061a8f37e8dc32fcd12d68000765c72f525fa1fec83ed",
    );
}

#[test]
fn release_source_build_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_SOURCE_BUILD_TRACE_001",
        "verification/cases/release/RELEASE_SOURCE_BUILD_OPENOT_001.toml",
        "sha256:e2699f838ac7c445caab0a4ce1871903bf57de9f2464620401797c5db38df834",
    );
}

#[test]
fn release_hardware_claim_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_HARDWARE_CLAIM_TRACE_001",
        "verification/cases/release/REL_CLAIM_001.toml",
        "sha256:6c43e4b61d6d7a28f1a3e862b2cdc3cad3c929cf89d49112222495d3bd11de01",
    );
}

#[test]
fn release_conformance_status_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_CONFORMANCE_STATUS_TRACE_001",
        "verification/cases/release/REL_CONF_001.toml",
        "sha256:49789ceeda69fc9124a1d6da8620bb6ea4e4170863c74fe5b2d0ca96f678e829",
    );
}

#[test]
fn release_version_chain_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_VERSION_CHAIN_TRACE_001",
        "verification/cases/release/REL_VERSION_001.toml",
        "sha256:8334f6490750e61a37dc64517cd50c0104bfdda4216c652db89ac8f7703bf8db",
    );
}

#[test]
fn runtime_behavior_lock_trace_cases() {
    run_release_trace(
        "TEST_RUNTIME_BEHAVIOR_LOCK_TRACE_001",
        "verification/cases/release/RUNTIME_BEHAVIOR_LOCKED_001.toml",
        "sha256:8cd784697fd75c26a2d037238aeda0245c77d156b015225dafd1abee01cba3c6",
    );
}

#[test]
fn debug_behavior_lock_trace_cases() {
    run_release_trace(
        "TEST_DEBUG_BEHAVIOR_LOCK_TRACE_001",
        "verification/cases/editor_safety/DEBUG_BEHAVIOR_LOCKED_001.toml",
        "sha256:c9f62e785579906ab8efd6891d5b06e972216e122627b6aca035c3545fa76072",
    );
}

#[test]
fn platform_path_trace_cases() {
    run_release_trace(
        "TEST_PLATFORM_PATH_TRACE_001",
        "verification/cases/supply_chain_platform/PLAT_PATH_001.toml",
        "sha256:d25db02f57666e284d1a75cc1f1ec178159dc7b2453ef975be956e92764e77b9",
    );
}

#[test]
fn vsix_target_identity_trace_cases() {
    run_release_trace(
        "TEST_VSIX_TARGET_IDENTITY_TRACE_001",
        "verification/cases/supply_chain_platform/PLAT_VSCODE_001.toml",
        "sha256:65aa3faf7937b1574cb95393a593d6e94fba977574001e5c9e8625d656a78cc2",
    );
}

#[test]
fn artifact_provenance_trace_cases() {
    run_release_trace(
        "TEST_ARTIFACT_PROVENANCE_TRACE_001",
        "verification/cases/supply_chain_platform/SEC_ARTIFACT_001.toml",
        "sha256:0007ac7f923e0a6f40d3cc47adb956b8eaf72f14d8f353d88db4b1fae00a3b38",
    );
}

#[test]
fn dependency_exception_trace_cases() {
    run_release_trace(
        "TEST_DEPENDENCY_EXCEPTION_TRACE_001",
        "verification/cases/supply_chain_platform/SEC_DEP_AUDIT_001.toml",
        "sha256:e2e2a445652fcad7a4c1de6e54390abc491cf3cd3eccad0e23822cd13afc4847",
    );
}

fn run_release_trace(test_id: &str, case_file: &str, case_file_digest: &str) {
    let workspace = workspace_root();
    let mut probe = ReleaseProbe::default();
    let config = RunConfig::new(test_id, workspace.join(case_file), case_file_digest);
    let artifact = run_case_file(&config, &mut probe, run_release_case)
        .expect("release-evidence artifact must be written");
    let failures = artifact
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
        failures.is_empty(),
        "release-evidence trace failures: {}",
        failures.join("; ")
    );
}

fn run_release_case(
    case: &CaseRecord,
    probe: &mut ReleaseProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let output = Command::new("python3")
        .args([
            "scripts/release_claim_contract.py",
            "--scenario",
            scenario,
        ])
        .env("TRUST_RELEASE_EVIDENCE_DATE", "2026-07-17")
        .current_dir(workspace_root())
        .output()
        .map_err(|error| format!("failed to run release claim contract: {error}"))?;
    let observed_error = if output.status.success() {
        None
    } else {
        Some(String::from_utf8_lossy(&output.stderr).trim().to_string())
    };
    if output.status.success() {
        let payload: JsonValue = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("release claim output was not JSON: {error}"))?;
        if payload.get("scenario").and_then(JsonValue::as_str) != Some(scenario) {
            return Err(format!("release claim output did not bind scenario {scenario}"));
        }
        probe.target = Some(payload);
    } else {
        probe.target = Some(json!({"scenario": scenario, "passed": false}));
    }
    Ok(CaseExecution {
        result: if observed_error.is_none() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error,
        observed_status: Some("release_contract_checked".to_string()),
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("verification-cases must be inside workspace/crates")
        .to_path_buf()
}

#[derive(Default)]
struct ReleaseProbe {
    target: Option<JsonValue>,
}

impl StateProbe for ReleaseProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.target.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
