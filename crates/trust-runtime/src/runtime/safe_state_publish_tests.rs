//! The safe state of a stop or a fault is what the reads between scans show (specification 11,
//! "Safe state in the reads"; checklist G228): Live Values and `io.read` answer with the outputs as
//! the safe state drove them, marked as the safe state, with the number of the last scan that
//! completed, and the event `safe_state_applied` records each output with its value.

use crate::debug::{DebugControl, RuntimeEvent};
use crate::harness::TestHarness;
use crate::io::{IoAddress, IoSafeState, IoSnapshot, IoSnapshotValue};
use crate::value::{Duration, Value};

/// A program that drives its lamp on.
const LAMP_ON: &str = "PROGRAM Main
VAR
    Lamp AT %QX0.0 : BOOL;
END_VAR
    Lamp := TRUE;
END_PROGRAM
";

/// The value `snapshot` shows for the output `%QX0.0`.
fn lamp(snapshot: &IoSnapshot) -> Value {
    let lamp = IoAddress::parse("%QX0.0").expect("the address parses");
    let entry = snapshot
        .outputs
        .iter()
        .find(|entry| entry.address == lamp)
        .unwrap_or_else(|| panic!("the snapshot shows the lamp: {snapshot:?}"));
    match &entry.value {
        IoSnapshotValue::Value(value) => value.clone(),
        other => panic!("a value, not {other:?}"),
    }
}

/// Catches the G228 finding on a computer: after a stop's safe state the published snapshot still
/// showed the program's last scan (`%QX0.0 TRUE`) although the drivers had the safe image
/// (`FALSE`), nothing marked it, and no event recorded the safe state.
#[test]
fn a_stops_safe_state_is_what_the_reads_show_and_an_event_records() {
    let mut harness = TestHarness::from_source(LAMP_ON).expect("the program compiles");
    let runtime = harness.runtime_mut();
    let debug = DebugControl::new();
    runtime.set_debug_control(debug.clone());
    runtime.set_io_safe_state(IoSafeState {
        outputs: vec![(
            IoAddress::parse("%QX0.0").expect("the address parses"),
            Value::Bool(false),
        )],
    });
    runtime.set_current_time(Duration::from_millis(10));
    runtime.execute_cycle().expect("the cycle runs");
    let scan = debug.latest_io_snapshot().expect("the scan's snapshot");
    assert_eq!(lamp(&scan), Value::Bool(true));
    assert!(!scan.safe_state, "a scan's snapshot has no mark");
    let _ = debug.drain_runtime_events();

    runtime
        .apply_io_safe_state()
        .expect("the safe state is applied");
    let after = debug
        .latest_io_snapshot()
        .expect("the safe state's snapshot");
    assert_eq!(lamp(&after), Value::Bool(false), "the outputs as driven");
    assert!(after.safe_state, "marked as the safe state");
    assert_eq!(after.scan, scan.scan, "the last completed scan's number");
    let applied: Vec<_> = debug
        .drain_runtime_events()
        .into_iter()
        .filter_map(|event| match event {
            RuntimeEvent::SafeStateApplied { outputs, .. } => Some(outputs),
            _ => None,
        })
        .collect();
    assert_eq!(
        applied,
        [vec![(
            IoAddress::parse("%QX0.0").expect("the address parses"),
            smol_str::SmolStr::new("Bool(false)"),
        )]]
    );
}
