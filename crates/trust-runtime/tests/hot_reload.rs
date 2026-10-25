//! Online change of a compiled program (specification 11 §6.7.1; specification 34 §11.3;
//! specification 44 §12): the change commits at a cycle boundary, the new program keeps the values
//! the running one holds, restarts its body and the scan counter, continues the task schedule, and
//! a refused change leaves the running program as it was.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::io::{IoAddress, IoDriver};
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{ResourceCommand, ResourceRunner, StdClock};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RetainSnapshot;
use trust_runtime_core::native::change::CarriedValueEvent;

/// The debug image of `source` for this computer.
fn image(source: &str) -> Vec<u8> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!(
        "trust-hot-reload-{}-{}",
        std::process::id(),
        source.len()
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

fn assert_numeric_output(harness: &TestHarness, name: &str, expected: i64) {
    let value = numeric_output(harness, name).unwrap_or_else(|| panic!("missing output '{name}'"));
    assert_eq!(value, expected, "unexpected value for '{name}'");
}

fn numeric_output(harness: &TestHarness, name: &str) -> Option<i64> {
    let actual = harness.get_output(name)?;
    Some(match actual {
        Value::Int(v) => i64::from(v),
        Value::DInt(v) => i64::from(v),
        Value::LInt(v) => v,
        other => panic!("unexpected numeric type for '{name}': {other:?}"),
    })
}

#[derive(Debug)]
struct SleepOnReadDriver {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    reads: usize,
}

impl SleepOnReadDriver {
    fn new(entered: mpsc::Sender<()>, release: mpsc::Receiver<()>) -> Self {
        Self {
            entered,
            release,
            reads: 0,
        }
    }
}

impl IoDriver for SleepOnReadDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if self.reads == 0 {
            let _ = self.entered.send(());
            let _ = self.release.recv_timeout(StdDuration::from_secs(2));
        }
        self.reads = self.reads.saturating_add(1);
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Specification 11 §6.7.1: a change commits at a cycle boundary, never inside a cycle. The first
/// cycle blocks in its input read; a change queued meanwhile is not answered while that cycle runs
/// (no reply within 40 ms), and is answered with success once the read is released and the cycle
/// ends. Wrong behaviour caught: a change applied in the middle of a running cycle.
#[test]
fn hot_reload_cycle_boundary_contract_holds_for_vm_reload() {
    let source_v1 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 10;
END_PROGRAM
"#;

    let mut runtime = TestHarness::from_source(source_v1)
        .expect("build the program")
        .into_runtime();
    let updated_bytes = image(source_v2);
    let (read_entered_tx, read_entered_rx) = mpsc::channel();
    let (release_read_tx, release_read_rx) = mpsc::channel();
    runtime.add_io_driver(
        "sleep-on-first-read",
        Box::new(SleepOnReadDriver::new(read_entered_tx, release_read_rx)),
    );

    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("hot-reload-cycle-boundary").expect("spawn");
    let control = handle.control();

    read_entered_rx
        .recv_timeout(StdDuration::from_secs(1))
        .expect("runtime should enter the first blocking input read");

    let (tx, rx) = mpsc::channel();
    let queued_at = Instant::now();
    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes: updated_bytes,
            claim: trust_runtime::scheduler::ReloadClaim::default(),
            respond_to: tx,
        })
        .expect("queue reload command");

    let early = rx.recv_timeout(StdDuration::from_millis(40));
    assert!(
        matches!(early, Err(RecvTimeoutError::Timeout)),
        "reload should not complete while cycle is in-flight"
    );
    release_read_tx
        .send(())
        .expect("release first blocking input read");

    let result = rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("reload response");
    assert!(result.is_ok(), "reload failed: {result:?}");
    assert!(
        queued_at.elapsed() >= StdDuration::from_millis(40),
        "reload should complete only after cycle-boundary handoff"
    );

    handle.stop();
    handle.join().expect("join resource thread");
}

#[test]
fn hot_reload_keeps_retained_and_other_values_and_instances() {
    // Specification 11 §6.7.1 (decision OC-1): an online change keeps every variable at the same
    // place, retained or not, and the members of function-block instances. The test sets g_ret 30,
    // g_plain 40, p_ret 50, p_plain 60 and the instance member g_fb.count 7, and changes the program
    // to the same declarations: all five values are kept. Wrong behaviour caught: variables that are
    // not retained reset to their initial values (g_plain 2, p_plain 4) and an instance built anew
    // (count 0), which was the rule before decision OC-1.
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    g_ret : INT := 1;
END_VAR
VAR_GLOBAL
    g_plain : INT := 2;
    g_fb : CounterFb;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

FUNCTION_BLOCK CounterFb
VAR
    count : INT := 0;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR RETAIN
    p_ret : INT := 3;
END_VAR
VAR
    p_plain : INT := 4;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("build the program");

    harness.set_input("g_ret", Value::Int(30));
    harness.set_input("g_plain", Value::Int(40));
    harness.set_input("p_ret", Value::Int(50));
    harness.set_input("p_plain", Value::Int(60));
    harness.set_input("g_fb.count", Value::Int(7));

    harness.reload_source(source).expect("apply online change");

    assert_numeric_output(&harness, "g_ret", 30);
    assert_numeric_output(&harness, "g_plain", 40);
    assert_numeric_output(&harness, "p_ret", 50);
    assert_numeric_output(&harness, "p_plain", 60);
    assert_eq!(
        harness.get_output("g_fb.count"),
        Some(Value::Int(7)),
        "the instance keeps its member's value"
    );
}

/// Specification 11 §6.7.1: the commit rebinds I/O. A program that writes `FALSE` to its output
/// `%QX0.0` is changed to one that writes `TRUE`; the output image shows `FALSE` after the first
/// cycle and `TRUE` after the first cycle of the changed program. Wrong behaviour caught: a binding
/// lost or left on the old program at the change.
#[test]
fn hot_reload_rebinds_instance_backed_io_after_warm_restart() {
    let source_v1 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    let output = IoAddress::parse("%QX0.0").expect("an output address");
    let mut harness = TestHarness::from_source(source_v1).expect("build the program");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "initial cycle: {:?}", cycle.errors);
    assert_eq!(
        harness.runtime().read_io(&output).expect("read %QX0.0"),
        Value::Bool(false)
    );

    harness
        .reload_source(source_v2)
        .expect("apply online change");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "post-reload cycle: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.runtime().read_io(&output).expect("read %QX0.0"),
        Value::Bool(true)
    );
}

/// Specification 11 §6.7.1: a changed body runs from its entry point on the cycle after the change,
/// the scan counter restarts at 0, and the variable keeps its running value (decision OC-1): `count`
/// is 3 after the change and 3 + 10 = 13 after the next cycle. Wrong behaviour caught: a body that
/// resumes mid-way, a scan counter that goes on, and a value reset to its initial 0 (the rule before
/// checklist C1-24).
#[test]
fn hot_reload_changed_body_restarts_at_entrypoint_policy() {
    let source_v1 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let source_v2 = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 10;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source_v1).expect("build the program");
    harness.cycle();
    harness.cycle();
    harness.cycle();
    assert_numeric_output(&harness, "count", 3);
    assert_eq!(harness.runtime().cycle_counter(), 3);

    harness
        .reload_source(source_v2)
        .expect("apply online change");

    assert_eq!(harness.runtime().cycle_counter(), 0);
    assert_numeric_output(&harness, "count", 3);

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected errors after reload: {:?}",
        cycle.errors
    );
    assert_numeric_output(&harness, "count", 13);
}

/// A retain store that holds `snapshot` and counts the loads of it.
struct CountingRetainStore {
    snapshot: RetainSnapshot,
    loads: Arc<AtomicUsize>,
}

impl RetainStore for CountingRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(self.snapshot.clone())
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[test]
fn hot_reload_keeps_the_running_retained_values_and_reads_no_retain_store() {
    // Specification 11 §6.7.1 and specification 34 §11.3 (decision OC-1): an online change keeps
    // the retained values that the running program holds, and reads no retain store: the store
    // holds older values, and stays the source of a restart after a stop or a power loss. The
    // running program holds `g_ret` 30 and `p_ret` 50; the store holds 5 and 6 for them. After the
    // change the candidate holds 30 and 50, and the store was never loaded. Wrong behaviour caught:
    // a change that loads the store (one load, or a refusal when the load fails) and applies its
    // older values over the running ones (5 and 6).
    let source = r#"
CONFIGURATION Conf
VAR_GLOBAL RETAIN
    g_ret : INT := 1;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION

PROGRAM Main
VAR RETAIN
    p_ret : INT := 3;
END_VAR
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("build the program");
    harness.set_input("g_ret", Value::Int(30));
    harness.set_input("p_ret", Value::Int(50));
    let mut stored = RetainSnapshot::default();
    stored.insert("g_ret", Value::Int(5));
    stored.insert("@program/P1/p_ret", Value::Int(6));
    let loads = Arc::new(AtomicUsize::new(0));
    harness
        .runtime_mut()
        .set_retain_store(
            Some(Box::new(CountingRetainStore {
                snapshot: stored,
                loads: Arc::clone(&loads),
            })),
            None,
        )
        .expect("the retain store is set");
    let loads_before = loads.load(Ordering::SeqCst);

    harness.reload_source(source).expect("apply online change");

    assert_numeric_output(&harness, "g_ret", 30);
    assert_numeric_output(&harness, "p_ret", 50);
    assert_eq!(
        loads.load(Ordering::SeqCst),
        loads_before,
        "the change does not read the store"
    );
}

#[test]
fn hot_reload_retained_value_that_does_not_fit_takes_its_initial_value() {
    // Specification 11 §6.7.1 ("keep what fits", owner 20 October 2026): a variable whose new
    // declaration does not hold the running value takes its declared initial value, the change
    // records it as an event, and no value refuses the change. The running program holds the
    // retained `limited` 100, which its `INT` takes, and its program variable `marker` 1 after one
    // scan; the candidate declares `limited : INT(0..10) := INT#0` and adds 10 to `marker`. After the
    // change `limited` is 0, `marker` keeps 1 and the scan counter restarts at 0; the next scan makes
    // `marker` 11. Wrong behaviour caught: a change refused because of a value (the rule before the
    // owner's decision), a value cut to the subrange (10), and a value that does not fit kept as it
    // is (100 outside 0..10).
    let source_v1 = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
VAR_GLOBAL RETAIN
    limited : INT := INT#0;
END_VAR
TASK T (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
VAR
    marker : DINT := DINT#0;
END_VAR
marker := marker + DINT#1;
END_PROGRAM
"#;

    let source_v2 = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
VAR_GLOBAL RETAIN
    limited : INT(0..10) := INT#0;
END_VAR
TASK T (INTERVAL := T#1ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
VAR
    marker : DINT := DINT#0;
END_VAR
marker := marker + DINT#10;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source_v1).expect("build the program");
    harness.advance_time(Duration::from_millis(1));
    let first_cycle = harness.cycle();
    assert!(
        first_cycle.errors.is_empty(),
        "initial cycle failed: {first_cycle:?}"
    );
    assert_numeric_output(&harness, "P1.marker", 1);
    assert_eq!(harness.runtime().cycle_counter(), 1);

    harness.set_input("limited", Value::Int(100));

    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    harness
        .reload_source(source_v2)
        .expect("a value that does not fit refuses no change");
    assert_numeric_output(&harness, "limited", 0);
    assert_numeric_output(&harness, "P1.marker", 1);
    assert_eq!(
        harness.runtime().cycle_counter(),
        0,
        "the committed change restarts the scan counter"
    );
    let not_kept = debug
        .drain_runtime_events()
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                RuntimeEvent::OnlineChangeValue {
                    event: CarriedValueEvent::NotKept { name },
                    ..
                } if name.eq_ignore_ascii_case("limited")
            )
        })
        .count();
    assert_eq!(not_kept, 1, "the change records the value it did not keep");

    harness.advance_time(Duration::from_millis(1));
    let next_cycle = harness.cycle();
    assert!(
        next_cycle.errors.is_empty(),
        "the new program runs: {next_cycle:?}"
    );
    assert_numeric_output(&harness, "P1.marker", 11);
    assert_eq!(harness.runtime().cycle_counter(), 1);
}

/// Specification 11 §6.7.1: bytes that are no program image fail the preparation as
/// `InvalidBundle`, with the admission's reason, and the running program stays. Wrong behaviour
/// caught: a change that accepts any bytes, or reports another kind of failure.
#[test]
fn hot_reload_invalid_module_reports_deterministic_error() {
    let source = r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source)
        .expect("build the program")
        .into_runtime();
    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("hot-reload-invalid-image").expect("spawn");
    let control = handle.control();
    let (tx, rx) = mpsc::channel();

    control
        .send_command(ResourceCommand::ReloadBytecode {
            bytes: vec![0x00, 0x01, 0x02, 0x03],
            claim: trust_runtime::scheduler::ReloadClaim::default(),
            respond_to: tx,
        })
        .expect("send reload command");

    let result = rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("reload response");
    let err = result.expect_err("reload should fail");
    assert!(
        matches!(err, RuntimeError::InvalidBundle(_)),
        "expected the admission's refusal, got {err:?}"
    );

    handle.stop();
    handle.join().expect("join resource thread");
}

#[test]
fn hot_reload_new_task_counts_its_first_activation_from_the_last_cycle_before_the_change() {
    // Specification 11 §6.7.1 and specification 34 §11.3 (decision OC-2): a task that the running
    // program does not have counts its first activation from the program's logical time at the
    // commit, the time of the last cycle that started before it, since the program's time advances
    // when a cycle starts. The running program's 10 ms task `Fast` runs at 10, 20 and 30 ms. The
    // change follows the cycle at 30 ms, as a computer's scan thread takes it before the cycle at
    // 40 ms, and adds the 25 ms task `Slow` with the program `P2`. `Slow` counts from 30 ms,
    // so it is due at 55 ms: it has not run after the cycles at 40 and 50 ms, runs at 60 ms, and
    // runs again at 80 ms, its next activation being due at 80 ms; `Fast` keeps its schedule and
    // runs in every cycle. Wrong behaviour caught: a new task counted from the program's start (due
    // at 25 ms, so it runs at 40 ms) or run at the first cycle after the change, and one counted
    // from that cycle (due at 65 ms, so it has not run at 60 ms).
    let source_v1 = r#"
CONFIGURATION Conf
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION

PROGRAM Counter
VAR
    runs : DINT := 0;
END_VAR
runs := runs + 1;
END_PROGRAM
"#;
    let source_v2 = r#"
CONFIGURATION Conf
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#25ms, PRIORITY := 2);
    PROGRAM P1 WITH Fast : Counter;
    PROGRAM P2 WITH Slow : Late;
END_RESOURCE
END_CONFIGURATION

PROGRAM Counter
VAR
    runs : DINT := 0;
END_VAR
runs := runs + 1;
END_PROGRAM

PROGRAM Late
VAR
    runs : DINT := 0;
END_VAR
runs := runs + 1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source_v1).expect("build the program");
    for _ in 0..3 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "the scan runs: {cycle:?}");
    }
    assert_numeric_output(&harness, "P1.runs", 3);
    assert_eq!(harness.runtime().current_time(), Duration::from_millis(30));

    harness
        .reload_source(source_v2)
        .expect("apply online change");

    // The cycles at 40, 50, 60, 70 and 80 ms.
    let mut slow_runs = Vec::new();
    for _ in 0..5 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "the scan runs: {cycle:?}");
        slow_runs.push(numeric_output(&harness, "P2.runs").expect("P2.runs"));
    }
    assert_eq!(slow_runs, [0, 0, 1, 1, 2], "Slow's runs after each cycle");
    assert_numeric_output(&harness, "P1.runs", 8);
}
