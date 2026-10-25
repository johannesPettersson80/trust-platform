//! The `trust-dev test` command: discovery, the compiled run of the tests and the reports. The tests
//! that run ST tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use super::*;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const DISCOVERY_TRACE_TEST_ID: &str = "TEST_DEV_TEST_DISCOVERY_TRACE_001";
const DISCOVERY_TRACE_CASE_FILE: &str =
    "verification/cases/plcopen_devtools/DEV_TEST_DISCOVERY_001.toml";
const DISCOVERY_TRACE_CASE_DIGEST: &str =
    "sha256:d217e40fa2ade0354fd1ad5bda019650d382c05eda109b8dbc74d76b3bfb0cff";

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        if chars.next_if_eq(&'[').is_none() {
            continue;
        }

        for control in chars.by_ref() {
            if control.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[test]
fn discovery_finds_test_pous_with_namespace_qualification() {
    let sources = vec![
        LoadedSource {
            path: PathBuf::from("b.st"),
            text: r#"
TEST_PROGRAM Plain
END_TEST_PROGRAM
"#
            .to_string(),
        },
        LoadedSource {
            path: PathBuf::from("a.st"),
            text: r#"
NAMESPACE NS.Core
TEST_FUNCTION_BLOCK CaseOne
END_TEST_FUNCTION_BLOCK
END_NAMESPACE
"#
            .to_string(),
        },
    ];

    let discovered = discover_tests(&sources);
    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].name, "CaseOne");
    assert_eq!(discovered[0].kind, TestKind::FunctionBlock);
    assert_eq!(
        discovered[0].source_line.as_deref(),
        Some("TEST_FUNCTION_BLOCK CaseOne")
    );
    assert_eq!(discovered[1].name, "Plain");
    assert_eq!(discovered[1].kind, TestKind::Program);
    assert_eq!(
        discovered[1].source_line.as_deref(),
        Some("TEST_PROGRAM Plain")
    );
}

#[test]
fn discovery_ignores_comments_after_test_name() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("comments.st"),
        text: r#"
TEST_PROGRAM InlineComment (* inline comment *)
END_TEST_PROGRAM

TEST_PROGRAM NextLineComment
(* line comment right after declaration *)
END_TEST_PROGRAM
"#
        .to_string(),
    }];

    let discovered = discover_tests(&sources);
    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].name, "InlineComment");
    assert_eq!(discovered[1].name, "NextLineComment");
}

#[test]
fn load_sources_finds_mixed_case_extensions_under_literal_glob_chars() {
    let root = unique_temp_dir("discovery-[literal-root]");
    std::fs::create_dir_all(root.join("Nested")).expect("create source tree");
    std::fs::write(
        root.join("Motor.St"),
        "TEST_PROGRAM MotorCase\nEND_TEST_PROGRAM\n",
    )
    .expect("write mixed-case st file");
    std::fs::write(
        root.join("Nested").join("Library.Pou"),
        "TEST_PROGRAM LibraryCase\nEND_TEST_PROGRAM\n",
    )
    .expect("write mixed-case pou file");
    std::fs::write(
        root.join("Ignored.txt"),
        "TEST_PROGRAM Ignored\nEND_TEST_PROGRAM\n",
    )
    .expect("write ignored file");

    let sources = load_sources(&root).expect("load sources");
    let mut names = sources
        .iter()
        .map(|source| {
            source
                .path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, vec!["Library.Pou", "Motor.St"]);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn developer_test_discovery_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-dev must be inside workspace/crates")
        .to_path_buf();
    let mut probe = DiscoveryTraceProbe::default();
    let config = RunConfig::new(
        DISCOVERY_TRACE_TEST_ID,
        workspace.join(DISCOVERY_TRACE_CASE_FILE),
        DISCOVERY_TRACE_CASE_DIGEST,
    );
    let artifact = run_case_file(&config, &mut probe, run_discovery_trace_case)
        .expect("discovery artifact must be written");
    let failures = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            case.observed_error
                .clone()
                .unwrap_or_else(|| case.id.clone())
        })
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "discovery failures: {}",
        failures.join("; ")
    );
}

fn run_discovery_trace_case(
    case: &CaseRecord,
    probe: &mut DiscoveryTraceProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} scenario must be a string", case.id))?;
    let root = unique_temp_dir("trace-discovery-[Ω]");
    std::fs::create_dir_all(root.join("deep")).map_err(|error| error.to_string())?;
    let failure = match scenario {
        "SUPPORTED_EXTENSION_CASES" => {
            for (path, name) in [
                ("A.st", "A"),
                ("B.St", "B"),
                ("deep/C.sT", "C"),
                ("deep/D.ST", "D"),
                ("E.pou", "E"),
                ("F.PoU", "F"),
                ("G.POU", "G"),
            ] {
                std::fs::write(
                    root.join(path),
                    format!("TEST_PROGRAM {name}\nEND_TEST_PROGRAM\n"),
                )
                .map_err(|error| error.to_string())?;
            }
            let sources = load_sources(&root).map_err(|error| error.to_string())?;
            let discovered = discover_tests(&sources);
            probe.target = Some(json!({"sources": sources.len(), "tests": discovered.len()}));
            (sources.len() != 7 || discovered.len() != 7)
                .then(|| format!("sources={}, tests={}", sources.len(), discovered.len()))
        }
        "UNSUPPORTED_OR_EMPTY_DISCOVERY" => {
            let sources_root = root.join("src");
            std::fs::create_dir_all(&sources_root).map_err(|error| error.to_string())?;
            std::fs::write(
                sources_root.join("Main.txt"),
                "TEST_PROGRAM Main\nEND_TEST_PROGRAM\n",
            )
            .map_err(|error| error.to_string())?;
            std::fs::write(sources_root.join("Generated.stbc"), "not a source")
                .map_err(|error| error.to_string())?;
            let sources = load_sources(&root).map_err(|error| error.to_string())?;
            let result = run_test_json(Some(root.clone()), None, false, 1);
            probe.target = Some(json!({
                "sources": sources.len(),
                "diagnostic": result.as_ref().err().map(|error| format!("{error:#}")),
            }));
            if !sources.is_empty() {
                Some(format!("unsupported sources discovered: {sources:?}"))
            } else if !result
                .as_ref()
                .err()
                .is_some_and(|error| format!("{error:#}").contains("no ST sources"))
            {
                Some(format!("empty discovery was not visible: {result:?}"))
            } else {
                None
            }
        }
        other => return Err(format!("unreviewed discovery scenario {other}")),
    };
    let _ = std::fs::remove_dir_all(root);
    Ok(CaseExecution {
        result: if failure.is_none() {
            CaseResult::Passed
        } else {
            CaseResult::Failed
        },
        observed_error: failure,
        observed_status: Some("source_discovery_checked".to_string()),
    })
}

#[derive(Default)]
struct DiscoveryTraceProbe {
    target: Option<JsonValue>,
    after: bool,
}

impl StateProbe for DiscoveryTraceProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.after {
            self.target = None;
        }
        self.after = !self.after;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.target.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}

/// The outcome and message of each test of `source`, the project's one file `src/tests.st`, when
/// `run` names the tests that run in their order, compiled for this computer (specification 44
/// §14.2).
fn compiled_outcomes(
    label: &str,
    source: &str,
    run: &[&str],
) -> Vec<(String, TestOutcome, String)> {
    let project = unique_temp_dir(label);
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(project.join("src/tests.st"), source).expect("write the tests");
    let sources_root = resolve_sources_root(&project, None).expect("sources");
    let sources = load_sources(&sources_root).expect("load");
    let discovered = discover_tests(&sources);
    let tests: Vec<DiscoveredTest> = run
        .iter()
        .map(|name| {
            discovered
                .iter()
                .find(|case| case.name.as_str() == *name)
                .unwrap_or_else(|| panic!("{name} is discovered"))
                .clone()
        })
        .collect();
    let results = execute_native_tests(&project, &discovered, &tests, 5).expect("the compiled run");
    let _ = std::fs::remove_dir_all(&project);
    results
        .into_iter()
        .map(|result| {
            (
                result.case.name.to_string(),
                result.outcome,
                result.message.unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn execution_reports_assertion_failure_for_test_program() {
    let outcomes = compiled_outcomes(
        "assertion-failure",
        r#"
TEST_PROGRAM FailCase
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#,
        &["FailCase"],
    );
    assert_eq!(outcomes[0].1, TestOutcome::Failed, "{outcomes:?}");
}

/// A runtime error that is no assertion is the test's error with the runtime's words, here the
/// division of `DINT#1` by a variable that holds 0 (specification 10 §10). Catches a runtime error
/// reported as a failed assertion.
#[test]
fn execution_preserves_non_assertion_runtime_error_classification() {
    let outcomes = compiled_outcomes(
        "runtime-error",
        r#"
TEST_PROGRAM RuntimeErrorCase
VAR
    Zero : DINT := DINT#0;
    Result : DINT;
END_VAR
Result := DINT#1 / Zero;
END_TEST_PROGRAM
"#,
        &["RuntimeErrorCase"],
    );
    assert_eq!(
        (outcomes[0].1, outcomes[0].2.as_str()),
        (
            TestOutcome::Error,
            RuntimeError::DivisionByZero.to_string().as_str()
        )
    );
}

#[test]
fn execution_runs_test_function_block() {
    let outcomes = compiled_outcomes(
        "function-block",
        r#"
TEST_FUNCTION_BLOCK FbPass
ASSERT_TRUE(FALSE);
END_TEST_FUNCTION_BLOCK

PROGRAM Main
END_PROGRAM
"#,
        &["FbPass"],
    );
    assert_eq!(
        outcomes[0].1,
        TestOutcome::Failed,
        "function-block test body was not executed: {outcomes:?}"
    );
}

/// Each run of a test starts from a cold restart: `Counter` is 1 after the body in both runs of
/// the same test, which its own assertion checks. Catches a second run that sees the first run's
/// 1 and counts to 2.
#[test]
fn execution_isolated_per_test_case() {
    let outcomes = compiled_outcomes(
        "isolation",
        r#"
VAR_GLOBAL
    Counter : INT := INT#0;
END_VAR

TEST_PROGRAM Isolated
Counter := Counter + INT#1;
ASSERT_EQUAL(INT#1, Counter);
END_TEST_PROGRAM
"#,
        &["Isolated", "Isolated"],
    );
    for (execution, outcome) in outcomes.iter().enumerate() {
        assert_eq!(
            outcome.1,
            TestOutcome::Passed,
            "execution {} did not run from cold initial state: {outcome:?}",
            execution + 1
        );
    }
}

/// Two tests of one image: the second starts from a cold restart, so it finds `Counter` 0 after
/// the first set it to 1. Catches a program image that keeps one test's state for the next.
#[test]
fn prepared_runtime_cold_restarts_between_cases() {
    let outcomes = compiled_outcomes(
        "cold-restarts",
        r#"
VAR_GLOBAL
    Counter : INT := INT#0;
END_VAR

TEST_PROGRAM FirstCase
Counter := Counter + INT#1;
ASSERT_EQUAL(INT#1, Counter);
END_TEST_PROGRAM

TEST_PROGRAM SecondCase
ASSERT_EQUAL(INT#0, Counter);
Counter := INT#2;
END_TEST_PROGRAM
"#,
        &["FirstCase", "SecondCase"],
    );
    assert_eq!(outcomes[0].1, TestOutcome::Passed, "{outcomes:?}");
    assert_eq!(
        outcomes[1].1,
        TestOutcome::Passed,
        "second test did not execute after a cold restart: {outcomes:?}"
    );
}

/// A test program runs beside a `CONFIGURATION` that does not bind it (specification 44 §14.2).
/// Catches a test program left out of the image because the configuration names other programs.
#[test]
fn a_test_program_runs_beside_a_configuration() {
    let outcomes = compiled_outcomes(
        "configuration",
        r#"
CONFIGURATION Cfg
    RESOURCE Res ON PLC
        TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM MainInst WITH MainTask : Main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM

TEST_PROGRAM Probe
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#,
        &["Probe"],
    );
    assert_eq!(
        outcomes[0].1,
        TestOutcome::Failed,
        "the test program was not executed: {outcomes:?}"
    );
}

#[test]
fn run_test_executes_test_program_when_configuration_is_present() {
    let project = unique_temp_dir("config-test-program-project");
    let sources = project.join("src");
    std::fs::create_dir_all(&sources).expect("create src dir");
    std::fs::write(
        sources.join("tests.st"),
        r#"
CONFIGURATION Cfg
    RESOURCE Res ON PLC
        TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM MainInst WITH MainTask : Main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM

TEST_PROGRAM Probe
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#,
    )
    .expect("write config + test source");

    let result = run_test(
        Some(project.clone()),
        Some("Probe".to_string()),
        false,
        0,
        TestOutput::Human,
        false,
        TestRunner::Native,
    );
    let error = result.expect_err("configured test program must execute its failing assertion");
    assert!(
        format!("{error:#}").contains("1 ST test(s) failed"),
        "unexpected test command failure: {error:#}"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn json_output_contract() {
    let results = sample_results();
    let summary = summarize_results(&results);
    let output = render_output(
        TestOutput::Json,
        Path::new("/tmp/project"),
        &results,
        summary,
        results.len(),
        None,
        6,
    )
    .expect("json output");
    let value: serde_json::Value = serde_json::from_str(&output).expect("valid json");

    assert_eq!(value["version"], 1);
    assert_eq!(value["summary"]["total"], 3);
    assert_eq!(value["summary"]["passed"], 1);
    assert_eq!(value["summary"]["failed"], 1);
    assert_eq!(value["summary"]["errors"], 1);
    assert_eq!(value["tests"][0]["status"], "passed");
    assert_eq!(value["tests"][1]["status"], "failed");
    assert_eq!(value["tests"][2]["status"], "error");
    assert_eq!(value["tests"][1]["source"], "ASSERT_EQUAL(INT#2, X);");
    assert_eq!(value["summary"]["duration_ms"], 6);
    assert_eq!(value["tests"][0]["duration_ms"], 1);
    assert_eq!(value["tests"][1]["duration_ms"], 2);
    assert_eq!(value["tests"][2]["duration_ms"], 3);
}

#[test]
fn tap_output_contract() {
    let results = sample_results();
    let summary = summarize_results(&results);
    let output = render_output(
        TestOutput::Tap,
        Path::new("/tmp/project"),
        &results,
        summary,
        results.len(),
        None,
        6,
    )
    .unwrap();

    assert!(output.starts_with("TAP version 13\n1..3\n"));
    assert!(output.contains("ok 1 - TEST_PROGRAM::PassCase"));
    assert!(output.contains("not ok 2 - TEST_PROGRAM::FailCase"));
    assert!(output.contains("not ok 3 - TEST_FUNCTION_BLOCK::ErrCase"));
    assert!(output.contains("# file: tests.st"));
    assert!(output.contains("# line: 12"));
    assert!(output.contains("# source: ASSERT_EQUAL(INT#2, X);"));
}

#[test]
fn junit_output_contract() {
    let results = sample_results();
    let summary = summarize_results(&results);
    let output = render_output(
        TestOutput::Junit,
        Path::new("/tmp/project"),
        &results,
        summary,
        results.len(),
        None,
        6,
    )
    .unwrap();

    assert!(output.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(output.contains(
        "<testsuite name=\"trust-runtime\" tests=\"3\" failures=\"1\" errors=\"1\" skipped=\"0\">"
    ));
    assert!(output.contains("<testcase name=\"TEST_PROGRAM::PassCase\""));
    assert!(output
        .contains("<failure message=\"ASSERT_EQUAL failed: expected &lt;2&gt; &amp; got 3\">"));
    assert!(output.contains("<error message=\"runtime &lt;panic&gt;\">"));
}

fn sample_results() -> Vec<ExecutedTest> {
    vec![
        ExecutedTest {
            case: DiscoveredTest {
                kind: TestKind::Program,
                name: "PassCase".into(),
                file: PathBuf::from("tests.st"),
                byte_offset: 0,
                line: 4,
                source_line: Some("ASSERT_TRUE(TRUE);".to_string()),
            },
            outcome: TestOutcome::Passed,
            message: None,
            duration_ms: 1,
        },
        ExecutedTest {
            case: DiscoveredTest {
                kind: TestKind::Program,
                name: "FailCase".into(),
                file: PathBuf::from("tests.st"),
                byte_offset: 10,
                line: 12,
                source_line: Some("ASSERT_EQUAL(INT#2, X);".to_string()),
            },
            outcome: TestOutcome::Failed,
            message: Some("ASSERT_EQUAL failed: expected <2> & got 3".to_string()),
            duration_ms: 2,
        },
        ExecutedTest {
            case: DiscoveredTest {
                kind: TestKind::FunctionBlock,
                name: "ErrCase".into(),
                file: PathBuf::from("fb_tests.st"),
                byte_offset: 20,
                line: 20,
                source_line: Some("ASSERT_TRUE(FALSE);".to_string()),
            },
            outcome: TestOutcome::Error,
            message: Some("runtime <panic>".to_string()),
            duration_ms: 3,
        },
    ]
}

#[test]
fn human_output_shows_failure_summary_with_source_context() {
    let results = sample_results();
    let summary = summarize_results(&results);
    let output = render_output(
        TestOutput::Human,
        Path::new("/tmp/project"),
        &results,
        summary,
        results.len(),
        None,
        6,
    )
    .expect("human output");

    let plain = strip_ansi(&output);
    assert!(plain.contains("FAIL [2/3] TEST_PROGRAM::FailCase tests.st:12 [2ms]"));
    assert!(plain.contains("reason   : ASSERT_EQUAL failed: expected <2> & got 3"));
    assert!(plain.contains("source   : ASSERT_EQUAL(INT#2, X);"));
    assert!(plain.contains("Failure summary:"));
    assert!(plain.contains("1. TEST_PROGRAM::FailCase @ tests.st:12"));
    assert!(plain.contains("2. TEST_FUNCTION_BLOCK::ErrCase @ fb_tests.st:20"));
    assert!(plain.contains("1 passed, 1 failed, 1 errors (6ms)"));
}

#[test]
fn human_output_filter_zero_message_is_clear() {
    let output = render_output(
        TestOutput::Human,
        Path::new("/tmp/project"),
        &[],
        TestSummary::default(),
        2,
        Some("START"),
        0,
    )
    .expect("human output");
    let plain = strip_ansi(&output);
    assert!(plain.contains("0 tests matched filter \"START\""));
    assert!(plain.contains("(2 tests discovered, all filtered out)"));
}

#[test]
fn list_output_contract() {
    let tests = vec![
        DiscoveredTest {
            kind: TestKind::Program,
            name: "CaseA".into(),
            file: PathBuf::from("/tmp/project/src/tests.st"),
            byte_offset: 0,
            line: 1,
            source_line: None,
        },
        DiscoveredTest {
            kind: TestKind::FunctionBlock,
            name: "CaseB".into(),
            file: PathBuf::from("/tmp/project/src/tests.st"),
            byte_offset: 12,
            line: 24,
            source_line: None,
        },
    ];
    let text = render_list_output(Path::new("/tmp/project"), &tests, 2, None);
    assert!(text.contains("TEST_PROGRAM::CaseA (src/tests.st:1)"));
    assert!(text.contains("TEST_FUNCTION_BLOCK::CaseB (src/tests.st:24)"));
    assert!(text.contains("2 test(s) listed"));
}

/// A test that never ends ends with the statement budget of a test, 100,000,000 statements and
/// loop passes (specification 44 NX-10), as the test's error. Catches a test run that hangs, and
/// one whose end is reported as a failed assertion.
#[test]
fn a_test_that_never_ends_is_stopped_by_its_statement_budget() {
    let outcomes = compiled_outcomes(
        "endless",
        r#"
TEST_PROGRAM TimeoutCase
WHILE TRUE DO
END_WHILE;
END_TEST_PROGRAM
"#,
        &["TimeoutCase"],
    );
    let limit = RuntimeError::ExecutionLimit {
        kind: trust_runtime::error::ExecutionLimitKind::Work,
        used: 100_000_000,
        requested: 1,
        limit: 100_000_000,
    };
    assert_eq!(
        (outcomes[0].1, outcomes[0].2.as_str()),
        (TestOutcome::Error, limit.to_string().as_str())
    );
}

#[test]
fn ci_mode_defaults_human_output_to_junit() {
    assert_eq!(effective_output(TestOutput::Human, true), TestOutput::Junit);
    assert_eq!(effective_output(TestOutput::Json, true), TestOutput::Json);
    assert_eq!(effective_output(TestOutput::Tap, true), TestOutput::Tap);
    assert_eq!(effective_output(TestOutput::Junit, true), TestOutput::Junit);
    assert_eq!(
        effective_output(TestOutput::Human, false),
        TestOutput::Human
    );
}

#[test]
fn timeout_message_pluralization() {
    assert_eq!(timeout_message(1), "test timed out after 1 second");
    assert_eq!(timeout_message(5), "test timed out after 5 seconds");
}

#[test]
fn filtered_configured_test_registers_all_test_declarations_but_executes_only_selected() {
    let project = unique_temp_dir("configured-filtered-declarations");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src/tests.st"),
        r#"
CONFIGURATION Cfg
RESOURCE Res ON PLC
TASK Scan(INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM Production WITH Scan : Main;
END_RESOURCE
END_CONFIGURATION
PROGRAM Main
ASSERT_TRUE(FALSE);
END_PROGRAM
TEST_PROGRAM ChosenCase
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
TEST_PROGRAM OtherCase
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#,
    )
    .unwrap();
    run_test(
        Some(project.clone()),
        Some("ChosenCase".into()),
        false,
        0,
        TestOutput::Human,
        false,
        TestRunner::Native,
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(project);
}

/// The tests of a project compiled for this computer pass and fail with the runtime's words (specification 44 §14.2): `ASSERT_EQUAL` of `INT#5` and `INT#4` fails
/// "expected 5, actual 4"; `ASSERT_NEAR` of 1.0 and 1.5 within 0.1 fails with the binary64 spelling
/// of the `REAL` 0.1, 0.10000000149011612, and the difference 0.5; `ASSERT_TRUE(FALSE)` fails; an
/// enumeration is written `Type::Value`; a text is quoted with `$$` for `$` and `$'` for `'`, so the
/// text `it's $5` is written `'it$'s $$5'`; two texts compare by their bytes, so `'AB'` is less than
/// `'B'` and not greater; a test function block runs its body once on a fresh instance, so `Calls`
/// goes from its initial 10 to 11 and the latch it sets holds, while a latch set and reset in one
/// call is FALSE; and a test that asserts what holds passes. Each test starts from a cold restart,
/// so `Counter` is 1 in both tests that count. The tests need the toolchain of specification 44
/// §4.6 at `TRUST_TOOLCHAIN`.
#[test]
fn compiled_tests_pass_and_fail_with_the_runtimes_words() {
    let project =
        std::env::temp_dir().join(format!("trust-dev-native-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        r#"
TYPE Color : (Red, Green); END_TYPE

VAR_GLOBAL
    Counter : INT;
END_VAR

TEST_PROGRAM Passes
Counter := Counter + 1;
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Counter);
ASSERT_TRUE(IN := TRUE);
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := REAL#1.05, DELTA := REAL#0.1);
END_TEST_PROGRAM

TEST_PROGRAM CountsAgain
Counter := Counter + 1;
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Counter);
END_TEST_PROGRAM

TEST_PROGRAM EqualFails
ASSERT_EQUAL(EXPECTED := INT#5, ACTUAL := INT#4);
END_TEST_PROGRAM

TEST_PROGRAM NearFails
ASSERT_NEAR(EXPECTED := REAL#1.0, ACTUAL := REAL#1.5, DELTA := REAL#0.1);
END_TEST_PROGRAM

TEST_PROGRAM TrueFails
ASSERT_TRUE(IN := FALSE);
END_TEST_PROGRAM

TEST_PROGRAM ColorFails
VAR
    c : Color := Green;
END_VAR
ASSERT_EQUAL(EXPECTED := Color#Red, ACTUAL := c);
END_TEST_PROGRAM

TEST_PROGRAM TextsPass
ASSERT_EQUAL(EXPECTED := CONCAT('N', 'E'), ACTUAL := 'NE');
ASSERT_LESS(VALUE := 'AB', BOUND := 'B');
END_TEST_PROGRAM

TEST_PROGRAM TextFails
VAR
    direction : STRING[8] := 'NE';
END_VAR
ASSERT_EQUAL(EXPECTED := 'it$'s $$5', ACTUAL := direction);
END_TEST_PROGRAM

TEST_PROGRAM TextOrderFails
ASSERT_GREATER(VALUE := 'AB', BOUND := 'B');
END_TEST_PROGRAM

FUNCTION_BLOCK Latch
VAR_INPUT
    Arm : BOOL;
    Clear : BOOL;
END_VAR
VAR_OUTPUT
    Q : BOOL;
END_VAR
Q := (Q OR Arm) AND NOT Clear;
END_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK LatchHolds
VAR
    Dut : Latch;
    Calls : INT := 10;
END_VAR
Calls := Calls + 1;
Dut(Arm := TRUE);
Dut(Arm := FALSE);
ASSERT_TRUE(IN := Dut.Q);
ASSERT_EQUAL(EXPECTED := INT#11, ACTUAL := Calls);
END_TEST_FUNCTION_BLOCK

TEST_FUNCTION_BLOCK LatchFails
VAR
    Dut : Latch;
END_VAR
Dut(Arm := TRUE, Clear := TRUE);
ASSERT_TRUE(IN := Dut.Q);
END_TEST_FUNCTION_BLOCK
"#,
    )
    .expect("write the tests");
    let sources_root = resolve_sources_root(&project, None).expect("sources");
    let sources = load_sources(&sources_root).expect("load");
    let discovered = discover_tests(&sources);
    let native = execute_native_tests(&project, &discovered, &discovered, 5).expect("native run");
    let outcome = |results: &[ExecutedTest], name: &str| {
        let result = results
            .iter()
            .find(|result| result.case.name.as_str() == name)
            .unwrap_or_else(|| panic!("{name} ran"));
        (result.outcome, result.message.clone().unwrap_or_default())
    };
    for (name, expected, message) in [
        ("Passes", TestOutcome::Passed, ""),
        ("CountsAgain", TestOutcome::Passed, ""),
        (
            "EqualFails",
            TestOutcome::Failed,
            "ASSERT_EQUAL failed: expected 5, actual 4",
        ),
        (
            "NearFails",
            TestOutcome::Failed,
            "ASSERT_NEAR failed: expected 1, actual 1.5, delta 0.10000000149011612, diff 0.5",
        ),
        (
            "TrueFails",
            TestOutcome::Failed,
            "ASSERT_TRUE expected TRUE, got FALSE",
        ),
        (
            "ColorFails",
            TestOutcome::Failed,
            "ASSERT_EQUAL failed: expected Color::Red, actual Color::Green",
        ),
        ("TextsPass", TestOutcome::Passed, ""),
        (
            "TextFails",
            TestOutcome::Failed,
            "ASSERT_EQUAL failed: expected 'it$'s $$5', actual 'NE'",
        ),
        (
            "TextOrderFails",
            TestOutcome::Failed,
            "ASSERT_GREATER failed: value 'AB' is not greater than bound 'B'",
        ),
        ("LatchHolds", TestOutcome::Passed, ""),
        (
            "LatchFails",
            TestOutcome::Failed,
            "ASSERT_TRUE expected TRUE, got FALSE",
        ),
    ] {
        assert_eq!(
            outcome(&native, name),
            (expected, message.to_string()),
            "{name} compiled"
        );
    }
    let _ = std::fs::remove_dir_all(&project);
}

/// The test image is the kind of image the project deploys (specification 44 §13.2, §14.2): a debug
/// image when the project has no settings file, a release image when its `trust-lsp.toml` says
/// `[build] debug = false`, which bit 0 of the header flags tells. Catches test images that ignore
/// the setting, as they did when every test image was built without hooks. The test needs the
/// toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
#[test]
fn the_test_image_is_the_kind_of_image_the_project_builds() {
    use trust_runtime::native::{Target, Toolchain};
    use trust_runtime_core::native::image::Image;

    let project = std::env::temp_dir().join(format!("trust-dev-image-kind-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM Holds\nASSERT_TRUE(IN := TRUE);\nEND_TEST_PROGRAM\n",
    )
    .expect("write the test");
    let sources_root = resolve_sources_root(&project, None).expect("sources");
    let sources = load_sources(&sources_root).expect("load");
    let discovered = discover_tests(&sources);
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let release = || {
        let built = build_test_image(&project, &discovered, host, &toolchain)
            .expect("the test image builds");
        Image::parse(&built.image)
            .expect("the image reads back")
            .header()
            .is_release()
    };
    assert!(!release(), "a project without settings tests a debug image");
    std::fs::write(project.join("trust-lsp.toml"), "[build]\ndebug = false\n")
        .expect("write the settings");
    assert!(release(), "[build] debug = false tests a release image");
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn a_device_runs_the_tests_compiled_for_it() {
    // Specification 44 §14.2 and specification 22: `--device` with `--target` runs the compiled
    // tests on the device, and without them the kernel of this computer runs them. Catches a device
    // run that silently runs the tests here.
    let device = || Some(("tcp://127.0.0.1:5555".to_string(), "esp32c6".to_string()));
    let on_device = TestRunner::Device {
        port: "tcp://127.0.0.1:5555".into(),
        target: "esp32c6".into(),
    };
    assert_eq!(TestRunner::of(device()), on_device);
    assert_eq!(TestRunner::of(None), TestRunner::Native);
}
