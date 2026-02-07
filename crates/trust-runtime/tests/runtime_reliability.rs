use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::{FileRetainStore, RetainStore};
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::{Duration, Value};
use trust_runtime::watchdog::{WatchdogAction, WatchdogPolicy};
use trust_runtime::RestartMode;

#[derive(Clone, Debug)]
struct StepClock {
    inner: Arc<Mutex<Duration>>,
    step: Duration,
}

impl StepClock {
    fn new(step: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Duration::ZERO)),
            step,
        }
    }

    fn set(&self, time: Duration) {
        let mut guard = self.inner.lock().expect("step clock lock poisoned");
        *guard = time;
    }
}

impl Clock for StepClock {
    fn now(&self) -> Duration {
        let mut guard = self.inner.lock().expect("step clock lock poisoned");
        let now = *guard;
        let next = now.as_nanos().saturating_add(self.step.as_nanos());
        *guard = Duration::from_nanos(next);
        now
    }

    fn sleep_until(&self, deadline: Duration) {
        let mut guard = self.inner.lock().expect("step clock lock poisoned");
        *guard = deadline;
    }

    fn wake(&self) {}
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    let pid = std::process::id();
    path.push(format!("trust_runtime_reliability_{pid}_{name}.bin"));
    path
}

#[test]
fn e2e_startup_io_restart() {
    let source = r#"
PROGRAM Main
VAR
    input : BOOL := FALSE;
    output : BOOL := FALSE;
END_VAR
output := input;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.bind_direct("input", "%IX0.0").unwrap();
    harness.bind_direct("output", "%QX0.0").unwrap();
    harness
        .set_direct_input("%IX0.0", Value::Bool(true))
        .unwrap();
    harness.cycle();
    let out = harness.get_direct_output("%QX0.0").unwrap();
    assert_eq!(out, Value::Bool(true));

    harness.advance_time(Duration::from_millis(10));
    harness.restart(RestartMode::Warm).unwrap();
    assert_eq!(harness.current_time(), Duration::ZERO);
}

#[test]
fn e2e_retain_roundtrip_restart() {
    let source = r#"
PROGRAM Main
VAR RETAIN
    r : INT := 1;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let path = temp_path("retain_roundtrip");
    let store = FileRetainStore::new(&path);
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(store)), Some(Duration::from_millis(0)));
    harness.set_input("r", Value::Int(42));
    harness.runtime_mut().mark_retain_dirty();
    harness.runtime_mut().save_retain_store().unwrap();

    harness.restart_with_retain(RestartMode::Warm).unwrap();
    assert_eq!(harness.get_output("r"), Some(Value::Int(42)));

    let _ = std::fs::remove_file(path);
}

#[test]
fn retain_power_loss_does_not_persist_unsaved() {
    let source = r#"
PROGRAM Main
VAR RETAIN
    r : INT := 0;
END_VAR
END_PROGRAM
"#;

    let path = temp_path("retain_power_loss");
    let _ = std::fs::remove_file(&path);
    let mut harness = TestHarness::from_source(source).unwrap();
    let store = FileRetainStore::new(&path);
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(store)), None);
    harness.set_input("r", Value::Int(77));
    harness.runtime_mut().mark_retain_dirty();
    drop(harness);

    let store = FileRetainStore::new(&path);
    let snapshot = store.load().expect("load retain snapshot");
    assert!(snapshot.values().is_empty());
}

#[test]
fn watchdog_faults_resource_on_overrun() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT := 0;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

    let runtime = TestHarness::from_source(source).unwrap().into_runtime();
    let clock = StepClock::new(Duration::from_millis(10));
    clock.set(Duration::from_millis(0));

    let mut runner = ResourceRunner::new(runtime, clock.clone(), Duration::from_millis(1));
    runner.runtime_mut().set_watchdog_policy(WatchdogPolicy {
        enabled: true,
        // Watchdog is wall-clock based, so use an effectively-zero timeout.
        timeout: Duration::from_nanos(1),
        action: WatchdogAction::Halt,
    });

    let mut handle = runner.spawn("watchdog-test").unwrap();
    let start = Instant::now();
    loop {
        if handle.state() == ResourceState::Faulted {
            break;
        }
        if start.elapsed() >= StdDuration::from_millis(100) {
            panic!(
                "resource did not fault in time (state {:?})",
                handle.state()
            );
        }
        std::thread::yield_now();
    }
    assert!(matches!(
        handle.last_error(),
        Some(RuntimeError::WatchdogTimeout)
    ));
    handle.join().unwrap();
}
