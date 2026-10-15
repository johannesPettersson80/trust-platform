//! Shared-global synchronization is admitted before a resource starts, and a later transfer
//! that the destination range rejects enters the owning fault policy (spec 34 §4.3.1
//! "shared-global synchronization admission").
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::debug::RuntimeEvent;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner, ResourceState, SharedGlobals};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::FaultPolicy;

/// The source resource owns `shared` at the wider declared domain 0..10 and writes 7 each cycle.
const SOURCE_WRITES_SEVEN: &str = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT(0..10) := INT#0; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main
shared := INT#7;
END_PROGRAM
"#;

/// The source resource starts with 7, which is admissible for its own 0..10 domain.
const SOURCE_INITIAL_SEVEN: &str = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT(0..10) := INT#7; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main END_PROGRAM
"#;

/// The destination declares a narrower domain 0..5 for the same shared name.
const DESTINATION_NARROW: &str = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT(0..5) := INT#0; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main END_PROGRAM
"#;

/// The destination has no declaration named `shared`.
const DESTINATION_WITHOUT_SHARED: &str = r#"
CONFIGURATION C
VAR_GLOBAL other : INT(0..5) := INT#0; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main END_PROGRAM
"#;

fn wait_for_cycle(rx: &std::sync::mpsc::Receiver<RuntimeEvent>, target: u64) {
    let start = Instant::now();
    loop {
        let event = rx
            .recv_timeout(StdDuration::from_millis(100))
            .expect("timed out waiting for runtime event");
        let cycle = match event {
            RuntimeEvent::CycleStart { cycle, .. } | RuntimeEvent::CycleEnd { cycle, .. } => cycle,
            _ => continue,
        };
        if cycle >= target {
            break;
        }
        if start.elapsed() > StdDuration::from_millis(500) {
            panic!("timed out waiting for cycle {target}");
        }
    }
}

fn source_shared(source: &str) -> SharedGlobals {
    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    SharedGlobals::from_runtime(vec!["shared".into()], &runtime).unwrap()
}

/// Spec 34 §4.3.1: a shared-global source name must exist when the shared set is admitted.
#[test]
fn checked_admission_missing_shared_source_name_is_refused_when_admitted() {
    let source = TestHarness::from_source(SOURCE_INITIAL_SEVEN)
        .unwrap()
        .into_runtime();
    let error = SharedGlobals::from_runtime(vec!["missing".into()], &source)
        .expect_err("a missing source name must be refused");
    assert_eq!(error, RuntimeError::UndefinedVariable("missing".into()));
}

/// Spec 34 §4.3.1: a shared-global destination name must exist; the resource is refused
/// before its thread starts.
#[test]
fn checked_admission_missing_shared_destination_name_is_refused_before_start() {
    let shared = source_shared(SOURCE_INITIAL_SEVEN);
    let destination = TestHarness::from_source(DESTINATION_WITHOUT_SHARED)
        .unwrap()
        .into_runtime();
    let starts = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&starts);
    let runner = ResourceRunner::new(destination, ManualClock::new(), Duration::from_millis(10))
        .with_thread_init_hook(Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));
    let Err(error) = runner.spawn_with_shared("missing-destination", shared) else {
        panic!("a missing destination name must be refused")
    };
    // The destination lookup in the shared admission path reports NullReference
    // (vm/engine/access.rs prepare_named_write). This pins the current variant.
    assert_eq!(error, RuntimeError::NullReference);
    assert_eq!(starts.load(Ordering::SeqCst), 0);
}

/// Spec 34 §4.3.1: an initial shared value that the destination range rejects is refused
/// before the resource starts.
#[test]
fn checked_admission_initial_shared_value_outside_destination_range_is_refused_before_start() {
    let shared = source_shared(SOURCE_INITIAL_SEVEN);
    let destination = TestHarness::from_source(DESTINATION_NARROW)
        .unwrap()
        .into_runtime();
    let starts = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&starts);
    let runner = ResourceRunner::new(destination, ManualClock::new(), Duration::from_millis(10))
        .with_thread_init_hook(Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));
    let Err(error) = runner.spawn_with_shared("narrow-initial", shared.clone()) else {
        panic!("an out-of-range initial value must be refused")
    };
    assert_eq!(
        error,
        RuntimeError::SubrangeViolation {
            value: 7,
            lower: 0,
            upper: 5,
        }
    );
    assert_eq!(starts.load(Ordering::SeqCst), 0);
    assert_eq!(shared.get("shared"), Some(Value::Int(7)));
}

/// Spec 34 §4.3.1: each later transfer is checked against the destination range, and the
/// rejection enters the destination's owning fault policy with no write committed.
#[test]
fn checked_admission_later_shared_transfer_outside_destination_range_latches_the_owner_fault() {
    let shared = source_shared(SOURCE_WRITES_SEVEN);
    let mut source = ResourceRunner::new(
        TestHarness::from_source(SOURCE_WRITES_SEVEN)
            .unwrap()
            .into_runtime(),
        ManualClock::new(),
        Duration::from_millis(10),
    );
    let mut destination = TestHarness::from_source(DESTINATION_NARROW)
        .unwrap()
        .into_runtime();
    destination.set_fault_policy(FaultPolicy::Halt);
    let mut runner =
        ResourceRunner::new(destination, ManualClock::new(), Duration::from_millis(10));

    // The admissible initial value 0 is transferred first.
    assert_eq!(runner.tick_with_shared(&shared), Ok(()));
    // The source publishes 7, which is admissible for its own domain only.
    source.tick_with_shared(&shared).unwrap();
    assert_eq!(shared.get("shared"), Some(Value::Int(7)));

    let rejected = RuntimeError::SubrangeViolation {
        value: 7,
        lower: 0,
        upper: 5,
    };
    assert_eq!(runner.tick_with_shared(&shared), Err(rejected.clone()));
    assert!(runner.runtime().faulted());
    assert_eq!(runner.runtime().last_fault(), Some(&rejected));
    assert_eq!(
        runner.runtime().storage().get_global("shared"),
        Some(&Value::Int(0))
    );
    assert_eq!(shared.get("shared"), Some(Value::Int(7)));
    assert_eq!(
        runner.tick_with_shared(&shared),
        Err(RuntimeError::ResourceFaulted)
    );
}

/// Spec 34 §4.3.1: the resource handle reports the same fault state and error as the runtime
/// owner after a later shared transfer is rejected.
#[test]
fn checked_admission_resource_handle_reports_the_owner_fault_for_a_later_shared_transfer() {
    let shared = source_shared(SOURCE_WRITES_SEVEN);
    let mut source = ResourceRunner::new(
        TestHarness::from_source(SOURCE_WRITES_SEVEN)
            .unwrap()
            .into_runtime(),
        ManualClock::new(),
        Duration::from_millis(10),
    );
    let mut destination = TestHarness::from_source(DESTINATION_NARROW)
        .unwrap()
        .into_runtime();
    destination.set_fault_policy(FaultPolicy::Halt);
    let debug = destination.enable_debug();
    let (tx, rx) = std::sync::mpsc::channel();
    debug.set_runtime_sender(tx);
    let clock = ManualClock::new();
    let mut handle = ResourceRunner::new(destination, clock.clone(), Duration::from_millis(10))
        .spawn_with_shared("narrow-transfer", shared.clone())
        .expect("admissible initial value starts the resource");

    wait_for_cycle(&rx, 0);
    source.tick_with_shared(&shared).unwrap();
    clock.advance(Duration::from_millis(10));

    let deadline = Instant::now() + StdDuration::from_secs(5);
    while handle.state() != ResourceState::Faulted {
        assert!(
            Instant::now() < deadline,
            "the destination did not latch the rejected transfer"
        );
        std::thread::sleep(StdDuration::from_millis(1));
    }
    handle.stop();
    handle.join().unwrap();
    assert_eq!(handle.state(), ResourceState::Faulted);
    assert_eq!(
        handle.last_error(),
        Some(RuntimeError::SubrangeViolation {
            value: 7,
            lower: 0,
            upper: 5,
        })
    );
    assert_eq!(shared.get("shared"), Some(Value::Int(7)));
}
