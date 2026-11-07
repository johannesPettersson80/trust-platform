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
