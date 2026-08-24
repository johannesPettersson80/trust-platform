use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_HARNESS_WORKSPACE_ID: AtomicUsize = AtomicUsize::new(0);

struct HarnessWorkspace {
    root: PathBuf,
}

impl HarnessWorkspace {
    fn new(label: &str) -> Self {
        let id = NEXT_HARNESS_WORKSPACE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-dev-agent-harness-contract-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create harness workspace");
        Self {
            root: root.canonicalize().expect("canonical harness workspace"),
        }
    }

    fn server(&self) -> AgentServer {
        AgentServer::new(self.root.clone())
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture parent");
        }
        fs::write(path, text).expect("write fixture");
    }
}

impl Drop for HarnessWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bool_value(value: bool) -> Value {
    json!({"type": "BOOL", "value": value})
}

fn inline_source(text: &str) -> Value {
    json!([{"path": "main.st", "text": text}])
}

fn execute(
    server: &AgentServer,
    source: &str,
    steps: Value,
    assertions: Value,
    watch: Value,
) -> Result<Value, AgentCommandError> {
    server.harness_execute(
        serde_json::from_value(json!({
            "inline_sources": inline_source(source),
            "steps": steps,
            "assertions": assertions,
            "watch": watch,
        }))
        .expect("execute params"),
    )
}

fn passthrough_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    input : BOOL;
    output : BOOL;
END_VAR
output := input;
END_PROGRAM
"#
}

fn timeout_source() -> &'static str {
    r#"
PROGRAM Main
VAR
    q : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#
}

fn failure(
    kind: &'static str,
    step_index: Option<usize>,
    expected: Option<Value>,
    actual: Option<Value>,
) -> HarnessExecuteFailure {
    HarnessExecuteFailure {
        kind,
        step_index,
        step: None,
        assertion: None,
        message: Some(format!("{kind} message")),
        path: None,
        expected,
        actual,
        errors: Vec::new(),
    }
}

#[test]
fn source_selection_rejects_empty_inline_and_file_lists() {
    let workspace = HarnessWorkspace::new("empty-selectors");
    let server = workspace.server();

    for params in [
        HarnessLoadParams {
            inline_sources: Some(Vec::new()),
            ..HarnessLoadParams::default()
        },
        HarnessLoadParams {
            files: Some(Vec::new()),
            ..HarnessLoadParams::default()
        },
    ] {
        let error = server
            .collect_harness_sources(&params)
            .expect_err("empty explicit selector");
        assert_eq!(error.code, -32602);
    }
}

#[test]
fn source_selection_rejects_multiple_selector_kinds() {
    let workspace = HarnessWorkspace::new("ambiguous-selectors");
    workspace.write("main.st", passthrough_source());
    fs::create_dir_all(workspace.root.join("project")).unwrap();
    let server = workspace.server();

    let matrices = [
        HarnessLoadParams {
            project: None,
            files: Some(vec!["main.st".into()]),
            inline_sources: Some(vec![InlineSource {
                path: Some("inline.st".into()),
                text: passthrough_source().into(),
            }]),
        },
        HarnessLoadParams {
            project: Some("project".into()),
            files: Some(vec!["main.st".into()]),
            inline_sources: None,
        },
        HarnessLoadParams {
            project: Some("project".into()),
            files: None,
            inline_sources: Some(vec![InlineSource {
                path: None,
                text: passthrough_source().into(),
            }]),
        },
    ];

    for params in matrices {
        let error = server
            .collect_harness_sources(&params)
            .expect_err("ambiguous source selectors");
        assert_eq!(error.code, -32602);
    }
}

#[test]
fn source_selection_rejects_blank_inline_text() {
    let workspace = HarnessWorkspace::new("blank-inline");
    let server = workspace.server();
    let params = HarnessLoadParams {
        inline_sources: Some(vec![InlineSource {
            path: Some("blank.st".into()),
            text: " \n\t".into(),
        }]),
        ..HarnessLoadParams::default()
    };

    let error = server
        .collect_harness_sources(&params)
        .expect_err("blank inline source");

    assert_eq!(error.code, -32602);
}

#[test]
fn source_selection_rejects_duplicate_normalized_file_paths() {
    let workspace = HarnessWorkspace::new("duplicate-files");
    workspace.write("src/main.st", passthrough_source());
    let server = workspace.server();
    let params = HarnessLoadParams {
        files: Some(vec!["src/main.st".into(), "./src/main.st".into()]),
        ..HarnessLoadParams::default()
    };

    let error = server
        .collect_harness_sources(&params)
        .expect_err("duplicate file selector");

    assert_eq!(error.code, -32602);
}

#[test]
fn source_selection_preserves_explicit_file_order() {
    let workspace = HarnessWorkspace::new("file-order");
    workspace.write("a.st", "PROGRAM A\nEND_PROGRAM\n");
    workspace.write("b.st", "PROGRAM B\nEND_PROGRAM\n");
    let server = workspace.server();
    let params = HarnessLoadParams {
        files: Some(vec!["b.st".into(), "a.st".into()]),
        ..HarnessLoadParams::default()
    };

    let sources = server.collect_harness_sources(&params).unwrap();

    assert_eq!(
        sources,
        vec![
            "PROGRAM B\nEND_PROGRAM\n".to_string(),
            "PROGRAM A\nEND_PROGRAM\n".to_string()
        ]
    );
}

#[test]
fn cycle_step_defaults_to_one_and_preserves_explicit_zero() {
    let defaulted: HarnessExecuteStep = serde_json::from_value(json!({"op": "cycle"})).unwrap();
    let explicit_zero: HarnessExecuteStep =
        serde_json::from_value(json!({"op": "cycle", "count": 0})).unwrap();

    match defaulted {
        HarnessExecuteStep::Cycle { count, dt_ms } => {
            assert_eq!(count, 1);
            assert_eq!(dt_ms, None);
        }
        _ => panic!("expected cycle"),
    }
    match explicit_zero {
        HarnessExecuteStep::Cycle { count, .. } => assert_eq!(count, 0),
        _ => panic!("expected cycle"),
    }
}

#[test]
fn unknown_step_and_assertion_discriminators_are_invalid_params() {
    let workspace = HarnessWorkspace::new("unknown-discriminators");
    let mut server = workspace.server();

    for params in [
        json!({
            "inline_sources": inline_source(passthrough_source()),
            "steps": [{"op": "teleport"}]
        }),
        json!({
            "inline_sources": inline_source(passthrough_source()),
            "assertions": [{"kind": "approximately_equal"}]
        }),
    ] {
        let error = server
            .execute("harness.execute", Some(params))
            .expect_err("unknown discriminator");
        assert_eq!(error.code, -32602);
    }
}

#[test]
fn malformed_typed_value_is_invalid_params_before_execution() {
    let workspace = HarnessWorkspace::new("malformed-value");
    let mut server = workspace.server();

    let error = server
        .execute(
            "harness.execute",
            Some(json!({
                "inline_sources": inline_source(passthrough_source()),
                "steps": [{
                    "op": "set_input",
                    "name": "input",
                    "value": {"type": "BOOL", "value": "yes"}
                }]
            })),
        )
        .expect_err("invalid typed value");

    assert_eq!(error.code, -32602);
}

#[test]
fn invalid_cycle_time_and_iteration_bounds_are_invalid_params() {
    let workspace = HarnessWorkspace::new("invalid-bounds");
    let mut server = workspace.server();

    for step in [
        json!({"op": "cycle", "count": 0}),
        json!({"op": "cycle", "count": 1, "dt_ms": -1}),
        json!({"op": "advance_time", "duration_ms": -1}),
        json!({
            "op": "run_until",
            "name": "output",
            "equals": bool_value(true),
            "max_cycles": 0
        }),
    ] {
        let error = server
            .execute(
                "harness.execute",
                Some(json!({
                    "inline_sources": inline_source(passthrough_source()),
                    "steps": [step]
                })),
            )
            .expect_err("invalid execution bound");
        assert_eq!(error.code, -32602);
    }
}

#[test]
fn restart_mode_accepts_case_insensitive_cold_and_warm_only() {
    assert_eq!(parse_restart_mode("cold").unwrap(), RestartMode::Cold);
    assert_eq!(parse_restart_mode("COLD").unwrap(), RestartMode::Cold);
    assert_eq!(parse_restart_mode("warm").unwrap(), RestartMode::Warm);
    assert_eq!(parse_restart_mode("WaRm").unwrap(), RestartMode::Warm);

    for invalid in ["", "hot", " cold ", "power_cycle"] {
        assert!(matches!(
            parse_restart_mode(invalid),
            Err(HarnessAutomationError::InvalidArgument(_))
        ));
    }
}

#[test]
fn empty_watch_snapshot_has_stable_camel_case_shape() {
    assert_eq!(
        empty_watch_snapshot(),
        json!({
            "cycleCount": 0,
            "elapsedMs": 0,
            "values": {}
        })
    );
}

#[test]
fn successful_result_omits_failures_and_satisfies_accounting() {
    let result = build_harness_execute_result(2, 3, 4, 4, empty_watch_snapshot(), Vec::new());
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["sourceCount"], 2);
    assert_eq!(value["stepsRun"], 3);
    assert_eq!(value["status"], "pass");
    assert_eq!(value["passed"], true);
    assert_eq!(value["assertions"]["total"], 4);
    assert_eq!(value["assertions"]["evaluated"], 4);
    assert_eq!(value["assertions"]["passed"], 4);
    assert_eq!(value["assertions"]["failed"], 0);
    assert!(value.get("failures").is_none());
}

#[test]
fn assertion_failures_are_counted_as_evaluated_and_failed() {
    let result = build_harness_execute_result(
        1,
        2,
        2,
        4,
        empty_watch_snapshot(),
        vec![
            failure(
                "assertion_failed",
                None,
                Some(bool_value(true)),
                Some(bool_value(false)),
            ),
            failure(
                "assertion_failed",
                None,
                Some(bool_value(false)),
                Some(bool_value(true)),
            ),
        ],
    );
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["status"], "fail");
    assert_eq!(value["passed"], false);
    assert_eq!(value["assertions"]["total"], 4);
    assert_eq!(value["assertions"]["evaluated"], 4);
    assert_eq!(value["assertions"]["passed"], 2);
    assert_eq!(value["assertions"]["failed"], 2);
    assert_eq!(value["failures"].as_array().unwrap().len(), 2);
}

#[test]
fn step_failure_does_not_falsely_increment_assertion_counts() {
    let result = build_harness_execute_result(
        1,
        1,
        0,
        3,
        empty_watch_snapshot(),
        vec![failure("runtime_cycle_error", Some(1), None, None)],
    );
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["stepsRun"], 1);
    assert_eq!(value["assertions"]["total"], 3);
    assert_eq!(value["assertions"]["evaluated"], 0);
    assert_eq!(value["assertions"]["passed"], 0);
    assert_eq!(value["assertions"]["failed"], 0);
}

#[test]
fn compile_failure_is_a_fixture_result_with_zero_steps() {
    let workspace = HarnessWorkspace::new("compile-failure");
    let server = workspace.server();
    let invalid_source = r#"
PROGRAM Main
VAR
    broken : TYPE_THAT_DOES_NOT_EXIST;
END_VAR
END_PROGRAM
"#;

    let value = execute(
        &server,
        invalid_source,
        json!([{"op": "cycle", "count": 1}]),
        json!([]),
        json!([]),
    )
    .expect("compile failure result");

    assert_eq!(value["status"], "fail");
    assert_eq!(value["passed"], false);
    assert_eq!(value["stepsRun"], 0);
    assert_eq!(value["failures"][0]["kind"], "compile_error");
    assert!(value["failures"][0].get("stepIndex").is_none());
}

#[test]
fn assertion_mismatch_returns_typed_expected_and_actual_values() {
    let workspace = HarnessWorkspace::new("assertion-mismatch");
    let server = workspace.server();

    let value = execute(
        &server,
        passthrough_source(),
        json!([
            {"op": "set_input", "name": "input", "value": bool_value(true)},
            {"op": "cycle", "count": 1}
        ]),
        json!([
            {"kind": "output_equals", "name": "output", "equals": bool_value(false)}
        ]),
        json!(["output"]),
    )
    .unwrap();

    assert_eq!(value["status"], "fail");
    assert_eq!(value["stepsRun"], 2);
    assert_eq!(value["assertions"]["evaluated"], 1);
    assert_eq!(value["assertions"]["failed"], 1);
    assert_eq!(value["failures"][0]["kind"], "assertion_failed");
    assert_eq!(value["failures"][0]["expected"], bool_value(false));
    assert_eq!(value["failures"][0]["actual"], bool_value(true));
}

#[test]
fn later_assertions_still_run_after_an_ordinary_mismatch() {
    let workspace = HarnessWorkspace::new("assertion-order");
    let server = workspace.server();

    let value = execute(
        &server,
        passthrough_source(),
        json!([{"op": "cycle", "count": 1}]),
        json!([
            {"kind": "output_equals", "name": "output", "equals": bool_value(true)},
            {"kind": "output_equals", "name": "input", "equals": bool_value(false)}
        ]),
        json!([]),
    )
    .unwrap();

    assert_eq!(value["assertions"]["total"], 2);
    assert_eq!(value["assertions"]["evaluated"], 2);
    assert_eq!(value["assertions"]["passed"], 1);
    assert_eq!(value["assertions"]["failed"], 1);
    assert_eq!(value["failures"].as_array().unwrap().len(), 1);
}

#[test]
fn assertion_boundary_error_is_a_structured_fixture_failure() {
    let workspace = HarnessWorkspace::new("assertion-boundary");
    let server = workspace.server();

    let value = execute(
        &server,
        passthrough_source(),
        json!([]),
        json!([
            {"kind": "output_equals", "name": "missing", "equals": bool_value(true)}
        ]),
        json!([]),
    )
    .expect("boundary failure result");

    assert_eq!(value["status"], "fail");
    assert_eq!(value["assertions"]["total"], 1);
    assert_eq!(value["assertions"]["evaluated"], 1);
    assert_eq!(value["assertions"]["failed"], 1);
    assert_eq!(value["failures"][0]["kind"], "unresolved_name");
    assert_eq!(value["failures"][0]["path"], "missing");
}

#[test]
fn run_until_timeout_is_a_step_failure_with_last_actual_value() {
    let workspace = HarnessWorkspace::new("run-until-timeout");
    let server = workspace.server();

    let value = execute(
        &server,
        timeout_source(),
        json!([{
            "op": "run_until",
            "name": "q",
            "equals": bool_value(true),
            "max_cycles": 1,
            "dt_ms": 10
        }]),
        json!([]),
        json!(["q"]),
    )
    .expect("timeout fixture result");

    assert_eq!(value["status"], "fail");
    assert_eq!(value["stepsRun"], 0);
    assert_eq!(value["failures"][0]["kind"], "run_until_timeout");
    assert_eq!(value["failures"][0]["stepIndex"], 0);
    assert_eq!(value["failures"][0]["expected"], bool_value(true));
    assert_eq!(value["failures"][0]["actual"], bool_value(false));
    assert_eq!(
        value["watchSnapshot"]["values"]["q"]["value"],
        bool_value(false)
    );
}

#[test]
fn execute_requests_are_fresh_and_do_not_share_runtime_state() {
    let workspace = HarnessWorkspace::new("fresh-execute");
    let server = workspace.server();
    let source = r#"
PROGRAM Main
VAR
    count : DINT;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    for _ in 0..2 {
        let value = execute(
            &server,
            source,
            json!([{"op": "cycle", "count": 1}]),
            json!([]),
            json!(["count"]),
        )
        .unwrap();
        assert_eq!(
            value["watchSnapshot"]["values"]["count"]["value"],
            json!({"type": "DINT", "value": 2}),
            "one initial cycle plus one explicit cycle must start from fresh state"
        );
    }
}

#[test]
fn execute_does_not_mutate_the_stateful_loaded_harness() {
    let workspace = HarnessWorkspace::new("execute-isolation");
    let mut server = workspace.server();
    server
        .harness_load(
            serde_json::from_value(json!({
                "inline_sources": inline_source(passthrough_source())
            }))
            .unwrap(),
        )
        .unwrap();
    server
        .harness_set_input(
            serde_json::from_value(json!({
                "name": "input",
                "value": bool_value(true)
            }))
            .unwrap(),
        )
        .unwrap();
    server
        .harness_cycle(serde_json::from_value(json!({"count": 1, "watch": []})).unwrap())
        .unwrap();

    let isolated = server
        .execute(
            "harness.execute",
            Some(json!({
                "inline_sources": inline_source(passthrough_source()),
                "steps": [{"op": "cycle", "count": 1}],
                "watch": ["output"]
            })),
        )
        .unwrap();
    assert_eq!(
        isolated["watchSnapshot"]["values"]["output"]["value"],
        bool_value(false)
    );

    let persistent = server
        .harness_get_output(serde_json::from_value(json!({"name": "output"})).unwrap())
        .unwrap();
    assert_eq!(persistent["value"], bool_value(true));
}

#[test]
fn stateful_methods_before_load_preserve_not_loaded_error() {
    let workspace = HarnessWorkspace::new("not-loaded");
    let mut server = workspace.server();

    let errors = [
        server
            .harness_cycle(serde_json::from_value(json!({"count": 1, "watch": []})).unwrap())
            .unwrap_err(),
        server
            .harness_get_output(serde_json::from_value(json!({"name": "q"})).unwrap())
            .unwrap_err(),
        server
            .harness_advance_time(serde_json::from_value(json!({"duration_ms": 1})).unwrap())
            .unwrap_err(),
    ];

    for error in errors {
        assert_eq!(error.code, ERROR_HARNESS_NOT_LOADED);
    }
}

#[test]
fn execute_failure_projection_preserves_step_and_error_order() {
    let step = HarnessExecuteStep::Cycle {
        count: 3,
        dt_ms: Some(10),
    };
    let failure = harness_execute_failure(
        Some(4),
        Some(&step),
        HarnessAutomationError::RuntimeCycle {
            message: "cycle failed".into(),
            errors: vec!["first".into(), "second".into()],
        },
        None,
    );
    let value = serde_json::to_value(failure).unwrap();

    assert_eq!(value["kind"], "runtime_cycle_error");
    assert_eq!(value["stepIndex"], 4);
    assert_eq!(value["step"]["op"], "cycle");
    assert_eq!(value["step"]["count"], 3);
    assert_eq!(value["step"]["dt_ms"], 10);
    assert_eq!(value["errors"], json!(["first", "second"]));
}

#[test]
fn execute_result_failure_order_is_stable() {
    let result = build_harness_execute_result(
        1,
        0,
        0,
        2,
        empty_watch_snapshot(),
        vec![
            failure("assertion_failed", None, None, None),
            failure("unresolved_name", None, None, None),
        ],
    );
    let value = serde_json::to_value(result).unwrap();

    assert_eq!(value["failures"][0]["kind"], "assertion_failed");
    assert_eq!(value["failures"][1]["kind"], "unresolved_name");
}
