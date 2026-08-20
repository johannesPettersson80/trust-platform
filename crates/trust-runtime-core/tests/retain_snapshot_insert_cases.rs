use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
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
    let expected = match scenario {
        "NEW_RESOLVED_RETAINED_NAME" => {
            probe.snapshot.insert("FIRST", Value::DInt(11));
            vec![("FIRST".to_string(), Value::DInt(11))]
        }
        "EXISTING_RESOLVED_RETAINED_NAME" => {
            probe.snapshot.insert("FIRST", Value::DInt(1));
            probe.snapshot.insert("SIBLING", Value::Bool(true));
            probe
                .snapshot
                .insert("FIRST", Value::String("replacement".into()));
            vec![
                ("FIRST".to_string(), Value::String("replacement".into())),
                ("SIBLING".to_string(), Value::Bool(true)),
            ]
        }
        "DISTINCT_RESOLVED_RETAINED_NAME" => {
            probe.snapshot.insert("Motor", Value::DInt(1));
            probe.snapshot.insert("MOTOR", Value::UDInt(7));
            vec![
                ("Motor".to_string(), Value::DInt(1)),
                ("MOTOR".to_string(), Value::UDInt(7)),
            ]
        }
        other => {
            return Err(format!(
                "unreviewed retain snapshot insertion scenario {other}"
            ))
        }
    };
    probe.revision += 1;
    let observed = probe
        .snapshot
        .values()
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect::<Vec<_>>();
    let failures = (observed != expected)
        .then(|| format!("expected {expected:?}, observed {observed:?}"))
        .into_iter()
        .collect::<Vec<_>>();
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
