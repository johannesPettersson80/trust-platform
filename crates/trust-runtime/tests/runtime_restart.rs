use std::env;

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
fn retain_rules() {
    let source = r#"
PROGRAM Main
VAR
    x : INT := INT#0;
END_VAR
x := x + INT#1;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.advance_time(Duration::from_millis(5));
    harness.restart(RestartMode::Warm).unwrap();
    assert_eq!(harness.current_time(), Duration::ZERO);
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
        .set_retain_store(Some(Box::new(store)), Some(Duration::from_millis(1)));
    harness.cycle();

    harness.set_input("r", Value::Int(42));
    harness.set_input("u", Value::Int(7));
    harness.runtime_mut().mark_retain_dirty();
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

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    clock.advance(Duration::from_millis(30));
    runner.tick().expect("pre-restart cycle");
    assert_eq!(runner.runtime().task_overrun_count("Fast"), Some(2));

    runner.restart(RestartMode::Warm).expect("warm restart");
    let overrun_count_after_restart = runner.runtime().task_overrun_count("Fast");
    runner.tick().expect("first post-restart cycle");

    assert_eq!(
        runner.runtime().task_overrun_count("Fast"),
        overrun_count_after_restart,
        "pre-restart elapsed time must not produce a new phantom overrun"
    );
}
