use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
use trust_runtime::harness::SourceFile as HarnessSourceFile;

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
fn execution_reports_assertion_failure_for_test_program() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("tests.st"),
        text: r#"
TEST_PROGRAM FailCase
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 1);

    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "tests.st",
        sources[0].text.clone(),
    )]);
    let err = execute_test_case(&session, &tests[0], None).unwrap_err();
    assert!(matches!(err, RuntimeError::AssertionFailed(_)));
}

#[test]
fn execution_runs_test_function_block() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("tests_fb.st"),
        text: r#"
TEST_FUNCTION_BLOCK FbPass
ASSERT_FALSE(FALSE);
END_TEST_FUNCTION_BLOCK

PROGRAM Main
END_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 1);

    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "tests_fb.st",
        sources[0].text.clone(),
    )]);
    execute_test_case(&session, &tests[0], None).unwrap();
}

#[test]
fn execution_isolated_per_test_case() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("isolation.st"),
        text: r#"
TEST_PROGRAM Isolated
VAR
    X : INT := INT#0;
END_VAR
X := X + INT#1;
ASSERT_EQUAL(INT#1, X);
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 1);

    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "isolation.st",
        sources[0].text.clone(),
    )]);
    execute_test_case(&session, &tests[0], None).unwrap();
    execute_test_case(&session, &tests[0], None).unwrap();
}

#[test]
fn prepared_runtime_cold_restarts_between_cases() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("prepared_runtime.st"),
        text: r#"
VAR_GLOBAL
    Counter : INT := INT#0;
END_VAR

TEST_PROGRAM FirstCase
Counter := Counter + INT#1;
ASSERT_EQUAL(INT#1, Counter);
END_TEST_PROGRAM

TEST_PROGRAM SecondCase
ASSERT_EQUAL(INT#0, Counter);
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 2);

    let extra_program_instances = tests
        .iter()
        .map(|case| case.name.clone())
        .collect::<Vec<_>>();
    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "prepared_runtime.st",
        sources[0].text.clone(),
    )])
    .with_extra_program_instances(extra_program_instances);
    let mut runtime = session.build_runtime().expect("build runtime");

    execute_test_case_in_runtime(&mut runtime, &tests[0], None).unwrap();
    execute_test_case_in_runtime(&mut runtime, &tests[1], None).unwrap();
}

#[test]
fn execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("tests.st"),
        text: r#"
CONFIGURATION Cfg
    RESOURCE Res ON PLC
        TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM MainInst WITH MainTask : Main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM

TEST_PROGRAM Probe
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 1);

    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "tests.st",
        sources[0].text.clone(),
    )]);
    let err = execute_test_case(&session, &tests[0], None).unwrap_err();
    assert!(
        matches!(&err, RuntimeError::ControlError(message)
            if message.contains("unbound PROGRAM declaration(s) under CONFIGURATION")
                && message.contains("Probe")),
        "unexpected error: {err}"
    );
}

#[test]
fn execute_test_case_runs_test_program_when_session_registers_extra_program_instance() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("tests.st"),
        text: r#"
CONFIGURATION Cfg
    RESOURCE Res ON PLC
        TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM MainInst WITH MainTask : Main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM

TEST_PROGRAM Probe
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    assert_eq!(tests.len(), 1);

    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "tests.st",
        sources[0].text.clone(),
    )])
    .with_extra_program_instances([tests[0].name.clone()]);
    execute_test_case(&session, &tests[0], None).unwrap();
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
ASSERT_TRUE(TRUE);
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
    );
    assert!(result.is_ok(), "expected test command success: {result:?}");

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

#[test]
fn execute_test_case_returns_execution_timeout_for_deadline_overrun() {
    let sources = vec![LoadedSource {
        path: PathBuf::from("timeout.st"),
        text: r#"
TEST_PROGRAM TimeoutCase
WHILE TRUE DO
END_WHILE;
END_TEST_PROGRAM
"#
        .to_string(),
    }];
    let tests = discover_tests(&sources);
    let session = CompileSession::from_sources(vec![HarnessSourceFile::with_path(
        "timeout.st",
        sources[0].text.clone(),
    )]);
    let err = execute_test_case(&session, &tests[0], Some(StdDuration::ZERO)).unwrap_err();
    assert!(matches!(err, RuntimeError::ExecutionTimeout));
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
