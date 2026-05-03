use super::*;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

fn recv_stop(stop_rx: &Receiver<DebugStop>, timeout: Duration, label: &str) -> DebugStop {
    stop_rx
        .recv_timeout(timeout)
        .unwrap_or_else(|err| panic!("{label}: {err:?}"))
}

#[test]
fn breakpoint_clears_pending_pause() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 5);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    {
        let (lock, _) = &*control.state;
        let mut state = lock.lock().expect("debug state poisoned");
        state.pending_stop = Some(DebugStopReason::Pause);
        state.mode = DebugMode::Running;
    }

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&location), 0);
    });

    let stop = recv_stop(&stop_rx, Duration::from_secs(2), "breakpoint stop");
    assert_eq!(stop.reason, DebugStopReason::Breakpoint);

    control.continue_run();
    handle.join().unwrap();

    let (lock, _) = &*control.state;
    let state = lock.lock().expect("debug state poisoned");
    assert!(state.pending_stop.is_none());
}

#[test]
fn pause_after_continue_while_waiting_emits_pause_stop() {
    let control = DebugControl::new();
    let location = SourceLocation::new(0, 0, 5);
    control.set_breakpoints_for_file(0, vec![DebugBreakpoint::new(location)]);

    let (stop_tx, stop_rx) = channel();
    control.set_stop_sender(stop_tx);

    let mut hook = control.clone();
    let handle = thread::spawn(move || {
        hook.on_statement(Some(&location), 0);
    });

    let first = recv_stop(&stop_rx, Duration::from_secs(2), "breakpoint stop");
    assert_eq!(first.reason, DebugStopReason::Breakpoint);

    // Race window: continue and immediately pause again while the first hook may still be waking.
    control.continue_run();
    control.pause();

    // Always drive a fresh statement boundary after pause so a pending pause stop is consumed
    // deterministically even if the first hook already exited after continue.
    // Use a different location so this fallback path cannot hit the original breakpoint.
    let second_location = SourceLocation::new(1, 0, 5);
    let mut second_hook = control.clone();
    let second_handle = thread::spawn(move || {
        second_hook.on_statement(Some(&second_location), 0);
    });

    let second = recv_stop(&stop_rx, Duration::from_secs(2), "pause stop");
    assert_eq!(second.reason, DebugStopReason::Pause);

    control.continue_run();
    handle.join().expect("hook thread joins");
    second_handle.join().expect("second hook thread joins");
}
