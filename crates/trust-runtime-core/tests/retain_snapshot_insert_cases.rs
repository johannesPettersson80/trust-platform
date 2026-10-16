use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value;
use verification_cases::{
    check_case_expectation, check_trace_fields, run_case_file, CaseExecution, CaseRecord,
    CaseResult, ObservedOperation, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_RUNTIME_RETAIN_SNAPSHOT_INSERT_TRACE_001";
const CASE_FILE: &str = "verification/cases/runtime_safety/RT_RETAIN_SNAPSHOT_INSERT_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:b834b8923ac48e8f74660a88a4031d88653a35fde1c567aaa5ffb558c61bd097";

#[test]
fn retain_snapshot_insert_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime-core must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = RetainSnapshotProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_snapshot_case)
        .expect("retain snapshot insertion artifact must be written");
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
        "retain snapshot insertion failures: {}",
        failed.join("; ")
    );
}

fn run_snapshot_case(
    case: &CaseRecord,
    probe: &mut RetainSnapshotProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    probe.snapshot = RetainSnapshot::default();
    // Each scenario ends with one insertion, the operation under test. `before` is the
    // retained state just before that insertion and `target` is the name it writes.
    let (before, target, expected) = match scenario {
        "NEW_RESOLVED_RETAINED_NAME" => {
            let before = retained_entries(&probe.snapshot);
            probe.snapshot.insert("FIRST", Value::DInt(11));
            (
                before,
                "FIRST",
                vec![("FIRST".to_string(), Value::DInt(11))],
            )
        }
        "EXISTING_RESOLVED_RETAINED_NAME" => {
            probe.snapshot.insert("FIRST", Value::DInt(1));
            probe.snapshot.insert("SIBLING", Value::Bool(true));
            let before = retained_entries(&probe.snapshot);
            probe
                .snapshot
                .insert("FIRST", Value::String("replacement".into()));
            (
                before,
                "FIRST",
                vec![
                    ("FIRST".to_string(), Value::String("replacement".into())),
                    ("SIBLING".to_string(), Value::Bool(true)),
                ],
            )
        }
        "DISTINCT_RESOLVED_RETAINED_NAME" => {
            probe.snapshot.insert("Motor", Value::DInt(1));
            let before = retained_entries(&probe.snapshot);
            probe.snapshot.insert("MOTOR", Value::UDInt(7));
            (
                before,
                "MOTOR",
                vec![
                    ("Motor".to_string(), Value::DInt(1)),
                    ("MOTOR".to_string(), Value::UDInt(7)),
                ],
            )
        }
        other => {
            return Err(format!(
                "unreviewed retain snapshot insertion scenario {other}"
            ))
        }
    };
    probe.revision += 1;
    let observed = retained_entries(&probe.snapshot);
    let mut failures = (observed != expected)
        .then(|| format!("expected {expected:?}, observed {observed:?}"))
        .into_iter()
        .collect::<Vec<_>>();
    // The insertion is accepted (it returns no error) and reports no diagnostic.
    let before_state = retained_state(&before, target);
    let after_state = retained_state(&observed, target);
    let operation = ObservedOperation {
        outcome: "accept_value",
        fault_surface: None,
        status_before: None,
        status_after: None,
        before: &before_state,
        after: &after_state,
    };
    match check_case_expectation(case, &operation) {
        Ok(mismatches) => failures.extend(mismatches),
        Err(error) => failures.push(error),
    }
    for trace_step in case.trace.as_deref().unwrap_or(&[]) {
        if let Err(error) = check_trace_fields(&trace_step.expected, &[]) {
            failures.push(error);
        }
    }
    Ok(CaseExecution {
        result: if failures.is_empty() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!failures.is_empty()).then(|| failures.join("; ")),
        observed_status: Some(if failures.is_empty() {
            format!("{scenario}:contract_matched")
        } else {
            format!("{scenario}:contract_mismatch")
        }),
    })
}

/// The retained entries in insertion order, as (name, value) pairs.
fn retained_entries(snapshot: &RetainSnapshot) -> Vec<(String, Value)> {
    snapshot
        .values()
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect::<Vec<_>>()
}

/// The retained state on one side of an insertion. The written name is the target; the other
/// entries are siblings, which an insertion of a new name must leave unchanged.
fn retained_state(entries: &[(String, Value)], target: &str) -> StateSnapshot {
    let target_value = entries
        .iter()
        .find(|(name, _)| name.as_str() == target)
        .map(|(name, value)| format!("{name}={value:?}"));
    StateSnapshot {
        process_image_hash: None,
        retain_hash: Some(format!("{entries:?}")),
        target: target_value.map(|text| text.into()),
        siblings: entries
            .iter()
            .filter(|(name, _)| name.as_str() != target)
            .map(|(name, value)| (name.clone(), format!("{value:?}").into()))
            .collect(),
        diagnostics: Vec::new(),
    }
}

#[derive(Default)]
struct RetainSnapshotProbe {
    snapshot: RetainSnapshot,
    revision: u64,
}

impl StateProbe for RetainSnapshotProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: Some(format!("{}:{:?}", self.revision, self.snapshot.values())),
            target: None,
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
