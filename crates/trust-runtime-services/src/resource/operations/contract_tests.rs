//! Pause, resume, restart and shutdown rules with today's exact replies, refusal texts and codes.

use alloc::string::String;
use alloc::vec;

use serde_json::json;

use super::*;
use crate::protocol::ControlAfterResponse;
use crate::resource::commands::CommandSendError;
use crate::tests::fake_host::{Action, FakeHost};

const HUNG: &str = "resource scan thread did not return; the runtime process must be restarted";

fn refusal(response: &ControlResponse) -> (Option<&str>, Option<&str>) {
    assert!(!response.ok, "expected a refusal: {response:?}");
    (response.error.as_deref(), response.error_code.as_deref())
}

#[test]
fn pause_and_resume_reply_and_act_by_control_mode() {
    let production = FakeHost::running();
    assert_eq!(
        pause(1, &production).result,
        Some(json!({"status": "paused"}))
    );
    assert_eq!(
        resume(2, &production).result,
        Some(json!({"status": "running"}))
    );
    assert_eq!(
        production.actions(),
        vec![
            Action::SchedulerPause,
            Action::SchedulerResume,
            Action::DebuggerContinue
        ],
        "production pauses the scheduler; resume also releases a debugger hold"
    );

    let mut debug = FakeHost::running();
    debug.debug_mode = true;
    let _ = pause(1, &debug);
    let _ = resume(2, &debug);
    assert_eq!(
        debug.actions(),
        vec![Action::DebuggerPause, Action::DebuggerContinue],
        "debug mode holds the scan through the debugger only"
    );
}

#[test]
fn faulted_or_stopped_resource_refuses_pause_and_resume_and_changes_nothing() {
    for debug_mode in [false, true] {
        for (state, reason) in [
            (
                ResourceState::Faulted,
                "resource is faulted; restart required",
            ),
            (ResourceState::Stopped, "resource is stopped"),
        ] {
            let mut host = FakeHost::running();
            host.debug_mode = debug_mode;
            host.state.set(state);
            for response in [pause(1, &host), resume(2, &host)] {
                assert_eq!(
                    refusal(&response),
                    (Some(reason), Some("resource_not_running"))
                );
            }
            assert!(host.actions().is_empty(), "{state:?}: nothing was sent");
            assert!(!accepts_pause_resume(state));
        }
    }
    for state in [
        ResourceState::Boot,
        ResourceState::Ready,
        ResourceState::Running,
        ResourceState::Paused,
    ] {
        assert!(accepts_pause_resume(state), "{state:?}");
    }
}

#[test]
fn a_full_queue_is_busy_and_a_closed_scheduler_names_the_state_read_after_it() {
    let mut busy = FakeHost::running();
    busy.command_refusal = Some((CommandSendError::Full, ResourceState::Running));
    for response in [pause(1, &busy), resume(2, &busy)] {
        assert_eq!(
            refusal(&response),
            (
                Some("resource command queue is full; the request was not queued"),
                Some("resource_busy")
            )
        );
    }

    let mut closed = FakeHost::running();
    closed.command_refusal = Some((CommandSendError::Closed, ResourceState::Running));
    assert_eq!(
        refusal(&pause(1, &closed)),
        (
            Some("resource is running and its scheduler does not accept commands"),
            Some("resource_not_running")
        )
    );
    closed.command_refusal = Some((CommandSendError::Closed, ResourceState::Ready));
    assert_eq!(
        refusal(&resume(2, &closed)),
        (
            Some("resource is ready and its scheduler does not accept commands"),
            Some("resource_not_running")
        )
    );
    assert!(
        closed.actions().is_empty(),
        "a refused resume releases no debugger hold"
    );
}

#[test]
fn a_hung_scan_refuses_every_resource_request() {
    let mut host = FakeHost::running();
    host.hung = true;
    host.state.set(ResourceState::Faulted);
    let warm = Some(json!({"mode": "warm"}));
    for response in [pause(1, &host), resume(2, &host), restart(3, warm, &host)] {
        assert_eq!(
            refusal(&response),
            (Some(HUNG), Some("resource_not_running"))
        );
    }
    assert_eq!(host.actions(), [] as [Action; 0]);
    assert_eq!(*host.pending_restart.borrow(), None);
}

#[test]
fn restart_checks_its_parameters_before_the_resource() {
    let mut host = FakeHost::running();
    host.hung = true;
    for (params, message) in [
        (None, String::from("missing params")),
        (
            Some(json!({"mode": "hot"})),
            String::from("invalid restart mode"),
        ),
        (
            Some(json!({})),
            String::from("invalid params: missing field `mode`"),
        ),
    ] {
        let response = restart(1, params, &host);
        assert_eq!(response.error.as_deref(), Some(message.as_str()));
        assert_eq!(response.error_code, None);
    }
}

#[test]
fn restart_is_queued_unless_stopped_and_cold_keeps_precedence() {
    let host = FakeHost::running();
    for (mode, pending) in [
        ("warm", RestartMode::Warm),
        ("COLD", RestartMode::Cold),
        ("warm", RestartMode::Cold),
    ] {
        let response = restart(1, Some(json!({"mode": mode})), &host);
        assert_eq!(response.result, Some(json!({"status": "restart queued"})));
        assert_eq!(*host.pending_restart.borrow(), Some(pending), "mode={mode}");
    }

    let faulted = FakeHost::running();
    faulted.state.set(ResourceState::Faulted);
    let response = restart(2, Some(json!({"mode": "warm"})), &faulted);
    assert!(response.ok, "the faulted wait takes a restart");
    assert_eq!(*faulted.pending_restart.borrow(), Some(RestartMode::Warm));

    let stopped = FakeHost::running();
    stopped.state.set(ResourceState::Stopped);
    let response = restart(3, Some(json!({"mode": "warm"})), &stopped);
    assert_eq!(
        refusal(&response),
        (Some("resource is stopped"), Some("resource_not_running"))
    );
    assert_eq!(*stopped.pending_restart.borrow(), None);

    let mut unreadable = FakeHost::running();
    unreadable.unreadable = vec!["restart_slot"];
    let response = restart(4, Some(json!({"mode": "cold"})), &unreadable);
    assert!(
        response.ok,
        "an unreadable request slot still answers as today"
    );
}

#[test]
fn a_device_without_an_operating_system_takes_a_restart_while_stopped() {
    // Spec 34 STATE-08: the program takes the place of the process, so `restart` from Stopped
    // starts the installed program again; trust-runtime on a computer refuses it (above).
    for (mode, pending) in [("cold", RestartMode::Cold), ("warm", RestartMode::Warm)] {
        let mut device = FakeHost::running();
        device.starts_when_stopped = true;
        device.state.set(ResourceState::Stopped);
        let response = restart(1, Some(json!({"mode": mode})), &device);
        assert_eq!(response.result, Some(json!({"status": "restart queued"})));
        assert_eq!(
            *device.pending_restart.borrow(),
            Some(pending),
            "mode={mode}"
        );
        assert_eq!(
            device.actions(),
            [] as [Action; 0],
            "no scheduler command is sent"
        );
    }

    let mut device = FakeHost::running();
    device.starts_when_stopped = true;
    device.state.set(ResourceState::Stopped);
    for response in [pause(2, &device), resume(3, &device)] {
        assert_eq!(
            refusal(&response),
            (Some("resource is stopped"), Some("resource_not_running")),
            "pause and resume answer as trust-runtime answers them while stopped"
        );
    }
}

#[test]
fn a_device_without_a_program_refuses_restart_with_the_reason_and_queues_nothing() {
    // Spec 34 STATE-08: a device whose application region holds no program, or one it refused,
    // stays Stopped and cannot start until a program is installed.
    for mode in ["cold", "warm"] {
        let mut device = FakeHost::running();
        device.starts_when_stopped = true;
        device.state.set(ResourceState::Stopped);
        device.missing_program = Some("the application region is empty");
        let response = restart(1, Some(json!({"mode": mode})), &device);
        assert_eq!(
            refusal(&response),
            (
                Some("no installed program: the application region is empty"),
                Some("resource_not_running")
            ),
            "mode={mode}"
        );
        assert_eq!(*device.pending_restart.borrow(), None, "mode={mode}");
        assert_eq!(device.actions(), [] as [Action; 0], "mode={mode}");
    }
}

#[test]
fn restarts_coalesce_with_cold_precedence() {
    for (pending, requested, next) in [
        (None, RestartMode::Warm, RestartMode::Warm),
        (None, RestartMode::Cold, RestartMode::Cold),
        (
            Some(RestartMode::Warm),
            RestartMode::Cold,
            RestartMode::Cold,
        ),
        (
            Some(RestartMode::Warm),
            RestartMode::Warm,
            RestartMode::Warm,
        ),
        (
            Some(RestartMode::Cold),
            RestartMode::Warm,
            RestartMode::Cold,
        ),
        (
            Some(RestartMode::Cold),
            RestartMode::Cold,
            RestartMode::Cold,
        ),
    ] {
        assert_eq!(coalesce_restart(pending, requested), next);
    }
}

#[test]
fn shutdown_replies_stopping_and_defers_the_stop() {
    let response = shutdown(41, None);
    assert_eq!(response.result, Some(json!({"status": "stopping"})));
    assert_eq!(
        response.after_response,
        Some(ControlAfterResponse::StopResource)
    );
}

#[test]
fn shutdown_without_exit_or_with_exit_false_only_stops_the_program() {
    // Specification 11 §6.9 and 34 STATE-08: a plain `shutdown` leaves the process running in
    // the Stopped state, and `{"exit": false}` is a plain `shutdown`. Wrong behaviour: an empty
    // object or `exit: false` ends the process.
    for params in [None, Some(json!({})), Some(json!({"exit": false}))] {
        let response = shutdown(8, params.as_ref());
        assert_eq!(
            response.result,
            Some(json!({"status": "stopping"})),
            "{params:?}"
        );
        assert_eq!(
            response.after_response,
            Some(ControlAfterResponse::StopResource),
            "{params:?}: the process stays"
        );
    }
}

#[test]
fn shutdown_with_exit_true_replies_the_same_and_defers_the_exit_of_the_process() {
    // Specification 11 §6.9 and 34 STATE-08: `shutdown` with `{"exit": true}` is the explicit
    // exit. Its reply is the `stopping` of `shutdown`, written first, and the platform ends the
    // process after it. Wrong behaviour: the exit is ignored and the process stays, or the reply
    // differs from the one a client of `shutdown` reads.
    let response = shutdown(7, Some(&json!({"exit": true})));
    assert_eq!(response.result, Some(json!({"status": "stopping"})));
    assert_eq!(
        response.after_response,
        Some(ControlAfterResponse::ExitRuntime)
    );
}

#[test]
fn shutdown_refuses_a_malformed_parameter_and_stops_nothing() {
    // Specification 11 §6.9: `exit` must be a Boolean and no other member is accepted, so a typo
    // never turns into a different action. Wrong behaviour: `"true"` or `1` reads as plain
    // `shutdown` (the process stays when the caller asked for an exit), or an unknown member is
    // ignored.
    for (params, error) in [
        (
            json!({"exit": "true"}),
            "invalid params: exit must be a boolean",
        ),
        (json!({"exit": 1}), "invalid params: exit must be a boolean"),
        (
            json!({"exit": null}),
            "invalid params: parameter fields must not be null",
        ),
        (
            json!({"exits": true}),
            "invalid params: unknown parameter field",
        ),
        (
            json!("exit"),
            "invalid params: parameters must be an object",
        ),
        (
            json!([true]),
            "invalid params: parameters must be an object",
        ),
    ] {
        let response = shutdown(9, Some(&params));
        assert_eq!(refusal(&response), (Some(error), None), "{params}");
        assert_eq!(response.after_response, None, "{params}: nothing stops");
    }
}
