//! Multi-cycle tests (specification 22, "Multi-cycle tests"; specification 07, `TEST_FINISHED`), run compiled for this computer:
//! each expected cycle and time is worked by hand from the specification's rule that cycle `n` runs at the time `(n − 1)·cycle`.

use super::*;

/// The results of running the tests `run` of a project whose only source is `source`.
fn results(label: &str, source: &str, run: &[&str]) -> Vec<ExecutedTest> {
    let project = trust_test_support::unique_temp_path(&format!(
        "trust-dev-cycles-{label}-{}",
        std::process::id()
    ));
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
}

/// The outcome, message and cycles of `result`.
fn outcome(result: &ExecutedTest) -> (TestOutcome, String, Option<(u32, u64)>) {
    (
        result.outcome,
        result.message.clone().unwrap_or_default(),
        result
            .cycles
            .map(|run| (run.cycles, run.time_ns / 1_000_000)),
    )
}

/// A timer test: `TON` with `PT := T#50ms`, started in the first cycle. At the default 10 ms cycle, cycle `n` runs at
/// `(n − 1)·10 ms`, so the timer's elapsed time reaches 50 ms in cycle 6, which finishes the test there at 50 ms. Catches a test
/// that runs its body once (the timer never ends), a time taken from the wall clock (the six cycles take microseconds), and a
/// test that runs on after `TEST_FINISHED()`.
#[test]
fn a_multi_cycle_test_runs_in_simulated_time_until_it_calls_test_finished() {
    let source = r#"
TEST_PROGRAM TimerEnds
VAR
    Delay : TON;
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
Delay(IN := TRUE, PT := T#50ms);
IF Delay.Q THEN
    ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Cycles);
    TEST_FINISHED();
END_IF
END_TEST_PROGRAM
"#;
    let results = results("timer", source, &["TimerEnds"]);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((6, 50)))
    );
}

/// The budget (`test_max_cycles`) bounds a test that never finishes: 20 cycles, the last at `19·10 ms = 190 ms`, and the test
/// fails with that. Catches a test that runs on past its budget (a hang), and one that passes because nothing failed.
#[test]
fn a_test_that_does_not_finish_within_its_budget_fails() {
    let source = r#"
{attribute 'test_max_cycles' := '20'}
TEST_PROGRAM NeverFinishes
VAR
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
END_TEST_PROGRAM
"#;
    let results = results("budget", source, &["NeverFinishes"]);
    assert_eq!(
        outcome(&results[0]),
        (
            TestOutcome::Failed,
            "did not finish within 20 cycles (t = T#190ms)".to_owned(),
            Some((20, 190))
        )
    );
}

/// A failed assertion ends a multi-cycle test at its cycle, named with the time: at a 100 ms cycle (`test_cycle`), the count
/// reaches 3 in cycle 3, at `2·100 ms = 200 ms`. A `PROGRAM` with `{attribute 'test'}`, the CODESYS form, runs alike. Catches the
/// cycle or the time missing from the message, the cycle attribute ignored (the time would read 20 ms), and the attribute form
/// running once.
#[test]
fn a_failed_assertion_names_its_cycle_and_time() {
    let source = r#"
{attribute 'test'}
{attribute 'test_cycle' := 'T#100ms'}
PROGRAM PRG_TestCountStaysBelowThree
VAR
    Count : INT;
END_VAR
Count := Count + 1;
ASSERT_TRUE(Count < 3);
IF Count >= 10 THEN
    TEST_FINISHED();
END_IF
END_PROGRAM
"#;
    let results = results("assertion", source, &["PRG_TestCountStaysBelowThree"]);
    assert_eq!(
        outcome(&results[0]),
        (
            TestOutcome::Failed,
            "ASSERT_TRUE expected TRUE, got FALSE (cycle 3, t = T#200ms)".to_owned(),
            Some((3, 200))
        )
    );
}

/// A test that neither calls `TEST_FINISHED()` nor names a cycle runs its body once, as before, and reports no cycles. Catches
/// every test turned into a multi-cycle one (each would fail its budget).
#[test]
fn a_test_without_test_finished_runs_once() {
    let source = r#"
TEST_PROGRAM RunsOnce
VAR
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cycles);
END_TEST_PROGRAM
"#;
    let results = results("once", source, &["RunsOnce"]);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), None)
    );
}

/// A device behind a TCP bridge that steps as specification 11 §6.8.1 says and runs nothing: its resource state follows
/// `restart` and `shutdown`, an install's transfer is taken whole, a cold restart after an install starts the stepped time
/// at zero, and a step whose time is before the previous step's is refused (`time before the last step`). The finished mark
/// reads TRUE from the sixth step after a cold restart, as in a test that finishes in its sixth cycle. Records each step's
/// time.
fn stepping_device() -> (String, std::sync::Arc<std::sync::Mutex<Vec<u64>>>) {
    use std::io::{BufRead, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind the device's bridge");
    let port = format!(
        "tcp://{}",
        listener.local_addr().expect("the bridge's address")
    );
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = std::sync::Arc::clone(&steps);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the link connects");
        let mut replies = stream.try_clone().expect("the bridge's stream");
        let (mut state, mut last, mut installed, mut since_cold) =
            ("stopped", None::<u64>, false, 0_u32);
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let request: serde_json::Value = serde_json::from_str(&line).expect("a JSON request");
            let params = &request["params"];
            let result = match request["type"].as_str().unwrap_or_default() {
                "status" => Ok(json!({"state": state, "version": env!("CARGO_PKG_VERSION")})),
                "restart" => {
                    state = "running";
                    Ok(json!({}))
                }
                "shutdown" => {
                    state = "stopped";
                    Ok(json!({}))
                }
                "transfer.begin" => Ok(json!({"transfer": "t1"})),
                "program.install" => {
                    installed = true;
                    Ok(json!({}))
                }
                "cycle.step" => {
                    let time = params["time_ns"].as_u64().unwrap_or_default();
                    let cold = params["restart"] == "cold";
                    if cold && installed {
                        installed = false;
                        last = None;
                    }
                    if last.is_some_and(|last| time < last) {
                        Err("time before the last step")
                    } else {
                        last = Some(time);
                        since_cold = if cold { 1 } else { since_cold + 1 };
                        recorded.lock().expect("the steps").push(time);
                        Ok(json!({
                            "errors": [],
                            "values": {trust_runtime::native::TEST_FINISHED_GLOBAL: {"value": since_cold >= 6}},
                        }))
                    }
                }
                _ => Ok(json!({})),
            };
            // A device writes `id` first, and the link takes a reply by its leading `{"id"`.
            let id = &request["id"];
            let reply = match result {
                Ok(result) => format!(r#"{{"id":{id},"ok":true,"result":{result}}}"#),
                Err(error) => format!(r#"{{"id":{id},"ok":false,"error":"{error}"}}"#),
            };
            if writeln!(replies, "{reply}").is_err() {
                break;
            }
        }
    });
    (port, steps)
}

/// Specification 11 §6.8.1 and specification 22 ("Multi-cycle tests"), found on the C6 on 8 November: the stepped scans
/// keep one logical time across the tests, so after a multi-cycle test the next test starts where its last step was, not at
/// zero. Worked by hand for a multi-cycle test that finishes in its sixth cycle of 10 ms and a single-cycle test after it:
/// the steps run at 0, 10, 20, 30, 40 and 50 ms, and the single-cycle test's step at 50 ms; both pass, and the multi-cycle
/// test reports its own six cycles and 50 ms. Catches the single-cycle test stepped at zero after the multi-cycle test's
/// 50 ms, which the device refused with `time before the last step`.
#[test]
fn on_a_device_each_test_starts_where_the_previous_test_left_the_stepped_time() {
    let project = trust_test_support::unique_temp_path(&format!(
        "trust-dev-device-time-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM Finishes\nVAR\n    Cycles : INT;\nEND_VAR\nCycles := Cycles + 1;\nIF Cycles >= 6 THEN\n    \
         TEST_FINISHED();\nEND_IF\nEND_TEST_PROGRAM\n\nTEST_PROGRAM RunsOnce\nEND_TEST_PROGRAM\n",
    )
    .expect("write the tests");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let (port, steps) = stepping_device();
    let results =
        execute_native_tests_on_device(&project, &discovered, &discovered, 5, &port, "esp32c6", None)
            .expect("the run on the device");
    let _ = std::fs::remove_dir_all(&project);

    let outcomes: Vec<_> = results
        .iter()
        .map(|result| (result.case.name.to_string(), outcome(result)))
        .collect();
    assert_eq!(
        outcomes,
        [
            (
                "Finishes".to_string(),
                (TestOutcome::Passed, String::new(), Some((6, 50)))
            ),
            (
                "RunsOnce".to_string(),
                (TestOutcome::Passed, String::new(), None)
            ),
        ]
    );
    let millis: Vec<u64> = steps
        .lock()
        .expect("the steps")
        .iter()
        .map(|time| time / 1_000_000)
        .collect();
    assert_eq!(millis, [0, 10, 20, 30, 40, 50, 50]);
}
