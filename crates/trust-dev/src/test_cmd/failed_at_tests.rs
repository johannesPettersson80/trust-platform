//! Where a failed test is reported (specification 22, "Test reporting and CI output"; specification 44 §8.6): at the
//! statement that raised its failure or error, by the program image's statement table.
use super::*;

/// `src/tests.st`, its lines numbered by hand: the assertion of `FailsHere` is on line 6, the assertion of
/// `FailsAfterACall` on line 10, the call of the block in `FailsInABlock` on line 17 and the division of `Divides` on line
/// 25.
const TESTS: &str = "TEST_PROGRAM FailsHere
VAR
    Count : INT;
END_VAR
Count := Count + 1;
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := Count);
END_TEST_PROGRAM

TEST_PROGRAM FailsAfterACall
ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Twice(INT#2));
END_TEST_PROGRAM

TEST_PROGRAM FailsInABlock
VAR
    Check : CheckPositive;
END_VAR
Check(Value := INT#-1);
END_TEST_PROGRAM

TEST_PROGRAM Divides
VAR
    Divisor : DINT;
    Result : DINT;
END_VAR
Result := DINT#10 / Divisor;
END_TEST_PROGRAM

TEST_PROGRAM Passes
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
";

/// `src/lib.st`: the body of `Twice` is on line 5 and the assertion of `CheckPositive` on line 12.
const LIB: &str = "FUNCTION Twice : INT
VAR_INPUT
    Value : INT;
END_VAR
Twice := Value * INT#2;
END_FUNCTION

FUNCTION_BLOCK CheckPositive
VAR_INPUT
    Value : INT;
END_VAR
ASSERT_TRUE(Value > INT#0);
END_FUNCTION_BLOCK
";

fn project(label: &str) -> std::path::PathBuf {
    let project = trust_test_support::unique_temp_path(&format!("trust-dev-{label}"));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(project.join("src/tests.st"), TESTS).expect("write the tests");
    std::fs::write(project.join("src/lib.st"), LIB).expect("write the library");
    project
}

/// A path relative to the project folder as the reports write it, with the platform's separators.
fn relative(path: &str) -> String {
    path.split('/')
        .fold(std::path::PathBuf::new(), |path, part| path.join(part))
        .display()
        .to_string()
}

/// Worked by hand from the two files: `FailsHere` fails at its assertion, `src/tests.st` line 6, since `Count` is 1 after
/// the cold restart; `FailsAfterACall` at its assertion, line 10, though `Twice` ran its statement on `src/lib.st` line 5
/// last; `FailsInABlock` at the block's assertion, `src/lib.st` line 12, where -1 is not above 0; `Divides` ends in error at
/// its division, line 25; `Passes` names no statement. Catches a failure reported at the test's declaration, as before, and
/// a failure after a call reported at the callee's last statement, which a call that does not give its caller's statement
/// back records (specification 44 §8.6).
#[test]
fn a_failed_test_names_the_statement_that_raised_its_failure_on_this_computer() {
    let project = project("failed-at");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let results =
        execute_native_tests(&project, &discovered, &discovered, 5).expect("the compiled run");
    // The places as the reports name them: the statement's file relative to the project folder.
    let places: Vec<_> = results
        .iter()
        .map(|result| {
            (
                result.case.name.to_string(),
                result.outcome,
                failure_place(&project, result).map(|place| (place.file, place.line)),
            )
        })
        .collect();
    let _ = std::fs::remove_dir_all(&project);
    let at = |file: &str, line: usize| Some((relative(file), line));
    assert_eq!(
        places,
        [
            (
                "FailsHere".to_string(),
                TestOutcome::Failed,
                at("src/tests.st", 6)
            ),
            (
                "FailsAfterACall".to_string(),
                TestOutcome::Failed,
                at("src/tests.st", 10)
            ),
            (
                "FailsInABlock".to_string(),
                TestOutcome::Failed,
                at("src/lib.st", 12)
            ),
            (
                "Divides".to_string(),
                TestOutcome::Error,
                at("src/tests.st", 25)
            ),
            ("Passes".to_string(), TestOutcome::Passed, None),
        ]
    );
}

/// Specification 22: the human output and TAP report a failed test at its statement, JSON and JUnit keep the test's
/// declaration and add the statement. Worked by hand for `FailsInABlock`, declared on `src/tests.st` line 13 and failing on
/// `src/lib.st` line 12, whose text is `ASSERT_TRUE(Value > INT#0);`. Catches a report that still names the declaration's
/// line or source where the statement's belongs, and a JSON or JUnit report that moved the test's own file and line.
#[test]
fn every_report_names_the_failing_statement() {
    let project = project("failed-at-reports");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let block: Vec<_> = discovered
        .iter()
        .filter(|case| case.name == "FailsInABlock")
        .cloned()
        .collect();
    let results = execute_native_tests(&project, &discovered, &block, 5).expect("the compiled run");
    let summary = summarize_results(&results);
    let render = |output| {
        render_output(output, &project, &results, summary, results.len(), None, 1)
            .expect("the report")
    };
    let (human, tap, json, junit) = (
        render(TestOutput::Human),
        render(TestOutput::Tap),
        render(TestOutput::Json),
        render(TestOutput::Junit),
    );
    let _ = std::fs::remove_dir_all(&project);
    let (lib, tests) = (relative("src/lib.st"), relative("src/tests.st"));

    assert!(
        human.contains(&format!("TEST_PROGRAM::FailsInABlock {lib}:12 ")),
        "{human}"
    );
    assert!(
        human.contains("    source   : ASSERT_TRUE(Value > INT#0);\n"),
        "{human}"
    );
    assert!(human.contains(&format!("@ {lib}:12\n")), "{human}");
    assert!(
        tap.contains(&format!(
            "# file: {lib}\n# line: 12\n# source: ASSERT_TRUE(Value > INT#0);\n"
        )),
        "{tap}"
    );
    let json: serde_json::Value = serde_json::from_str(&json).expect("the JSON report");
    let test = &json["tests"][0];
    assert_eq!(
        (test["file"].as_str(), test["line"].as_u64()),
        (Some(tests.as_str()), Some(13)),
        "{json}"
    );
    assert_eq!(
        test["failure"],
        serde_json::json!({"file": lib, "line": 12, "source": "ASSERT_TRUE(Value > INT#0);"}),
        "{json}"
    );
    assert!(
        junit.contains(&format!("file=\"{tests}\" line=\"13\"")),
        "{junit}"
    );
    assert!(
        junit.contains(&format!(
            "\nat: {lib}:12\nsource: ASSERT_TRUE(Value &gt; INT#0);</failure>"
        )),
        "{junit}"
    );
}

/// The values of a failed `ASSERT_TRUE` as a device's step names them (specification 11 §6.8.1).
fn assertion_true_failed() -> serde_json::Value {
    json!({
        "name": "ASSERT_TRUE",
        "expected": {"type": "BOOL", "value": true},
        "actual": {"type": "BOOL", "value": false},
    })
}

/// A device behind a TCP bridge that runs nothing and answers each test's steps as a device whose program raised the
/// failures would (specification 11 §6.8.1): `FailsOnce` fails in its step at `src/tests.st` line 2, and
/// `FailsInCycleTwo` passes its first cycle and fails in its second at line 10, each naming the statement in
/// `error_at` and the failed `ASSERT_TRUE`'s values in `assertion`. Every other request succeeds, a status names the
/// state that `restart` and `shutdown` set, and every reply writes `id` first, as a device does.
fn failing_device() -> String {
    use std::io::{BufRead, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind the device's bridge");
    let port = format!(
        "tcp://{}",
        listener.local_addr().expect("the bridge's address")
    );
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the link connects");
        let mut replies = stream.try_clone().expect("the bridge's stream");
        let (mut state, mut cycle) = ("stopped", 0_u32);
        let failed = |line: u32| {
            json!({
                "errors": ["assertion failed: ASSERT_TRUE expected TRUE, got FALSE"],
                "error_at": {"file": "src/tests.st", "line": line},
                "assertion": assertion_true_failed(),
                "values": {},
            })
        };
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let request: serde_json::Value = serde_json::from_str(&line).expect("a JSON request");
            let params = &request["params"];
            let result = match request["type"].as_str().unwrap_or_default() {
                "status" => json!({"state": state, "version": env!("CARGO_PKG_VERSION")}),
                "restart" => {
                    state = "running";
                    json!({})
                }
                "shutdown" => {
                    state = "stopped";
                    json!({})
                }
                "transfer.begin" => json!({"transfer": "t1"}),
                "cycle.step" => {
                    cycle = if params["restart"] == "cold" {
                        1
                    } else {
                        cycle + 1
                    };
                    match (params["run"].as_str().unwrap_or_default(), cycle) {
                        ("FailsOnce", _) => failed(2),
                        ("FailsInCycleTwo", 2) => failed(10),
                        _ => json!({"errors": [], "values": {}}),
                    }
                }
                _ => json!({}),
            };
            let reply = format!(r#"{{"id":{},"ok":true,"result":{result}}}"#, request["id"]);
            if writeln!(replies, "{reply}").is_err() {
                break;
            }
        }
    });
    port
}

/// Specification 22: a device's run names the failing statement and the failed assertion's values as this computer's
/// does, from the statement its step names in `error_at` and the values in `assertion` (specification 11 §6.8.1). Worked
/// by hand from the fake device's replies: `FailsOnce` fails at `src/tests.st` line 2, and `FailsInCycleTwo` fails in its
/// second cycle at line 10, each an `ASSERT_TRUE` that expected TRUE and found FALSE. Catches a device run that drops the
/// statement and reports the test's declaration, lines 1 and 5, and one that drops the values.
#[test]
fn a_device_run_names_the_statement_its_step_names() {
    let project = trust_test_support::unique_temp_path("trust-dev-device-failed-at");
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM FailsOnce\nASSERT_TRUE(FALSE);\nEND_TEST_PROGRAM\n\nTEST_PROGRAM FailsInCycleTwo\nVAR\n    Cycles : INT;\n\
         END_VAR\nCycles := Cycles + 1;\nASSERT_TRUE(Cycles < 2);\nIF Cycles >= 3 THEN\n    TEST_FINISHED();\nEND_IF\nEND_TEST_PROGRAM\n",
    )
    .expect("write the tests");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let results = execute_native_tests_on_device(
        &project,
        &discovered,
        &discovered,
        5,
        &failing_device(),
        "esp32c6",
        None,
    )
    .expect("the run on the device");
    let _ = std::fs::remove_dir_all(&project);

    let places: Vec<_> = results
        .iter()
        .map(|result| {
            (
                result.case.name.to_string(),
                result.outcome,
                result
                    .failed_at
                    .as_ref()
                    .map(|at| (at.file.to_string(), at.line)),
            )
        })
        .collect();
    assert_eq!(
        places,
        [
            (
                "FailsOnce".to_string(),
                TestOutcome::Failed,
                Some(("src/tests.st".to_string(), 2))
            ),
            (
                "FailsInCycleTwo".to_string(),
                TestOutcome::Failed,
                Some(("src/tests.st".to_string(), 10))
            ),
        ]
    );
    let values: Vec<_> = results
        .iter()
        .map(|result| result.assertion.clone())
        .collect();
    assert_eq!(
        values,
        [Some(assertion_true_failed()), Some(assertion_true_failed())]
    );
}
