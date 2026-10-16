// Spec 11 section 6.9: pause and resume on a faulted or stopped resource, or on one whose
// scheduler no longer accepts commands, are refused with the stable code `resource_not_running`,
// name the resource state and change nothing. A healthy resource keeps today's replies and
// scheduler commands in both control modes.

const RESOURCE_STATE_SOURCE: &str = r#"
PROGRAM MainProgram
VAR
    Counter : INT;
END_VAR
Counter := Counter + 1;
END_PROGRAM

CONFIGURATION ResourceStateConfig
PROGRAM P1 : MainProgram;
END_CONFIGURATION
"#;

/// A control state whose resource reports `resource`. The command receiver is returned so a
/// test can see which scheduler commands were sent.
fn resource_state_fixture(
    mode: crate::config::ControlMode,
    resource: crate::scheduler::ResourceState,
) -> (
    ControlState,
    std::sync::mpsc::Receiver<crate::scheduler::ResourceCommand>,
) {
    let mut state = hmi_test_state(RESOURCE_STATE_SOURCE);
    *state.control_mode.lock().expect("control mode lock") = mode;
    let error = (resource == crate::scheduler::ResourceState::Faulted)
        .then_some(crate::error::RuntimeError::ResourceFaulted);
    let (control, cmd_rx) = crate::scheduler::ResourceControl::stub_with_state(
        crate::scheduler::StdClock::new(),
        resource,
        error,
    );
    state.resource = control;
    (state, cmd_rx)
}

#[test]
fn faulted_or_stopped_resource_refuses_pause_and_resume_and_changes_nothing() {
    use crate::config::ControlMode;
    use crate::debug::ControlAction;
    use crate::scheduler::ResourceState;
    for mode in [ControlMode::Production, ControlMode::Debug] {
        for (resource, reason) in [
            (
                ResourceState::Faulted,
                "resource is faulted; restart required",
            ),
            (ResourceState::Stopped, "resource is stopped"),
        ] {
            let label = format!("{mode:?} {resource:?}");
            let (state, cmd_rx) = resource_state_fixture(mode, resource);
            // A debugger hold that a refused resume must leave in place.
            let _ = state.debug.apply_action(ControlAction::Pause(None));

            for (id, kind) in [(1, "pause"), (2, "resume")] {
                let response = handle_request_value(json!({"id": id, "type": kind}), &state, None);
                assert!(!response.ok, "{label}: {kind} must be refused");
                assert_eq!(
                    response.error_code.as_deref(),
                    Some("resource_not_running"),
                    "{label}: {kind} carries the stable code"
                );
                assert_eq!(
                    response.error.as_deref(),
                    Some(reason),
                    "{label}: {kind} names the resource state"
                );
            }
            assert!(
                state.debug.is_paused(),
                "{label}: a refused resume releases no debugger hold"
            );
            assert!(
                cmd_rx.try_recv().is_err(),
                "{label}: no scheduler command is sent"
            );

            let status = handle_request_value(json!({"id": 3, "type": "status"}), &state, None);
            assert!(
                status.ok,
                "{label}: status should succeed: {:?}",
                status.error
            );
            let status = status.result.expect("status result");
            let expected = format!("{resource:?}").to_ascii_lowercase();
            assert_eq!(
                status.pointer("/state").and_then(serde_json::Value::as_str),
                Some(expected.as_str()),
                "{label}: status keeps the resource state"
            );
        }
    }
}

#[test]
fn scheduler_that_no_longer_accepts_commands_refuses_without_reporting_success() {
    use crate::config::ControlMode;
    use crate::debug::ControlAction;
    use crate::scheduler::ResourceState;
    // The resource reports running, but its command receiver is gone: the runner thread
    // has exited without updating the state.
    let (state, cmd_rx) = resource_state_fixture(ControlMode::Production, ResourceState::Running);
    drop(cmd_rx);

    let paused = handle_request_value(json!({"id": 1, "type": "pause"}), &state, None);
    assert!(
        !paused.ok,
        "a pause the scheduler cannot take must not report success"
    );
    assert_eq!(paused.error_code.as_deref(), Some("resource_not_running"));
    assert_eq!(
        paused.error.as_deref(),
        Some("resource is running and its scheduler does not accept commands")
    );

    let _ = state.debug.apply_action(ControlAction::Pause(None));
    let resumed = handle_request_value(json!({"id": 2, "type": "resume"}), &state, None);
    assert!(
        !resumed.ok,
        "a resume the scheduler cannot take must not report success"
    );
    assert_eq!(resumed.error_code.as_deref(), Some("resource_not_running"));
    assert!(
        state.debug.is_paused(),
        "a resume the scheduler cannot take releases no debugger hold"
    );
}

#[test]
fn healthy_resource_keeps_today_replies_and_scheduler_commands_in_both_modes() {
    use crate::config::ControlMode;
    use crate::scheduler::{ResourceCommand, ResourceState};
    for mode in [ControlMode::Production, ControlMode::Debug] {
        let label = format!("{mode:?}");
        let production = matches!(mode, ControlMode::Production);
        let (state, cmd_rx) = resource_state_fixture(mode, ResourceState::Running);

        let paused = handle_request_value(json!({"id": 1, "type": "pause"}), &state, None);
        assert!(
            paused.ok,
            "{label}: pause should succeed: {:?}",
            paused.error
        );
        assert_eq!(
            paused.result,
            Some(json!({"status": "paused"})),
            "{label}: pause reply"
        );
        assert_eq!(
            state.debug.is_paused(),
            !production,
            "{label}: only debug mode holds the scan on pause"
        );

        let resumed = handle_request_value(json!({"id": 2, "type": "resume"}), &state, None);
        assert!(
            resumed.ok,
            "{label}: resume should succeed: {:?}",
            resumed.error
        );
        assert_eq!(
            resumed.result,
            Some(json!({"status": "running"})),
            "{label}: resume reply"
        );
        assert!(!state.debug.is_paused(), "{label}: resume releases a hold");

        let sent: Vec<&str> = cmd_rx
            .try_iter()
            .map(|command| match command {
                ResourceCommand::Pause => "pause",
                ResourceCommand::Resume => "resume",
                _ => "other",
            })
            .collect();
        let expected: Vec<&str> = if production {
            vec!["pause", "resume"]
        } else {
            Vec::new()
        };
        assert_eq!(sent, expected, "{label}: scheduler commands");
    }
}

#[test]
fn restart_and_reload_follow_the_scan_thread_state() {
    // Spec 11 section 6.6: a faulted resource takes a restart request, because its scan thread is
    // alive in the faulted wait. A stopped resource has no thread, and a hung one has a scan thread
    // inside a call that did not return. Both refuse restart and bytecode reload with the stable
    // code. A refused reload sends no command, so it does not wait for a reply.
    use crate::config::ControlMode;
    use crate::scheduler::{ResourceControl, ResourceState, StdClock};
    use crate::RestartMode;

    let restart_request = json!({"id": 1, "type": "restart", "params": {"mode": "warm"}});
    let reload_request = json!({"id": 2, "type": "bytecode.reload", "params": {"bytes": "eA=="}});

    let (faulted, faulted_rx) =
        resource_state_fixture(ControlMode::Production, ResourceState::Faulted);
    let accepted = handle_request_value(restart_request.clone(), &faulted, None);
    assert!(
        accepted.ok,
        "faulted restart is accepted: {:?}",
        accepted.error
    );
    assert_eq!(
        faulted.pending_restart.lock().ok().and_then(|guard| *guard),
        Some(RestartMode::Warm)
    );
    drop(faulted_rx);

    let (stopped, stopped_rx) =
        resource_state_fixture(ControlMode::Production, ResourceState::Stopped);
    let refused = handle_request_value(restart_request.clone(), &stopped, None);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("resource_not_running"));
    assert_eq!(
        stopped.pending_restart.lock().ok().and_then(|guard| *guard),
        None,
        "a refused restart leaves no pending request"
    );
    let refused_reload = handle_request_value(reload_request.clone(), &stopped, None);
    assert!(!refused_reload.ok);
    assert_eq!(
        refused_reload.error_code.as_deref(),
        Some("resource_not_running")
    );
    assert!(
        stopped_rx.try_recv().is_err(),
        "a refused reload sends no scheduler command"
    );

    let (control, hung_rx) = ResourceControl::stub_with_state(
        StdClock::new(),
        ResourceState::Faulted,
        Some(crate::error::RuntimeError::WatchdogTimeout),
    );
    control.mark_hung_for_test();
    let (mut hung, _unused_rx) =
        resource_state_fixture(ControlMode::Production, ResourceState::Faulted);
    hung.resource = control;
    for request in [restart_request, reload_request] {
        let response = handle_request_value(request, &hung, None);
        assert!(!response.ok, "a hung resource refuses the request");
        assert_eq!(
            response.error_code.as_deref(),
            Some("resource_not_running"),
            "hung refusals carry the stable code"
        );
        assert!(
            response
                .error
                .as_deref()
                .is_some_and(|text| text.contains("did not return") && text.contains("restarted")),
            "hung refusals name the cause and the recovery: {:?}",
            response.error
        );
    }
    assert!(
        hung_rx.try_recv().is_err(),
        "a hung reload sends no scheduler command"
    );
    assert_eq!(
        hung.pending_restart.lock().ok().and_then(|guard| *guard),
        None,
        "a refused restart leaves no pending request"
    );
}
