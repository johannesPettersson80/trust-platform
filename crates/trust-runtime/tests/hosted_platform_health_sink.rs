//! Spec 34 §4.3 HOST-05 (line 353): a stalled reader of the control thread's
//! health sink cannot hold a scan beyond a bounded wait.
#![cfg(feature = "full")]

use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration as WallDuration,
};
use trust_runtime::harness::CompileSession;
use trust_runtime::value::Duration;

/// Spec 34 §4.3 HOST-05 (line 353): a control cycle completes while a status reader holds the health sink. EXPECTED-FAIL: `update_health` takes a blocking `sink.lock()` (runtime/io_subsystem.rs:198).
#[test]
fn stalled_health_reader_cannot_hold_the_control_cycle() {
    let mut runtime = CompileSession::from_source("PROGRAM Main END_PROGRAM")
        .build_runtime()
        .expect("source admits");
    let sink = Arc::new(Mutex::new(Vec::new()));
    runtime.set_io_health_sink(Some(Arc::clone(&sink)));

    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let reader = {
        let sink = Arc::clone(&sink);
        thread::spawn(move || {
            let guard = sink.lock().expect("health sink");
            locked_tx.send(()).expect("report the held lock");
            release_rx.recv().expect("release signal");
            drop(guard);
        })
    };
    locked_rx.recv().expect("reader holds the health sink");

    let (done_tx, done_rx) = mpsc::channel();
    let control = thread::spawn(move || {
        runtime.set_current_time(Duration::from_millis(10));
        let result = runtime.execute_cycle();
        let _ = done_tx.send(());
        result
    });
    let finished_while_held = done_rx.recv_timeout(WallDuration::from_secs(1)).is_ok();

    release_tx.send(()).expect("release the reader");
    reader.join().expect("reader thread");
    let result = control.join().expect("control thread");

    assert!(
        finished_while_held,
        "the control cycle waited on a stalled health reader for more than 1 s (spec 34 §4.3 HOST-05)"
    );
    assert_eq!(result, Ok(()), "the cycle must complete without a fault");
}
