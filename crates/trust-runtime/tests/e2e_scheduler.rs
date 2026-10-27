//! Periodic and event tasks on the resource scheduler (specification 10 §4.3): readiness, missed
//! intervals, backward clock steps, faults and a deterministic event trace.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use std::sync::{Arc, Mutex};

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

fn assert_counter(value: Result<Value, RuntimeError>, expected: i64) {
    match value {
        Ok(Value::Int(v)) => assert_eq!(i64::from(v), expected),
        Ok(Value::DInt(v)) => assert_eq!(i64::from(v), expected),
        Ok(Value::LInt(v)) => assert_eq!(v, expected),
        other => panic!("unexpected counter value {other:?}"),
    }
}

/// The program of `source` with a metrics sink, which counts each task's missed intervals.
fn with_metrics(source: &str) -> (CompiledRuntime, Arc<Mutex<RuntimeMetrics>>) {
    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    runtime.set_metrics_sink(Arc::clone(&metrics));
    (runtime, metrics)
}

/// The missed intervals the metrics counted for `task`, once the task ran.
fn overruns(metrics: &Mutex<RuntimeMetrics>, task: &str) -> Option<u64> {
    metrics
        .lock()
        .expect("metrics lock")
        .snapshot()
        .tasks
        .iter()
        .find(|stats| stats.name == task)
        .map(|stats| stats.overruns)
}

#[test]
fn periodic_and_event() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
TASK Event (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : PeriodicProg;
PROGRAM P2 WITH Event : EventProg;
END_CONFIGURATION

PROGRAM PeriodicProg
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM

PROGRAM EventProg
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    let periodic = "P1";
    let event = "P2";

    runner.tick().unwrap();
    assert_counter(
        runner.program().read_variable(&format!("{periodic}.count")),
        0,
    );
    assert_counter(runner.program().read_variable(&format!("{event}.count")), 0);

    runner
        .program_mut()
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();
    clock.advance(Duration::from_millis(10));
    runner.tick().unwrap();

    assert_counter(
        runner.program().read_variable(&format!("{periodic}.count")),
        1,
    );
    assert_counter(runner.program().read_variable(&format!("{event}.count")), 1);

    runner
        .program_mut()
        .write_variable("trigger", &Value::Bool(false))
        .unwrap();
    clock.advance(Duration::from_millis(10));
    runner.tick().unwrap();

    assert_counter(
        runner.program().read_variable(&format!("{periodic}.count")),
        2,
    );
    assert_counter(runner.program().read_variable(&format!("{event}.count")), 1);
}

#[test]
fn overrun_and_fault() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL
    fault_trigger : BOOL := FALSE;
END_VAR
TASK Fast (INTERVAL := T#5ms, PRIORITY := 0);
TASK FaultTask (SINGLE := fault_trigger, INTERVAL := T#0ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : OverrunProg;
PROGRAM P2 WITH FaultTask : FaultProg;
END_CONFIGURATION

PROGRAM OverrunProg
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM

PROGRAM FaultProg
VAR
    x : INT := 0;
END_VAR
x := 1 / 0;
END_PROGRAM
"#;

    let (runtime, metrics) = with_metrics(source);
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    clock.advance(Duration::from_millis(15));
    runner.tick().unwrap();
    assert_eq!(overruns(&metrics, "Fast"), Some(2));

    runner
        .program_mut()
        .write_variable("fault_trigger", &Value::Bool(true))
        .unwrap();
    let err = runner.tick().unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
    assert!(runner.program().faulted());

    let err = runner.tick().unwrap_err();
    assert!(matches!(err, RuntimeError::ResourceFaulted));
}

#[test]
fn backward_clock_step_does_not_replay_or_overrun_periodic_task() {
    let source = r#"
CONFIGURATION C
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P WITH Fast : Main;
END_CONFIGURATION

PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;
    let (runtime, metrics) = with_metrics(source);
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));
    let program = "P";

    clock.set_time(Duration::from_millis(10));
    runner.tick().unwrap();
    assert_counter(
        runner.program().read_variable(&format!("{program}.count")),
        1,
    );

    clock.set_time(Duration::from_millis(2));
    runner.tick().unwrap();
    assert_counter(
        runner.program().read_variable(&format!("{program}.count")),
        1,
    );
    assert_eq!(overruns(&metrics, "Fast"), Some(0));

    clock.set_time(Duration::from_millis(20));
    runner.tick().unwrap();
    assert_counter(
        runner.program().read_variable(&format!("{program}.count")),
        2,
    );
    assert_eq!(overruns(&metrics, "Fast"), Some(0));
}

#[test]
fn resumed_clock_jump_runs_once_and_records_missed_intervals() {
    let source = r#"
CONFIGURATION C
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P WITH Fast : Main;
END_CONFIGURATION

PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;
    let (runtime, metrics) = with_metrics(source);
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));
    let program = "P";

    clock.set_time(Duration::from_millis(55));
    runner.tick().unwrap();

    assert_counter(
        runner.program().read_variable(&format!("{program}.count")),
        1,
    );
    assert_eq!(overruns(&metrics, "Fast"), Some(4));
}

#[test]
fn trace_determinism() {
    fn run_trace() -> Vec<RuntimeEvent> {
        let source = r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
TASK Event (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : PeriodicProg;
PROGRAM P2 WITH Event : EventProg;
END_CONFIGURATION

PROGRAM PeriodicProg
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM

PROGRAM EventProg
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

        runner.tick().unwrap();
        runner
            .program_mut()
            .write_variable("trigger", &Value::Bool(true))
            .unwrap();
        clock.advance(Duration::from_millis(10));
        runner.tick().unwrap();
        runner
            .program_mut()
            .write_variable("trigger", &Value::Bool(false))
            .unwrap();
        clock.advance(Duration::from_millis(10));
        runner.tick().unwrap();

        debug.drain_runtime_events()
    }

    let trace_a = run_trace();
    let trace_b = run_trace();
    assert_eq!(trace_a, trace_b);
}
