//! Spec 11 retain fail-closed policy at the installed-runtime level: a due periodic save
//! failure fails that scan before physical outputs are committed, does not advance the save
//! cadence, and the next scan retries the save. The dirty flag itself is not observable here.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::io::IoDriver;
use trust_runtime::retain::RetainStore;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RetainSnapshot;

const SOURCE: &str = r#"
VAR_GLOBAL RETAIN kept : INT := INT#1; END_VAR
VAR_GLOBAL lamp AT %QX0.0 : BOOL; END_VAR
PROGRAM Main
kept := kept + INT#1;
lamp := TRUE;
END_PROGRAM
"#;

/// Fails the first `failures` store calls, then publishes the stored `kept` value.
struct FlakyStore {
    calls: Arc<AtomicUsize>,
    failures: usize,
    stored_kept: Arc<Mutex<Option<Value>>>,
}

impl RetainStore for FlakyStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(RetainSnapshot::default())
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call <= self.failures {
            return Err(RuntimeError::RetainStore(
                "injected periodic save failure".into(),
            ));
        }
        *self.stored_kept.lock().expect("stored value lock") =
            snapshot.values().get("kept").cloned();
        Ok(())
    }
}

/// Records every output image the runtime commits to a physical driver.
struct RecordingDriver {
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl IoDriver for RecordingDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, outputs: &[u8]) -> Result<(), RuntimeError> {
        self.writes
            .lock()
            .expect("driver writes lock")
            .push(outputs.to_vec());
        Ok(())
    }
}

/// Spec 11, "Retain-store failures use the following fail-closed transaction policy": "A due
/// periodic save failure fails that scan before physical outputs are committed" and "The
/// in-memory snapshot remains dirty so a later save may retry". The cadence restarts from a
/// successful save (spec 11 retain cadence: "rate-limited").
#[test]
fn failed_due_periodic_save_blocks_the_scan_outputs_and_the_next_scan_retries() {
    let mut runtime = CompileSession::from_source(SOURCE)
        .build_runtime()
        .expect("compile retain periodic fixture");
    let writes = Arc::new(Mutex::new(Vec::new()));
    runtime.add_io_driver(
        "recorder",
        Box::new(RecordingDriver {
            writes: writes.clone(),
        }),
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let stored_kept = Arc::new(Mutex::new(None));
    runtime.set_retain_store(
        Some(Box::new(FlakyStore {
            calls: calls.clone(),
            failures: 1,
            stored_kept: stored_kept.clone(),
        })),
        Some(Duration::from_millis(10)),
    );

    // The store was configured at t = 0 with a 10 ms interval, so the scan at t = 10 ms is due.
    runtime.advance_time(Duration::from_millis(10));
    let failed = runtime.execute_cycle();
    assert!(
        matches!(failed, Err(RuntimeError::RetainStore(_))),
        "due save failure must fail the scan with RetainStore, got {failed:?}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "one save attempt in the failed scan"
    );
    assert!(
        writes.lock().expect("driver writes lock").is_empty(),
        "the failed scan must not commit physical outputs"
    );
    assert!(
        runtime.faulted(),
        "the failed save is latched as the resource fault"
    );
    assert!(
        matches!(runtime.last_fault(), Some(RuntimeError::RetainStore(_))),
        "latched fault must be the RetainStore error, got {:?}",
        runtime.last_fault()
    );
    assert_eq!(
        *stored_kept.lock().expect("stored value lock"),
        None,
        "a failed save must not publish a snapshot"
    );

    // The operator clears the fault. The failed save did not move the last save time, so a
    // scan at the same t = 10 ms is still due and retries the save.
    runtime.clear_fault();
    runtime
        .execute_cycle()
        .expect("retried periodic save must succeed");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the next scan retries the failed save"
    );
    assert_eq!(
        *stored_kept.lock().expect("stored value lock"),
        Some(Value::Int(3)),
        "the retried save publishes the in-memory value of the scan that saved"
    );
    assert_eq!(runtime.storage().get_global("kept"), Some(&Value::Int(3)));
    let committed = writes.lock().expect("driver writes lock");
    assert_eq!(committed.len(), 1, "only the retried scan commits outputs");
    assert_eq!(
        committed[0].first().copied(),
        Some(1),
        "the retried scan commits the program's TRUE output"
    );
    drop(committed);

    // The successful save restarts the cadence: 5 ms later the scan is not due.
    runtime.advance_time(Duration::from_millis(5));
    runtime
        .execute_cycle()
        .expect("scan inside the save cadence");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "no save is attempted inside the interval after a successful save"
    );
}
