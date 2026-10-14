// Issue #136 and spec 11 section 6.9: step_in, step_over and step_out act only while the
// debugger holds the scan, in either control mode, and arm nothing otherwise. Resume
// releases a debugger hold so the scan cycles again, and status reports a held scan as
// paused.

const HELD_SCAN_SOURCE: &str = r#"
PROGRAM MainProgram
VAR
    Counter : INT;
END_VAR
Counter := Counter + 1;
Counter := Counter + 1;
END_PROGRAM

CONFIGURATION HeldScanConfig
PROGRAM P1 : MainProgram;
END_CONFIGURATION
"#;

const STEP_REQUIRES_HOLD_REFUSAL: &str =
    "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)";

fn held_scan_state(mode: crate::config::ControlMode) -> ControlState {
    let state = hmi_test_state(HELD_SCAN_SOURCE);
    *state.control_mode.lock().expect("control mode lock") = mode;
    state
}

fn held_scan_runtime(state: &ControlState) -> crate::Runtime {
    let mut runtime = crate::harness::CompileSession::from_source(HELD_SCAN_SOURCE)
        .build_runtime()
        .expect("held scan fixture compiles");
    runtime.set_debug_control(state.debug.clone());
    runtime
}

fn held_scan_counter(runtime: &crate::Runtime) -> crate::value::Value {
    let Some(crate::value::Value::Instance(program)) = runtime.storage().get_global("P1") else {
        panic!("program instance P1");
    };
    runtime
        .storage()
        .get_instance_var(*program, "Counter")
        .cloned()
        .expect("Counter")
}

fn held_scan_status(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(json!({"id": 90, "type": "status"}), state, None);
    assert!(response.ok, "status should succeed: {:?}", response.error);
    response.result.expect("status result")
}

/// Run one scan on a worker thread, so a debugger hold that the scan waits on fails
/// the caller instead of wedging it. A timed-out hold is released and the scan joined.
/// Returns the runtime and whether the scan had to be released.
fn held_scan_run_once(
    runtime: crate::Runtime,
    debug: &crate::debug::DebugControl,
) -> (crate::Runtime, bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut runtime = runtime;
        let result = runtime.execute_cycle();
        let _ = tx.send((runtime, result));
    });
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok((runtime, result)) => {
            result.expect("the scan completes");
            (runtime, false)
        }
        Err(_) => {
            let _ = debug.apply_action(crate::debug::ControlAction::Continue);
            let (runtime, _) = rx.recv().expect("the released scan returns its runtime");
            (runtime, true)
        }
    }
}

#[test]
fn production_step_requests_are_refused_and_resume_restarts_the_scan() {
    use crate::config::ControlMode;
    use crate::value::Value;
    let state = held_scan_state(ControlMode::Production);
    let runtime = held_scan_runtime(&state);

    let paused = handle_request_value(json!({"id": 1, "type": "pause"}), &state, None);
    assert!(paused.ok, "pause should succeed: {:?}", paused.error);
    for (id, kind) in [(2, "step_in"), (3, "step_over"), (4, "step_out")] {
        let refused = handle_request_value(json!({"id": id, "type": kind}), &state, None);
        assert!(
            !refused.ok,
            "{kind} after a production pause must be refused"
        );
        assert_eq!(
            refused.error.as_deref(),
            Some(STEP_REQUIRES_HOLD_REFUSAL),
            "{kind} is refused because no debugger holds the scan"
        );
    }
    assert!(
        matches!(state.debug.mode(), crate::debug::DebugMode::Running),
        "a refused step arms no debugger step"
    );

    let resumed = handle_request_value(json!({"id": 5, "type": "resume"}), &state, None);
    assert!(resumed.ok, "resume should succeed: {:?}", resumed.error);
    let (runtime, timed_out) = held_scan_run_once(runtime, &state.debug);
    assert!(
        !timed_out,
        "the scan must not wait for a debugger step that was refused"
    );
    assert_eq!(
        runtime.cycle_counter(),
        1,
        "resume restarts cyclic execution"
    );
    assert_eq!(held_scan_counter(&runtime), Value::Int(2));
}

#[test]
fn debug_hold_is_reported_by_status_and_released_by_resume_in_both_modes() {
    use crate::config::ControlMode;
    use crate::debug::ControlAction;
    use crate::value::Value;
    for mode in [ControlMode::Debug, ControlMode::Production] {
        let label = format!("{mode:?}");
        let state = held_scan_state(mode);
        let runtime = held_scan_runtime(&state);
        let _ = state.debug.apply_action(ControlAction::Pause(None));

        let held = held_scan_status(&state);
        assert_eq!(
            held.pointer("/state").and_then(serde_json::Value::as_str),
            Some("paused"),
            "{label}: status reports the hold instead of running"
        );
        assert_eq!(
            held.pointer("/debug_hold/reason")
                .and_then(serde_json::Value::as_str),
            Some("pause"),
            "{label}: status names the hold reason"
        );

        let resumed = handle_request_value(json!({"id": 6, "type": "resume"}), &state, None);
        assert!(
            resumed.ok,
            "{label}: resume should succeed: {:?}",
            resumed.error
        );
        assert!(
            !state.debug.is_paused(),
            "{label}: resume releases the debugger hold"
        );

        let released = held_scan_status(&state);
        assert!(
            released
                .pointer("/debug_hold")
                .is_some_and(serde_json::Value::is_null),
            "{label}: no hold is reported after resume"
        );
        assert_ne!(
            released
                .pointer("/state")
                .and_then(serde_json::Value::as_str),
            Some("paused"),
            "{label}: a released scan is not paused"
        );

        let (runtime, timed_out) = held_scan_run_once(runtime, &state.debug);
        assert!(!timed_out, "{label}: the released scan must cycle");
        assert_eq!(held_scan_counter(&runtime), Value::Int(2), "{label}");
    }
}

#[test]
fn debug_mode_steps_are_refused_without_a_hold_and_accepted_after_a_pause() {
    use crate::config::ControlMode;
    use crate::debug::DebugMode;
    let state = held_scan_state(ControlMode::Debug);

    // Without a hold, every step is refused and arms nothing.
    for (id, kind) in [(1, "step_in"), (2, "step_over"), (3, "step_out")] {
        let refused = handle_request_value(json!({"id": id, "type": kind}), &state, None);
        assert!(
            !refused.ok,
            "{kind} without a hold must be refused in debug mode"
        );
        assert_eq!(
            refused.error.as_deref(),
            Some(STEP_REQUIRES_HOLD_REFUSAL),
            "{kind} without a hold"
        );
    }
    assert!(
        matches!(state.debug.mode(), DebugMode::Running),
        "refused steps arm nothing"
    );

    // A debug-mode pause holds the scan, so each step is accepted. An accepted step
    // releases that hold, which is why every step starts from a fresh pause.
    for (id, kind) in [(4, "step_in"), (5, "step_over"), (6, "step_out")] {
        let paused = handle_request_value(json!({"id": id + 100, "type": "pause"}), &state, None);
        assert!(
            paused.ok,
            "debug-mode pause should succeed: {:?}",
            paused.error
        );
        let accepted = handle_request_value(json!({"id": id, "type": kind}), &state, None);
        assert!(
            accepted.ok,
            "{kind} after a debug-mode pause: {:?}",
            accepted.error
        );
        assert_eq!(
            accepted
                .result
                .as_ref()
                .and_then(|result| result.get("status"))
                .and_then(serde_json::Value::as_str),
            Some("stepping"),
            "{kind} after a debug-mode pause"
        );
    }
}

#[test]
fn production_mode_step_is_accepted_while_a_debugger_holds_the_scan() {
    use crate::config::ControlMode;
    use crate::debug::{ControlAction, DebugMode};
    let state = held_scan_state(ControlMode::Production);

    // An attached debugger holds the scan through the debug control, as it does at a
    // breakpoint. Each step is accepted while that hold exists.
    for (id, kind) in [(1, "step_in"), (2, "step_over"), (3, "step_out")] {
        let _ = state.debug.apply_action(ControlAction::Pause(None));
        let accepted = handle_request_value(json!({"id": id, "type": kind}), &state, None);
        assert!(
            accepted.ok,
            "{kind} while the debugger holds the scan in production: {:?}",
            accepted.error
        );
        assert_eq!(
            accepted
                .result
                .as_ref()
                .and_then(|result| result.get("status"))
                .and_then(serde_json::Value::as_str),
            Some("stepping"),
            "{kind} while the debugger holds the scan in production"
        );
        assert!(
            matches!(state.debug.mode(), DebugMode::Running),
            "the accepted {kind} releases the hold until the debugger holds the scan again"
        );
    }
}
