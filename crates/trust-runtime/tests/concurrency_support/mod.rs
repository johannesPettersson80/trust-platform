//! Helpers for the real-thread concurrency tests (specification 11 sections 6.6, 6.8.1 and 6.9.2).
//! Every wait is bounded, so a broken interleaving fails the test instead of hanging it.

#![allow(dead_code)]

use std::sync::mpsc;
use std::thread;
use std::time::{Duration as WallDuration, Instant};

use trust_runtime::debug::DebugSnapshot;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, ResourceHandle, StdClock};
use trust_runtime::value::Value;

/// Upper bound for every wait. A healthy run answers in milliseconds.
pub const WAIT: WallDuration = WallDuration::from_secs(10);

/// Polls `condition` until it holds, and fails with `what` once `WAIT` has passed.
pub fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(WallDuration::from_millis(1));
    }
}

/// Requests a stop and joins the resource thread within `WAIT`. A thread that does not end fails
/// here instead of blocking the test.
pub fn stop_and_join(mut handle: ResourceHandle<StdClock>) {
    handle.stop();
    let (done_tx, done_rx) = mpsc::channel();
    let joiner = thread::spawn(move || {
        let result = handle.join();
        let _ = done_tx.send(());
        result
    });
    done_rx
        .recv_timeout(WAIT)
        .expect("the resource thread ends within the bound");
    joiner
        .join()
        .expect("joiner thread")
        .expect("the resource thread ends without panicking");
}

/// Reads the storage through the scan thread's own snapshot. The reply proves that the thread
/// still answers the command channel.
pub fn snapshot(control: &ResourceControl<StdClock>) -> DebugSnapshot {
    let (tx, rx) = mpsc::channel();
    control
        .send_command(ResourceCommand::Snapshot { respond_to: tx })
        .expect("command channel open");
    rx.recv_timeout(WAIT)
        .expect("snapshot reply within the bound")
}

/// A `DINT` global of a snapshot.
pub fn dint(snapshot: &DebugSnapshot, name: &str) -> i32 {
    match snapshot.storage.get_global(name) {
        Some(Value::DInt(value)) => *value,
        other => panic!("global {name} must be DINT, found {other:?}"),
    }
}

/// An `INT` global of a snapshot.
pub fn int(snapshot: &DebugSnapshot, name: &str) -> i16 {
    match snapshot.storage.get_global(name) {
        Some(Value::Int(value)) => *value,
        other => panic!("global {name} must be INT, found {other:?}"),
    }
}
