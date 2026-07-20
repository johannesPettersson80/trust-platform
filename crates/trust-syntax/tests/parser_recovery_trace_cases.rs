use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use trust_syntax::{parser::parse, SyntaxKind};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const TEST_ID: &str = "TEST_IEC_PARSER_RECOVERY_TRACE_001";
const CASE_FILE: &str = "verification/cases/compiler_iec/IEC_PARSE_RECOVER_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:249d695260024e2d85615721af523ff57d80be2673791680c3d1b4d41823e040";

#[test]
fn parser_recovery_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-syntax must be inside the workspace crates directory")
        .to_path_buf();
    let mut probe = ParserProbe::default();
    let config = RunConfig::new(TEST_ID, workspace.join(CASE_FILE), CASE_FILE_DIGEST);
    let artifact = run_case_file(&config, &mut probe, run_parser_case)
        .expect("parser recovery case artifact must be written");
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
        "parser recovery failures: {}",
        failed.join("; ")
    );
}

fn run_parser_case(case: &CaseRecord, probe: &mut ParserProbe) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let (source, expected_diagnostic) = parser_fixture(scenario)?;
    let parsed = parse(&source);
    let diagnostics = parsed
        .errors()
        .iter()
        .map(|error| error.message.clone())
        .collect::<Vec<_>>();
    probe.diagnostics = diagnostics.clone();

    let expected_visible = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains(expected_diagnostic));
    let structure_preserved = if scenario == "MISSING_INNER_TERMINATOR_AT_OUTER_BOUNDARY" {
        diagnostics.iter().all(|diagnostic| {
            diagnostic != "expected END_IF" && diagnostic != "expected END_PROGRAM"
        }) && parsed
            .syntax()
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::AssignStmt)
            .count()
            == 2
    } else {
        true
    };
    let passed = !parsed.ok() && expected_visible && structure_preserved;
    Ok(CaseExecution {
        result: if passed {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: (!passed).then(|| {
            format!(
                "{scenario} expected failed parse containing {expected_diagnostic:?}; diagnostics={diagnostics:?}, structure_preserved={structure_preserved}"
            )
        }),
        observed_status: Some(if passed {
            "diagnostic_visible"
        } else {
            "recovery_contract_mismatch"
        }
        .to_string()),
    })
}

fn parser_fixture(scenario: &str) -> Result<(String, &'static str), String> {
    let wrapped =
        |body: &str| format!("PROGRAM Test\nVAR x, i : INT; END_VAR\n{body}\nEND_PROGRAM");
    let fixture = match scenario {
        "CASE_MISSING_OF" => (wrapped("CASE x\n1: x := 2;\nEND_CASE"), "expected OF"),
        "CASE_LABEL_MISSING_COLON" => (
            wrapped("CASE x OF\n1 x := 2;\nEND_CASE"),
            "expected ':' after CASE label",
        ),
        "ELSIF_MISSING_THEN" => (
            wrapped("IF x = 0 THEN\nx := 1;\nELSIF x = 1\nx := 2;\nEND_IF"),
            "expected THEN",
        ),
        "FOR_MISSING_CONTROL_VARIABLE" => (
            wrapped("FOR := 0 TO 2 DO\nx := x + 1;\nEND_FOR"),
            "expected FOR control variable",
        ),
        "FOR_MISSING_ASSIGNMENT" => (
            wrapped("FOR i TO 2 DO\nx := x + 1;\nEND_FOR"),
            "expected ':=' after FOR control variable",
        ),
        "FOR_MISSING_TO" => (
            wrapped("FOR i := 0 DO\nx := x + 1;\nEND_FOR"),
            "expected TO",
        ),
        "FOR_MISSING_DO" => (
            wrapped("FOR i := 0 TO 2\nx := x + 1;\nEND_FOR"),
            "expected DO",
        ),
        "WHILE_MISSING_DO" => (
            wrapped("WHILE x < 2\nx := x + 1;\nEND_WHILE"),
            "expected DO",
        ),
        "REPEAT_MISSING_UNTIL" => (
            wrapped("REPEAT\nx := x + 1;\nEND_REPEAT"),
            "expected UNTIL",
        ),
        "MISSING_INNER_TERMINATOR_AT_OUTER_BOUNDARY" => (
            "PROGRAM Test\nVAR x : INT; END_VAR\nIF x = 0 THEN\n    WHILE x < 2 DO\n        x := x + 1;\nEND_IF\nx := 7;\nEND_PROGRAM".to_string(),
            "expected END_WHILE",
        ),
        "MISSING_POU_TERMINATOR_AT_EOF" => (
            "PROGRAM Test\n    x := 1;\n".to_string(),
            "expected END_PROGRAM",
        ),
        "UNKNOWN_TOKEN" => (
            "PROGRAM Test\n    @@@ invalid @@@\nEND_PROGRAM".to_string(),
            "expected statement",
        ),
        "UNCLOSED_CALL_DELIMITER" => (
            "PROGRAM Test\n    x := MyFunc(1, 2;\nEND_PROGRAM".to_string(),
            "expected )",
        ),
        "DEEP_UNARY_NESTING" => (
            format!("PROGRAM Test\n    x := {}1;\nEND_PROGRAM", "+".repeat(2048)),
            "expression nesting exceeds parser limit",
        ),
        "DEEP_PAREN_NESTING" => (
            format!(
                "PROGRAM Test\n    x := {}1{};\nEND_PROGRAM",
                "(".repeat(1500),
                ")".repeat(1500)
            ),
            "expression nesting exceeds parser limit",
        ),
        other => return Err(format!("unreviewed parser recovery scenario {other}")),
    };
    Ok(fixture)
}

#[derive(Default)]
struct ParserProbe {
    diagnostics: Vec<String>,
    next_snapshot_is_after: bool,
}

impl StateProbe for ParserProbe {
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
