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
        "sha256:d30f9c141a13163f11cfd996b1c91f0aac9f1803e902897167773159a37dc7e6",
    );
}

#[test]
fn release_source_build_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_SOURCE_BUILD_TRACE_001",
        "verification/cases/release/RELEASE_SOURCE_BUILD_OPENOT_001.toml",
        "sha256:a65a72f8c8e3d6530edd8a578b991a4df624e0ff3113c045163d78ac195eeb3d",
    );
}

#[test]
fn release_hardware_claim_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_HARDWARE_CLAIM_TRACE_001",
        "verification/cases/release/REL_CLAIM_001.toml",
        "sha256:3d59e7839f25710ebcf18af490e7a1d42dc08d478c9b20992696a6964751f9af",
    );
}

#[test]
fn release_conformance_status_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_CONFORMANCE_STATUS_TRACE_001",
        "verification/cases/release/REL_CONF_001.toml",
        "sha256:62387bc468a5c2fd6b336c2d13b56418f9d338efec1b551c91b1819912cd3361",
    );
}

#[test]
fn release_version_chain_trace_cases() {
    run_release_trace(
        "TEST_RELEASE_VERSION_CHAIN_TRACE_001",
        "verification/cases/release/REL_VERSION_001.toml",
        "sha256:02f9e52d5aa10ad0cc99f1f6e1e7afdc0ad8d02eabf6d5a10f4462abc0287c80",
    );
}

#[test]
fn runtime_behavior_lock_trace_cases() {
    run_release_trace(
        "TEST_RUNTIME_BEHAVIOR_LOCK_TRACE_001",
        "verification/cases/release/RUNTIME_BEHAVIOR_LOCKED_001.toml",
        "sha256:a9191accaa6526929c3bc9d9a504787a42fbff514c694baf9a8f84fb357c3040",
    );
}

#[test]
fn debug_behavior_lock_trace_cases() {
    run_release_trace(
        "TEST_DEBUG_BEHAVIOR_LOCK_TRACE_001",
        "verification/cases/editor_safety/DEBUG_BEHAVIOR_LOCKED_001.toml",
        "sha256:a4629fce647fc2a1c0badfac29b009a8188857737308bb9f428203fd394dfc34",
    );
}

#[test]
fn platform_path_trace_cases() {
    run_release_trace(
        "TEST_PLATFORM_PATH_TRACE_001",
        "verification/cases/supply_chain_platform/PLAT_PATH_001.toml",
        "sha256:ce4e07acfcde20fc7a588ceb83f67092989060072bc6b23f70e91257e3ce7747",
    );
}

#[test]
fn vsix_target_identity_trace_cases() {
    run_release_trace(
        "TEST_VSIX_TARGET_IDENTITY_TRACE_001",
        "verification/cases/supply_chain_platform/PLAT_VSCODE_001.toml",
        "sha256:ae59d5885d4205ff89f2b959c9d83369abda4ea5e00ee07d7378e84a5854ee0c",
    );
}

#[test]
fn artifact_provenance_trace_cases() {
    run_release_trace(
        "TEST_ARTIFACT_PROVENANCE_TRACE_001",
        "verification/cases/supply_chain_platform/SEC_ARTIFACT_001.toml",
        "sha256:a4496e67ca1af802f38bbd5dd11939c0ce5d0574aeeda7a0e4fdc264588bdfd6",
    );
}

#[test]
fn dependency_exception_trace_cases() {
    run_release_trace(
        "TEST_DEPENDENCY_EXCEPTION_TRACE_001",
        "verification/cases/supply_chain_platform/SEC_DEP_AUDIT_001.toml",
        "sha256:686b42082700cefc5da1d5b0a69b90678a77d6d37ff974942151e2cb8d53202b",
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

fn run_release_case(case: &CaseRecord, probe: &mut ReleaseProbe) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let output = Command::new(python_program())
        .args(["scripts/release_claim_contract.py", "--scenario", scenario])
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
            return Err(format!(
                "release claim output did not bind scenario {scenario}"
            ));
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

fn python_program() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
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
