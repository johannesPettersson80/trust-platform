//! A failed assertion's values in the JSON report (specification 22, "Test reporting and CI output"; specification 07,
//! MP-014): its name and its typed inputs.
use super::*;

const TESTS: &str = "TEST_PROGRAM EqualInts
ASSERT_EQUAL(EXPECTED := INT#2, ACTUAL := INT#1);
END_TEST_PROGRAM

TEST_PROGRAM IsTrue
ASSERT_TRUE(FALSE);
END_TEST_PROGRAM

TEST_PROGRAM Greater
ASSERT_GREATER(VALUE := DINT#3, BOUND := DINT#5);
END_TEST_PROGRAM

TEST_PROGRAM Near
ASSERT_NEAR(EXPECTED := LREAL#1.0, ACTUAL := LREAL#1.5, DELTA := LREAL#0.25);
END_TEST_PROGRAM

TEST_PROGRAM Texts
ASSERT_EQUAL(EXPECTED := 'on', ACTUAL := 'off');
END_TEST_PROGRAM

TEST_PROGRAM Durations
ASSERT_EQUAL(EXPECTED := T#1s, ACTUAL := T#500ms);
END_TEST_PROGRAM

TEST_PROGRAM Divides
VAR
    Divisor : DINT;
END_VAR
Divisor := DINT#10 / Divisor;
END_TEST_PROGRAM

TEST_PROGRAM Passes
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
";

/// Specification 22: the JSON report gives a failed assertion's name and its inputs typed as a conformance artifact
/// writes a value. Worked by hand from the sources: `ASSERT_EQUAL` expected the INT 2 and found 1; `ASSERT_TRUE` expected
/// TRUE and found FALSE; `ASSERT_GREATER`'s VALUE 3 is its actual and 5 its bound; `ASSERT_NEAR` compared the LREALs 1.0
/// and 1.5 with the delta 0.25; the texts are the STRINGs 'on' and 'off'; the durations are 1 s and 500 ms in nanoseconds.
/// A division by zero and a passing test carry no assertion. Catches the values left out or untyped, the expected and the
/// actual swapped, a comparison's bound reported as its expected value, a text read from anywhere but the stack area,
/// and an assertion object on a test that no assertion failed.
#[test]
fn the_json_report_gives_a_failed_assertions_typed_values() {
    let project =
        trust_test_support::unique_temp_path(&format!("trust-dev-t2b-{}", std::process::id()));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(project.join("src/tests.st"), TESTS).expect("write the tests");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let results =
        execute_native_tests(&project, &discovered, &discovered, 5).expect("the compiled run");
    let summary = summarize_results(&results);
    let report = render_json_output(&project, &results, summary, 1).expect("the JSON report");
    let _ = std::fs::remove_dir_all(&project);

    let report: serde_json::Value = serde_json::from_str(&report).expect("JSON");
    let assertions: Vec<_> = report["tests"]
        .as_array()
        .expect("the tests")
        .iter()
        .map(|test| {
            (
                test["name"].as_str().unwrap_or_default().to_string(),
                test["assertion"].clone(),
            )
        })
        .collect();
    let typed = |ty: &str, value: serde_json::Value| json!({"type": ty, "value": value});
    assert_eq!(
        assertions,
        [
            (
                "EqualInts".to_string(),
                json!({"name": "ASSERT_EQUAL", "expected": typed("INT", json!(2)), "actual": typed("INT", json!(1))}),
            ),
            (
                "IsTrue".to_string(),
                json!({"name": "ASSERT_TRUE", "expected": typed("BOOL", json!(true)), "actual": typed("BOOL", json!(false))}),
            ),
            (
                "Greater".to_string(),
                json!({"name": "ASSERT_GREATER", "actual": typed("DINT", json!(3)), "bound": typed("DINT", json!(5))}),
            ),
            (
                "Near".to_string(),
                json!({
                    "name": "ASSERT_NEAR",
                    "expected": typed("LREAL", json!(1.0)),
                    "actual": typed("LREAL", json!(1.5)),
                    "delta": typed("LREAL", json!(0.25)),
                }),
            ),
            (
                "Texts".to_string(),
                json!({"name": "ASSERT_EQUAL", "expected": typed("STRING", json!("on")), "actual": typed("STRING", json!("off"))}),
            ),
            (
                "Durations".to_string(),
                json!({
                    "name": "ASSERT_EQUAL",
                    "expected": {"type": "TIME", "nanos": 1_000_000_000},
                    "actual": {"type": "TIME", "nanos": 500_000_000},
                }),
            ),
            ("Divides".to_string(), serde_json::Value::Null),
            ("Passes".to_string(), serde_json::Value::Null),
        ]
    );
}
