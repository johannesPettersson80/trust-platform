//! The Windows console stop (spec 11, sections 6.8 and 6.8.0a): Ctrl+C sets a flag, and the
//! flagged request reaches the ordinary runtime stop, which removes the endpoint file. The flag
//! logic does not depend on the platform, so it is tested everywhere.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::RecordingShutdownTarget;

#[test]
fn a_flagged_console_stop_requests_the_ordinary_runtime_stop() {
    let requested = Arc::new(AtomicBool::new(false));
    let target = Arc::new(RecordingShutdownTarget::default());
    let waiter = {
        let requested = Arc::clone(&requested);
        let target = Arc::clone(&target);
        std::thread::spawn(move || {
            super::super::request_shutdown_when_flagged(
                &requested,
                target.as_ref(),
                Duration::from_millis(5),
            );
        })
    };

    std::thread::sleep(Duration::from_millis(30));
    assert!(
        !target.called.load(Ordering::SeqCst),
        "nothing stops before the console asks"
    );

    requested.store(true, Ordering::SeqCst);
    waiter.join().expect("join the console stop waiter");

    assert!(
        target.called.load(Ordering::SeqCst),
        "the console stop uses the ordinary stop target"
    );
}
