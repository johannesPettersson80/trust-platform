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
