use trust_runtime::debug::RuntimeEvent;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::value::{Duration, Value};

fn assert_counter(value: Option<&Value>, expected: i64) {
    match value {
        Some(Value::Int(v)) => assert_eq!(i64::from(*v), expected),
        Some(Value::DInt(v)) => assert_eq!(i64::from(*v), expected),
        Some(Value::LInt(v)) => assert_eq!(*v, expected),
        other => panic!("unexpected counter value {other:?}"),
    }
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

    let periodic_id = match runner.runtime().storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P1 instance, got {other:?}"),
    };
    let event_id = match runner.runtime().storage().get_global("P2") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P2 instance, got {other:?}"),
    };

    runner.tick().unwrap();
    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(periodic_id, "count"),
        0,
    );
    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(event_id, "count"),
        0,
    );

    runner
        .runtime_mut()
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    clock.advance(Duration::from_millis(10));
    runner.tick().unwrap();

    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(periodic_id, "count"),
        1,
    );
    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(event_id, "count"),
        1,
    );

    runner
        .runtime_mut()
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    clock.advance(Duration::from_millis(10));
    runner.tick().unwrap();

    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(periodic_id, "count"),
        2,
    );
    assert_counter(
        runner
            .runtime()
            .storage()
            .get_instance_var(event_id, "count"),
        1,
    );
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

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

    clock.advance(Duration::from_millis(15));
    runner.tick().unwrap();
    assert_eq!(runner.runtime().task_overrun_count("Fast"), Some(2));

    runner
        .runtime_mut()
        .storage_mut()
        .set_global("fault_trigger", Value::Bool(true));
    let err = runner.tick().unwrap_err();
    assert!(matches!(err, RuntimeError::DivisionByZero));
    assert!(runner.runtime().faulted());

    let err = runner.tick().unwrap_err();
    assert!(matches!(err, RuntimeError::ResourceFaulted));
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
        let debug = runtime.enable_debug();
        let clock = ManualClock::new();
        let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));

        runner.tick().unwrap();
        runner
            .runtime_mut()
            .storage_mut()
            .set_global("trigger", Value::Bool(true));
        clock.advance(Duration::from_millis(10));
        runner.tick().unwrap();
        runner
            .runtime_mut()
            .storage_mut()
            .set_global("trigger", Value::Bool(false));
        clock.advance(Duration::from_millis(10));
        runner.tick().unwrap();

        debug.drain_runtime_events()
    }

    let trace_a = run_trace();
    let trace_b = run_trace();
    assert_eq!(trace_a, trace_b);
}
