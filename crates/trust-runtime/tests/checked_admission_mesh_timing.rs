//! Spec 34 §4.3.1 (existing mesh write timing) with spec 10 §4.7: a mesh update queued during a scan waits for the next command boundary.
#![cfg(feature = "full")]
use indexmap::IndexMap;
use smol_str::SmolStr;
use std::sync::mpsc;
use std::time::{Duration as StdDuration, Instant};
use trust_runtime::{
    error::RuntimeError,
    harness::CompileSession,
    io::IoDriver,
    scheduler::{ResourceCommand, ResourceRunner, StdClock},
    value::{Duration, Value},
};

const SOURCE: &str = "VAR_GLOBAL shared : INT := INT#0; observed : INT := INT#0; END_VAR \
    PROGRAM Main observed := shared; END_PROGRAM";

/// Blocks the first input read, so the first scan stays in flight until the test releases it.
struct GateDriver {
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
}

impl IoDriver for GateDriver {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), RuntimeError> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
            let _ = self.release.recv_timeout(StdDuration::from_secs(2));
        }
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}

/// Spec 34 §4.3.1 with spec 10 §4.7: a mesh update that arrives while a scan is in flight is not seen by that scan, is applied at the next command boundary, and is read by the following scan.
// The producer (the zenoh subscriber callback in crates/trust-runtime/src/host/mesh/startup_publish.rs:299-342)
// is not exercised; this drives the consumer `ResourceCommand::MeshApply` in process, as that callback does.
#[test]
fn mesh_update_queued_during_a_scan_is_applied_at_the_next_command_boundary() {
    let mut runtime = CompileSession::from_source(SOURCE).build_runtime().unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    runtime.add_io_driver(
        "gate",
        Box::new(GateDriver {
            entered: Some(entered_tx),
            release: release_rx,
        }),
    );
    let runner = ResourceRunner::new(runtime, StdClock::new(), Duration::from_millis(1));
    let mut handle = runner.spawn("mesh-command-boundary").expect("spawn");
    let control = handle.control();
    entered_rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("the first scan reaches its input read");

    let mut updates = IndexMap::new();
    updates.insert(SmolStr::new("shared"), Value::Int(7));
    control
        .send_command(ResourceCommand::MeshApply {
            updates,
            source: Some(SmolStr::new("peer")),
            sequence: Some(1),
        })
        .expect("queue the mesh update");
    let (snapshot_tx, snapshot_rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::Snapshot {
            respond_to: snapshot_tx,
        })
        .expect("queue the boundary snapshot");
    release_tx
        .send(())
        .expect("release the in-flight input read");

    // Commands are drained in order after the scan completes and before the next scan starts.
    let boundary = snapshot_rx
        .recv_timeout(StdDuration::from_secs(2))
        .expect("boundary snapshot");
    assert_eq!(
        boundary.storage.get_global("observed"),
        Some(&Value::Int(0)),
        "the in-flight scan must not see the mesh update"
    );
    assert_eq!(
        boundary.storage.get_global("shared"),
        Some(&Value::Int(7)),
        "the mesh update is applied before the next scan"
    );

    let deadline = Instant::now() + StdDuration::from_secs(2);
    loop {
        let (tx, rx) = mpsc::channel();
        control
            .send_command(ResourceCommand::Snapshot { respond_to: tx })
            .expect("queue a later snapshot");
        let later = rx
            .recv_timeout(StdDuration::from_secs(2))
            .expect("later snapshot");
        if later.storage.get_global("observed") == Some(&Value::Int(7)) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the next scan never read the applied mesh update"
        );
        std::thread::sleep(StdDuration::from_millis(1));
    }
    handle.stop();
    handle.join().expect("join resource thread");
}
