use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::RetainStore;
use trust_runtime::scheduler::{Clock, ResourceRunner, ResourceState};
use trust_runtime::value::Duration;
use trust_runtime::RetainSnapshot;

#[derive(Clone, Debug)]
struct StepClock {
    inner: Arc<Mutex<Duration>>,
}

impl StepClock {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }
}

impl Clock for StepClock {
    fn now(&self) -> Duration {
        *self.inner.lock().expect("step clock lock")
    }

    fn sleep_until(&self, deadline: Duration) {
        *self.inner.lock().expect("step clock lock") = deadline;
    }

    fn wake(&self) {}
}

#[derive(Debug)]
struct FailingRetainStore;

impl RetainStore for FailingRetainStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(RetainSnapshot::default())
    }

    fn store(&self, _snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        Err(RuntimeError::RetainStore("retain save failed".into()))
    }
}

/// Spec 11 §6.7: a requested stop whose retain save fails is exposed as the resource's retain error and faults the resource; it is not reported as a successful stop.
#[test]
fn requested_stop_with_failed_retain_save_faults_instead_of_stopping() {
    let mut runtime = TestHarness::from_source(
        "VAR_GLOBAL RETAIN kept : INT := INT#1; END_VAR PROGRAM Main kept := kept + INT#1; END_PROGRAM",
    )
    .expect("compile retain fixture")
    .into_runtime();
    runtime.set_retain_store(Some(Box::new(FailingRetainStore)), None);

    let runner = ResourceRunner::new(runtime, StepClock::new(), Duration::from_millis(1));
    let mut handle = runner
        .spawn("requested-stop-retain-save-failure")
        .expect("spawn runner");
    handle.stop();
    handle.join().expect("runner join");

    assert_eq!(handle.state(), ResourceState::Faulted);
    assert!(
        matches!(handle.last_error(), Some(RuntimeError::RetainStore(_))),
        "a failed stop checkpoint must surface as RetainStore"
    );
}
