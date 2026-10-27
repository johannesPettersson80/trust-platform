//! IEC 61131-3 Ed.3 §6.5.6.1 and §6.5.6.2, specification 10 §4.8: an instance declared `RETAIN`,
//! alone or as an element of an array, keeps its variables over a warm restart and a power loss.
//! A nested instance that is not qualified is kept with it; a variable or nested instance its
//! type declares `NON_RETAIN` starts over, also in the instances a `RETAIN` global carries over
//! a warm restart of the running runtime.

use trust_runtime::harness::TestHarness;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
FUNCTION_BLOCK Meter
VAR_OUTPUT
    Total : DINT;
END_VAR
VAR NON_RETAIN
    Scans : DINT;
END_VAR
    Total := Total + 1;
    Scans := Scans + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Station
VAR
    Inner : Meter;
END_VAR
VAR NON_RETAIN
    Spare : Meter;
END_VAR
    Inner();
    Spare();
END_FUNCTION_BLOCK

PROGRAM Main
VAR_EXTERNAL
    Line : ARRAY[1..2] OF Meter;
    Plant : Station;
END_VAR
VAR RETAIN
    Cells : ARRAY[0..1] OF Station;
END_VAR
    Line[2]();
    Plant();
    Cells[1]();
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL RETAIN
    Line : ARRAY[1..2] OF Meter;
    Plant : Station;
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

fn value(harness: &TestHarness, path: &str) -> Value {
    harness
        .get_output(path)
        .unwrap_or_else(|| panic!("{path:?} exists"))
}

fn three_scans(harness: &mut TestHarness) {
    for _ in 0..3 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
}

/// Every called meter has counted three scans; what is kept now holds 3 and the rest 0.
fn assert_kept_and_started_over(harness: &TestHarness) {
    for (path, expected, wrong) in [
        (
            "Line[2].Total",
            3,
            "an element of a RETAIN global instance array keeps its variables",
        ),
        (
            "Line[2].Scans",
            0,
            "a NON_RETAIN variable of an array element starts over",
        ),
        (
            "Plant.Inner.Total",
            3,
            "a nested instance that is not qualified is kept with its RETAIN instance",
        ),
        (
            "Plant.Inner.Scans",
            0,
            "a NON_RETAIN variable of a nested instance starts over",
        ),
        (
            "Plant.Spare.Total",
            0,
            "a nested instance declared NON_RETAIN starts over",
        ),
        (
            "P1.Cells[1].Inner.Total",
            3,
            "an element of a RETAIN program instance array keeps its nested instance",
        ),
        (
            "P1.Cells[1].Inner.Scans",
            0,
            "a NON_RETAIN variable starts over in a program array element",
        ),
        (
            "P1.Cells[1].Spare.Total",
            0,
            "a NON_RETAIN nested instance starts over in a program array element",
        ),
    ] {
        assert_eq!(
            value(harness, path),
            Value::DInt(expected),
            "{path}: {wrong}"
        );
    }
}

#[test]
fn a_warm_restart_keeps_retain_instance_trees_and_starts_over_their_non_retain_parts() {
    let mut harness = TestHarness::from_source(SOURCE).expect("compile and initialize");
    three_scans(&mut harness);
    harness.restart(RestartMode::Warm).expect("warm restart");
    assert_kept_and_started_over(&harness);
}

#[test]
fn retain_instance_trees_survive_a_power_loss_without_their_non_retain_parts() {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "trust_retain_instance_trees_{}_power_loss.bin",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut before = TestHarness::from_source(SOURCE).expect("compile and initialize");
    before
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&path))),
            Some(Duration::from_millis(1)),
        )
        .expect("set the retain store");
    three_scans(&mut before);
    before
        .runtime_mut()
        .save_retain_store()
        .expect("save the retain store");
    drop(before);

    // A power loss: a new runtime of the same program reads the store at start.
    let mut after = TestHarness::from_source(SOURCE).expect("compile and initialize again");
    after
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&path))),
            Some(Duration::from_millis(1)),
        )
        .expect("set the retain store");
    after
        .restart_with_retain(RestartMode::Warm)
        .expect("load the retain store");
    assert_kept_and_started_over(&after);
    let _ = std::fs::remove_file(&path);
}

/// IEC 61131-3 Ed.3 §6.5.6.2: on a warm restart a variable that is not kept takes its
/// variable-specific initial value first, the one an instance initializer gives it, before its
/// type's. The `NON_RETAIN` input of a `RETAIN` global instance initialized `(Speed := 50)`, which
/// the program's calls set to 60, starts over at 50, not at its type's 10, while the instance's
/// unqualified output is kept: a warm restart takes every variable that is not kept from the
/// starting memory, which holds the instance initializer's values (specification 44 §6.7).
/// Catches a warm restart that gives such a variable its type's initial value, or keeps it.
#[test]
fn a_warm_restart_gives_a_variable_that_is_not_kept_its_instance_initial_value() {
    const DRIVE: &str = r#"
FUNCTION_BLOCK Drive
VAR_INPUT NON_RETAIN
    Speed : INT := 10;
END_VAR
VAR_OUTPUT
    Runs : DINT;
END_VAR
    Runs := Runs + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR_EXTERNAL
    Fan : Drive;
END_VAR
    Fan(Speed := 60);
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL RETAIN
    Fan : Drive := (Speed := 50);
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let mut harness = TestHarness::from_source(DRIVE).expect("compile and initialize");
    three_scans(&mut harness);
    assert_eq!(value(&harness, "Fan.Speed"), Value::Int(60));
    harness.restart(RestartMode::Warm).expect("warm restart");
    assert_eq!(
        value(&harness, "Fan.Runs"),
        Value::DInt(3),
        "the unqualified output of a RETAIN instance is kept"
    );
    assert_eq!(
        value(&harness, "Fan.Speed"),
        Value::Int(50),
        "a NON_RETAIN variable starts over at the instance initializer's value"
    );
}
