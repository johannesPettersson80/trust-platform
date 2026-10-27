//! Task scheduling of a program's configuration (IEC 61131-3 §6.8.2; specification 10 sections 4.3
//! to 4.6): periodic intervals, SINGLE events, priorities, background programs, missed intervals
//! and the logical clock.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// `count` counted up by the program `P` that the configuration `tasks` runs; `tasks` declares the
/// task `T` and the instance `P1` of `P`.
fn counting(globals: &str, tasks: &str) -> TestHarness {
    TestHarness::from_source(&format!(
        "CONFIGURATION C\nVAR_GLOBAL\n    count : INT := 0;\n{globals}END_VAR\n{tasks}\n\
         END_CONFIGURATION\n\nPROGRAM P\ncount := count + 1;\nEND_PROGRAM\n"
    ))
    .expect("the program compiles")
}

fn scan(harness: &mut TestHarness) {
    harness.runtime_mut().execute_cycle().unwrap();
}

fn global(harness: &TestHarness, name: &str) -> Value {
    harness
        .runtime()
        .read_variable(name)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn set_trigger(harness: &mut TestHarness, value: bool) {
    harness
        .runtime_mut()
        .write_variable("trigger", &Value::Bool(value))
        .unwrap();
}

/// A debug control attached to the harness's runtime, which receives its events.
fn debugged(harness: &mut TestHarness) -> DebugControl {
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
}

/// The missed activations of `task` reported since the previous call.
fn missed(debug: &DebugControl, task: &str) -> u64 {
    debug
        .drain_runtime_events()
        .iter()
        .filter_map(|event| match event {
            RuntimeEvent::TaskOverrun { name, missed, .. } if name.as_str() == task => {
                Some(*missed)
            }
            _ => None,
        })
        .sum()
}

#[test]
fn periodic_interval() {
    let mut harness = counting(
        "",
        "TASK T (INTERVAL := T#10ms, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );

    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(0));

    harness.advance_time(Duration::from_millis(5));
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(0));

    harness.advance_time(Duration::from_millis(5));
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(1));
}

#[test]
fn interval_zero_disables_periodic() {
    let mut harness = counting(
        "",
        "TASK T (INTERVAL := T#0ms, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );

    harness.advance_time(Duration::from_millis(20));
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(0));
}

#[test]
fn event_single_rise() {
    let mut harness = counting(
        "    trigger : BOOL := FALSE;\n",
        "TASK T (SINGLE := trigger, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );

    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(0));

    set_trigger(&mut harness, true);
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(1));

    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(1));

    set_trigger(&mut harness, false);
    scan(&mut harness);
    set_trigger(&mut harness, true);
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(2));
}

#[test]
fn single_blocks_periodic() {
    let mut harness = counting(
        "    trigger : BOOL := TRUE;\n",
        "TASK T (SINGLE := trigger, INTERVAL := T#10ms, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );

    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(0));

    set_trigger(&mut harness, false);
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(global(&harness, "count"), Value::Int(1));
}

#[test]
fn event_edge_coalescing_between_samples() {
    let mut harness = counting(
        "    trigger : BOOL := FALSE;\n",
        "TASK T (SINGLE := trigger, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );

    set_trigger(&mut harness, true);
    set_trigger(&mut harness, false);
    set_trigger(&mut harness, true);
    scan(&mut harness);

    assert_eq!(global(&harness, "count"), Value::Int(1));
}

/// `A` sets `x` to 1 and `B` sets `y` to `x + 1`, so `y` is 2 exactly when `A` ran first.
const ORDERED: &str = "
PROGRAM A
x := 1;
END_PROGRAM

PROGRAM B
y := x + 1;
END_PROGRAM
";

#[test]
fn priority_order() {
    let mut harness = TestHarness::from_source(&format!(
        "CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    x : INT := 0;
    y : INT := 0;
END_VAR
TASK TaskA (SINGLE := trigger, PRIORITY := 0);
TASK TaskB (SINGLE := trigger, PRIORITY := 10);
PROGRAM IA WITH TaskA : A;
PROGRAM IB WITH TaskB : B;
END_CONFIGURATION
{ORDERED}"
    ))
    .unwrap();

    set_trigger(&mut harness, true);
    scan(&mut harness);
    assert_eq!(global(&harness, "y"), Value::Int(2));
}

#[test]
fn background_programs() {
    let mut harness = TestHarness::from_source(
        "CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    count : INT := 0;
    bg : INT := 0;
END_VAR
TASK T (SINGLE := trigger, PRIORITY := 1);
PROGRAM TaskInst WITH T : TaskProg;
PROGRAM BgInst : BgProg;
END_CONFIGURATION

PROGRAM TaskProg
count := count + 1;
END_PROGRAM

PROGRAM BgProg
bg := bg + 1;
END_PROGRAM
",
    )
    .unwrap();

    scan(&mut harness);
    assert_eq!(global(&harness, "bg"), Value::Int(1));
    assert_eq!(global(&harness, "count"), Value::Int(0));

    set_trigger(&mut harness, true);
    scan(&mut harness);
    assert_eq!(global(&harness, "bg"), Value::Int(2));
    assert_eq!(global(&harness, "count"), Value::Int(1));
}

#[test]
fn fifo_order_by_due_time_within_priority() {
    let mut harness = TestHarness::from_source(&format!(
        "CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
    x : INT := 0;
    y : INT := 0;
END_VAR
TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
TASK Event (SINGLE := trigger, PRIORITY := 0);
PROGRAM IA WITH Periodic : A;
PROGRAM IB WITH Event : B;
END_CONFIGURATION
{ORDERED}"
    ))
    .unwrap();

    harness.advance_time(Duration::from_millis(20));
    set_trigger(&mut harness, true);
    scan(&mut harness);

    assert_eq!(global(&harness, "y"), Value::Int(2));
}

#[test]
fn task_overrun_drops_missed_intervals() {
    let mut harness = counting(
        "",
        "TASK T (INTERVAL := T#10ms, PRIORITY := 0);\nPROGRAM P1 WITH T : P;",
    );
    let debug = debugged(&mut harness);

    harness.advance_time(Duration::from_millis(35));
    scan(&mut harness);

    assert_eq!(global(&harness, "count"), Value::Int(1));
    assert_eq!(missed(&debug, "T"), 2);
}

/// Injected logical samples exercise IEC 61131-3 Ed.3 section 6.8.2 b.
/// These varying offsets are a manual-clock trace, not measured hosted wakeups.
#[test]
fn periodic_task_keeps_its_interval_when_cycles_start_late() {
    let mut harness = counting(
        "",
        "TASK T (INTERVAL := T#100ms, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );
    let debug = debugged(&mut harness);

    // Ten seconds of 50 ms slots, each cycle starting 70 µs or 20 µs after its slot.
    for slot in 1..=200_i64 {
        let late_us = if slot % 4 < 2 { 70 } else { 20 };
        harness
            .runtime_mut()
            .set_current_time(Duration::from_nanos(slot * 50_000_000 + late_us * 1_000));
        scan(&mut harness);
    }

    assert_eq!(
        global(&harness, "count"),
        Value::Int(100),
        "one run per 100 ms"
    );
    assert_eq!(missed(&debug, "T"), 0);
}

#[test]
fn periodic_task_non_multiple_interval_preserves_nominal_deadlines() {
    let mut harness = counting(
        "",
        "TASK T (INTERVAL := T#25ms, PRIORITY := 1);\nPROGRAM P1 WITH T : P;",
    );
    let debug = debugged(&mut harness);

    let mut activations = Vec::new();
    let mut previous = 0;
    for sample in (10..=1000).step_by(10) {
        harness
            .runtime_mut()
            .set_current_time(Duration::from_millis(sample));
        scan(&mut harness);
        let Value::Int(count) = global(&harness, "count") else {
            panic!("expected an INT counter");
        };
        if count != previous {
            assert_eq!(count, previous + 1, "never replay missed activations");
            activations.push(sample);
            previous = count;
        }
        assert_eq!(missed(&debug, "T"), 0);
    }
    assert_eq!(&activations[..4], &[30, 50, 80, 100]);
    assert_eq!(activations.len(), 40);
    assert_eq!(previous, 40);
    assert_eq!(activations.last(), Some(&1000));
}

#[test]
fn simulation_time_advance_saturates_at_duration_limit() {
    let mut harness = TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n").unwrap();
    harness
        .runtime_mut()
        .set_current_time(Duration::from_nanos(i64::MAX - 5));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        harness.advance_time(Duration::from_nanos(10));
    }));

    assert!(result.is_ok(), "simulation-time overflow must not panic");
    assert_eq!(harness.current_time(), Duration::from_nanos(i64::MAX));
}
