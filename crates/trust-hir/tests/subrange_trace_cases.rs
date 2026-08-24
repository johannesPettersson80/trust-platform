mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::check_errors;
use trust_hir::diagnostics::DiagnosticCode;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_IEC_SUBRANGE_TRACE_001";
const CASE_FILE: &str = "verification/cases/compiler_iec/IEC_SUBRANGE_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:fb5022e07d1db17f6939f21703c593e5231d2a9fe90592c4eaec4292803b0b51";

#[test]
fn subrange_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-hir must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = DiagnosticProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_subrange_case)
        .expect("subrange case artifact must be written");
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
        "subrange failures: {}",
        failed.join("; ")
    );
}

fn run_subrange_case(
    case: &CaseRecord,
    probe: &mut DiagnosticProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (source, expected) = match scenario {
        "BELOW_LOWER_BOUND_INITIALIZER" => (
            "TYPE Limited : INT (-2..2); END_TYPE\nPROGRAM Main\nVAR below : Limited := -3; END_VAR\nEND_PROGRAM",
            DiagnosticCode::OutOfRange,
        ),
        "ABOVE_UPPER_BOUND_INITIALIZER" => (
            "TYPE Limited : INT (-2..2); END_TYPE\nPROGRAM Main\nVAR above : Limited := 3; END_VAR\nEND_PROGRAM",
            DiagnosticCode::OutOfRange,
        ),
        "REAL_TO_INT_SUBRANGE" => (
            "TYPE Limited : INT (-2..2); END_TYPE\nPROGRAM Main\nVAR target : Limited; wrong : REAL; END_VAR\ntarget := wrong;\nEND_PROGRAM",
            DiagnosticCode::IncompatibleAssignment,
        ),
        other => return Err(format!("unreviewed subrange scenario {other}")),
    };
    let diagnostics = check_errors(source);
    probe.diagnostics = diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect();
    let expected_count = diagnostics.iter().filter(|code| **code == expected).count();
    let passed = expected_count == 1;
    Ok(CaseExecution {
        result: if passed {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!passed).then(|| {
            format!("{scenario} expected exactly one {expected:?}; observed {diagnostics:?}")
        }),
        observed_status: Some(
            if passed {
                "diagnostic_visible"
            } else {
                "subrange_contract_mismatch"
            }
            .to_string(),
        ),
    })
}

#[derive(Default)]
struct DiagnosticProbe {
    diagnostics: Vec<String>,
    next_snapshot_is_after: bool,
}

impl StateProbe for DiagnosticProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_after {
            self.diagnostics.clear();
        }
        self.next_snapshot_is_after = !self.next_snapshot_is_after;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: None,
            siblings: BTreeMap::new(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}
