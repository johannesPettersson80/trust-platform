//! IEC 61131-3 Ed.3 §6.5.6.2 and §6.5.2.2, specification 10 §4.8: a library block kept as a whole
//! keeps its own state over a warm restart and a power loss, so that the restart fabricates no edge
//! and a running timer keeps counting; only its time baseline starts over, so the first call after
//! the restart counts no time (`docs/IEC_DECISIONS.md`, 2026-07-12 and 2026-10-30).
//!
//! Every value is worked out by hand. Each scan advances the task time by 10 ms. In the three scans
//! before the restart, IN of the `TON` and the `TP` and CU and CLK stay TRUE from the first scan,
//! and the `TOF`'s IN falls in the second:
//! - `Delay` (`TON`, PT 100 ms): the first call samples the rising edge, ET 0; then 10 and 20 ms;
//! - `OffDelay` (`TOF`, PT 100 ms): Q TRUE; the falling-edge call gives ET 0; then 10 ms, Q TRUE;
//! - `Pulse` (`TP`, PT 100 ms): the pulse starts at ET 0; then 10 and 20 ms, Q TRUE;
//! - `Count` (`CTU`): one rising CU, CV 1;
//! - `Edge` (`R_TRIG`): Q TRUE in the first scan only.
//!
//! After the restart the first scan counts no time and fabricates no edge: ET stays 20, 10 and
//! 20 ms with Q FALSE, TRUE and TRUE, CV stays 1 and `Edge.Q` stays FALSE; the second scan adds
//! 10 ms to each timer. A block whose hidden state started over would give `Delay` and `Pulse` an
//! edge at ET 0, end the `TOF`'s delay at once (Q FALSE), count a second CU edge (CV 2) and pulse
//! `Edge.Q`.

use trust_runtime::harness::TestHarness;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const SOURCE: &str = r#"
PROGRAM Main
VAR RETAIN
    Phase : INT;
    Delay : TON;
    OffDelay : TOF;
    Pulse : TP;
    Count : CTU;
    Edge : R_TRIG;
END_VAR
    Delay(IN := TRUE, PT := T#100ms);
    OffDelay(IN := Phase = 0, PT := T#100ms);
    Pulse(IN := TRUE, PT := T#100ms);
    Count(CU := TRUE, R := FALSE, PV := INT#10);
    Edge(CLK := TRUE);
    Phase := Phase + 1;
END_PROGRAM

CONFIGURATION Config
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

fn scan(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
}

fn ms(value: i64) -> Value {
    Value::Time(Duration::from_millis(value))
}

/// The blocks after `scans` scans following the restart (1 or 2).
fn assert_continued(harness: &TestHarness, scans: i64) {
    let more = 10 * (scans - 1);
    for (path, expected, wrong) in [
        (
            "P1.Delay.ET",
            ms(20 + more),
            "the TON keeps counting, no new edge",
        ),
        (
            "P1.Delay.Q",
            Value::Bool(false),
            "the TON has not reached PT",
        ),
        (
            "P1.OffDelay.ET",
            ms(10 + more),
            "the TOF's off-delay keeps counting",
        ),
        (
            "P1.OffDelay.Q",
            Value::Bool(true),
            "the TOF's off-delay still runs",
        ),
        (
            "P1.Pulse.ET",
            ms(20 + more),
            "the TP's pulse keeps counting, no new edge",
        ),
        ("P1.Pulse.Q", Value::Bool(true), "the TP's pulse still runs"),
        (
            "P1.Count.CV",
            Value::Int(1),
            "the CTU counts no fabricated edge",
        ),
        (
            "P1.Edge.Q",
            Value::Bool(false),
            "the R_TRIG fabricates no edge",
        ),
    ] {
        assert_eq!(value(harness, path), expected, "{path}: {wrong}");
    }
}

fn before_restart() -> TestHarness {
    TestHarness::from_source(SOURCE).expect("compile and initialize")
}

fn three_scans_checked(harness: &mut TestHarness) {
    for _ in 0..3 {
        scan(harness);
    }
    assert_eq!(
        value(harness, "P1.Delay.ET"),
        ms(20),
        "the TON's ET before the restart"
    );
    assert_eq!(
        value(harness, "P1.OffDelay.ET"),
        ms(10),
        "the TOF's ET before the restart"
    );
    assert_eq!(
        value(harness, "P1.Pulse.ET"),
        ms(20),
        "the TP's ET before the restart"
    );
    assert_eq!(
        value(harness, "P1.Count.CV"),
        Value::Int(1),
        "one CU edge before the restart"
    );
}

#[test]
fn a_warm_restart_keeps_the_state_of_retained_library_blocks() {
    let mut harness = before_restart();
    three_scans_checked(&mut harness);
    harness.restart(RestartMode::Warm).expect("warm restart");
    scan(&mut harness);
    assert_continued(&harness, 1);
    scan(&mut harness);
    assert_continued(&harness, 2);
}

#[test]
fn retained_library_blocks_keep_their_state_over_a_power_loss() {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "trust_retain_library_blocks_{}_power_loss.bin",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut before = before_restart();
    before
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(FileRetainStore::new(&path))),
            Some(Duration::from_millis(1)),
        )
        .expect("set the retain store");
    three_scans_checked(&mut before);
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
    scan(&mut after);
    assert_continued(&after, 1);
    scan(&mut after);
    assert_continued(&after, 2);
    let _ = std::fs::remove_file(&path);
}
