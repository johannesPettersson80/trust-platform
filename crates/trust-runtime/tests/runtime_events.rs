//! The runtime events of a scan: the cycle and task boundaries, a task overrun and a fault.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

/// The runtime of `source` with a debug control attached, which receives its events.
fn debugged(source: &str) -> (CompiledRuntime, DebugControl) {
    let mut runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let control = DebugControl::new();
    runtime.set_debug_control(control.clone());
    (runtime, control)
}

#[test]
fn runtime_events_include_cycle_and_task() {
    let (mut runtime, control) = debugged(
        r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    count : INT := 0;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM I WITH T : P;
END_RESOURCE
END_CONFIGURATION

PROGRAM P
count := count + 1;
END_PROGRAM
"#,
    );
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();
    runtime.execute_cycle().unwrap();

    let events = control.drain_runtime_events();
    let mut iter = events.iter();
    assert!(matches!(iter.next(), Some(RuntimeEvent::CycleStart { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::TaskStart { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::TaskEnd { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::CycleEnd { .. })));
}

#[test]
fn runtime_event_overrun_emitted() {
    let (mut runtime, control) = debugged(
        r#"
CONFIGURATION C
VAR_GLOBAL
    count : INT := 0;
END_VAR
RESOURCE R ON PLC
    TASK T (INTERVAL := T#10ms, PRIORITY := 0);
    PROGRAM I WITH T : P;
END_RESOURCE
END_CONFIGURATION

PROGRAM P
count := count + 1;
END_PROGRAM
"#,
    );
    runtime.reset_task_timing(Duration::ZERO);
    runtime.set_current_time(Duration::from_millis(35));
    runtime.execute_cycle().unwrap();

    let events = control.drain_runtime_events();
    assert!(events
        .iter()
        .any(|event| matches!(event, RuntimeEvent::TaskOverrun { missed: 2, .. })));
}

#[test]
fn runtime_event_fault_emitted() {
    let (mut runtime, control) = debugged(
        r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    zero : INT := 0;
    x : INT := 0;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM I WITH T : P;
END_RESOURCE
END_CONFIGURATION

PROGRAM P
x := INT#1 / zero;
END_PROGRAM
"#,
    );
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();
    assert!(runtime.execute_cycle().is_err());

    let events = control.drain_runtime_events();
    assert!(events
        .iter()
        .any(|event| matches!(event, RuntimeEvent::Fault { .. })));
    assert!(runtime.faulted());
}
