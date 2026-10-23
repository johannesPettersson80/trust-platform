use std::sync::mpsc::Receiver;
use std::time::Duration as StdDuration;

use trust_runtime::debug::RuntimeEvent;
use trust_runtime::harness::TestHarness;
use trust_runtime::scheduler::{ManualClock, ResourceRunner, SharedGlobals};
use trust_runtime::value::{Duration, Value};

fn wait_for_cycle(rx: &Receiver<RuntimeEvent>, target: u64) {
    let start = std::time::Instant::now();
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

#[test]
fn multiple_resources() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let mut runtime_a = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_a = runtime_a.enable_debug();
    let (tx_a, rx_a) = std::sync::mpsc::channel();
    debug_a.set_runtime_sender(tx_a);

    let mut runtime_b = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_b = runtime_b.enable_debug();
    let (tx_b, rx_b) = std::sync::mpsc::channel();
    debug_b.set_runtime_sender(tx_b);

    let clock_a = ManualClock::new();
    let clock_b = ManualClock::new();

    let runner_a = ResourceRunner::new(runtime_a, clock_a.clone(), Duration::from_millis(10));
    let runner_b = ResourceRunner::new(runtime_b, clock_b.clone(), Duration::from_millis(10));

    let mut handle_a = runner_a.spawn("res-a").unwrap();
    let mut handle_b = runner_b.spawn("res-b").unwrap();

    assert_ne!(handle_a.thread_id(), handle_b.thread_id());
    assert_ne!(handle_a.thread_id(), std::thread::current().id());
    assert_ne!(handle_b.thread_id(), std::thread::current().id());

    wait_for_cycle(&rx_a, 0);
    wait_for_cycle(&rx_b, 0);

    handle_a.stop();
    handle_b.stop();
    handle_a.join().unwrap();
    handle_b.join().unwrap();
}

#[test]
fn global_sync() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL
    shared : INT := 0;
END_VAR
TASK T (INTERVAL := T#100ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_CONFIGURATION

PROGRAM Main
shared := shared + 1;
END_PROGRAM
"#;

    let mut runtime_a = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_a = runtime_a.enable_debug();
    let (tx_a, rx_a) = std::sync::mpsc::channel();
    debug_a.set_runtime_sender(tx_a);

    let mut runtime_b = TestHarness::from_source(source).unwrap().into_runtime();
    let debug_b = runtime_b.enable_debug();
    let (tx_b, rx_b) = std::sync::mpsc::channel();
    debug_b.set_runtime_sender(tx_b);

    let shared = SharedGlobals::from_runtime(vec!["shared".into()], &runtime_a).unwrap();

    let clock_a = ManualClock::new();
    let clock_b = ManualClock::new();

    let runner_a = ResourceRunner::new(runtime_a, clock_a.clone(), Duration::from_millis(100));
    let runner_b = ResourceRunner::new(runtime_b, clock_b.clone(), Duration::from_millis(100));

    let mut handle_a = runner_a.spawn_with_shared("res-a", shared.clone()).unwrap();
    let mut handle_b = runner_b.spawn_with_shared("res-b", shared.clone()).unwrap();

    wait_for_cycle(&rx_a, 0);
    wait_for_cycle(&rx_b, 0);

    clock_a.advance(Duration::from_millis(100));
    clock_b.advance(Duration::from_millis(100));

    wait_for_cycle(&rx_a, 1);
    wait_for_cycle(&rx_b, 1);

    handle_a.stop();
    handle_b.stop();
    handle_a.join().unwrap();
    handle_b.join().unwrap();

    match shared.get("shared") {
        Some(Value::Int(value)) => assert_eq!(i64::from(value), 2),
        Some(Value::DInt(value)) => assert_eq!(i64::from(value), 2),
        Some(Value::LInt(value)) => assert_eq!(value, 2),
        other => panic!("unexpected shared value {other:?}"),
    }
}

#[test]
fn shared_globals_reject_incompatible_destination_before_starting_a_thread() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT := 1; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main END_PROGRAM
"#;
    let source_runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let shared = SharedGlobals::from_runtime(vec!["shared".into()], &source_runtime).unwrap();
    let destination = TestHarness::from_source(&source.replace("INT := 1", "BOOL := FALSE"))
        .unwrap()
        .into_runtime();
    let starts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&starts);
    let runner = ResourceRunner::new(destination, ManualClock::new(), Duration::from_millis(10))
        .with_thread_init_hook(std::sync::Arc::new(move || {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }));
    let result = runner.spawn_with_shared("incompatible-shared", shared);
    assert!(matches!(
        result,
        Err(trust_runtime::error::RuntimeError::TypeMismatch)
    ));
    assert_eq!(starts.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn shared_globals_reject_read_only_destination_before_run() {
    let source = r#"
CONFIGURATION C
VAR_GLOBAL shared : INT := 1; END_VAR
PROGRAM P : Main;
END_CONFIGURATION
PROGRAM Main END_PROGRAM
"#;
    let source_runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let shared = SharedGlobals::from_runtime(vec!["shared".into()], &source_runtime).unwrap();
    let destination =
        TestHarness::from_source(&source.replace("VAR_GLOBAL", "VAR_GLOBAL CONSTANT"))
            .unwrap()
            .into_runtime();
    let result = ResourceRunner::new(destination, ManualClock::new(), Duration::from_millis(10))
        .spawn_with_shared("read-only-shared", shared);
    assert!(matches!(
        result,
        Err(trust_runtime::error::RuntimeError::ConstantWrite)
    ));
}
