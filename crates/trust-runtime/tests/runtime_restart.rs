//! Restarts of a program in process: the logical time, the retained and the other variables, and
//! the task overruns a warm restart must not report again.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::env;

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = env::temp_dir();
    let pid = std::process::id();
    path.push(format!("trust_runtime_restart_{pid}_{name}.bin"));
    path
}

#[test]
fn in_process_restart_preserves_monotonic_time() {
    let source = r#"
PROGRAM Main
VAR
    x : INT := INT#0;
END_VAR
x := x + INT#1;
END_PROGRAM
"#;

    for mode in [RestartMode::Warm, RestartMode::Cold] {
        let mut harness = TestHarness::from_source(source).unwrap();
        harness.advance_time(Duration::from_millis(5));
        harness.restart(mode).unwrap();
        assert_eq!(harness.current_time(), Duration::from_millis(5));
    }
}

#[test]
fn restart_with_retain_store_persists_values() {
    let source = r#"
PROGRAM Main
VAR RETAIN
    r : INT := 1;
END_VAR
VAR
    u : INT := 2;
END_VAR
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).unwrap();
    let path = temp_path("retain");
    let store = FileRetainStore::new(&path);
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(store)), Some(Duration::from_millis(1)))
        .expect("the retain file is set");
    harness.cycle();

    harness.set_input("r", Value::Int(42));
    harness.set_input("u", Value::Int(7));
    harness
        .runtime_mut()
        .save_retain_store()
        .expect("save retain");

    harness
        .restart_with_retain(RestartMode::Warm)
        .expect("restart warm");
    assert_eq!(harness.get_output("r"), Some(Value::Int(42)));
    assert_eq!(harness.get_output("u"), Some(Value::Int(2)));

    let _ = std::fs::remove_file(path);
}

#[test]
fn cold_restart_initializes_retain_values() {
    let source = r#"
PROGRAM Main
VAR RETAIN
    r : INT := 1;
END_VAR
VAR
    u : INT := 2;
END_VAR
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).unwrap();
    harness.set_input("r", Value::Int(42));
    harness.set_input("u", Value::Int(7));

    harness.restart(RestartMode::Cold).expect("cold restart");

    assert_eq!(harness.get_output("r"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("u"), Some(Value::Int(2)));
}

#[test]
fn warm_restart_preserves_retain_and_initializes_nonretain() {
    let source = r#"
PROGRAM Main
VAR RETAIN
    r : INT := 1;
END_VAR
VAR NON_RETAIN
    nr : INT := 2;
END_VAR
VAR
    u : INT := 3;
END_VAR
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).unwrap();
    harness.set_input("r", Value::Int(42));
    harness.set_input("nr", Value::Int(7));
    harness.set_input("u", Value::Int(8));

    harness.restart(RestartMode::Warm).expect("warm restart");

    assert_eq!(harness.get_output("r"), Some(Value::Int(42)));
    assert_eq!(harness.get_output("nr"), Some(Value::Int(2)));
    assert_eq!(
        harness.get_output("u"),
        Some(Value::Int(3)),
        "truST initializes unqualified variables on warm restart"
    );
}

#[test]
fn warm_restart_does_not_report_pre_restart_time_as_new_overrun() {
    let source = r#"
CONFIGURATION C
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH Fast : Main;
END_CONFIGURATION

PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    // The overruns of the task "Fast" reported since the previous call, as missed activations.
    let missed = || -> u64 {
        debug
            .drain_runtime_events()
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::TaskOverrun { name, missed, .. } if name.as_str() == "Fast" => {
                    Some(*missed)
                }
                _ => None,
            })
            .sum()
    };

    clock.advance(Duration::from_millis(30));
    runner.tick().expect("pre-restart cycle");
    assert_eq!(missed(), 2);

    runner.restart(RestartMode::Warm).expect("warm restart");
    let _ = missed();
    runner.tick().expect("first post-restart cycle");

    assert_eq!(
        missed(),
        0,
        "pre-restart elapsed time must not produce a new phantom overrun"
    );
}
