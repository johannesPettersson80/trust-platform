use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value as JsonValue;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

pub(super) fn run_trace_cases<F>(
    test_id: &str,
    case_file: &str,
    case_file_digest: &str,
    mut execute: F,
) where
    F: FnMut(&CaseRecord, &TraceStep) -> Result<JsonValue, String>,
{
    let mut probe = JsonProbe::default();
    let config = RunConfig::new(test_id, workspace_root().join(case_file), case_file_digest);
    let artifact = run_case_file(&config, &mut probe, |case, probe| {
        let trace = case
            .trace
            .as_deref()
            .ok_or_else(|| format!("{} has no trace", case.id))?;
        let mut mismatches = Vec::new();
        for step in trace {
            let observed = execute(case, step)?;
            compare_expected(step, &observed, &mut mismatches)?;
            probe.observed = Some(observed);
        }
        Ok::<_, String>(case_execution(mismatches))
    })
    .expect("trace artifact must be written");

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
    assert!(failed.is_empty(), "trace failures: {}", failed.join("; "));
}

pub(super) fn scenario(case: &CaseRecord) -> Result<&str, String> {
    case.input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))
}

pub(super) fn trace_string(step: &TraceStep, key: &str) -> Result<String, String> {
    step.stimulus
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace stimulus {key} must be a string"))
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

fn case_execution(mismatches: Vec<String>) -> CaseExecution {
    if mismatches.is_empty() {
        CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("trace_passed".to_string()),
        }
    } else {
        CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("trace_mismatch".to_string()),
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-lsp must be inside the workspace crates directory")
        .to_path_buf()
}

#[derive(Default)]
struct JsonProbe {
    observed: Option<JsonValue>,
    next_snapshot_is_before: bool,
}

impl StateProbe for JsonProbe {
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
