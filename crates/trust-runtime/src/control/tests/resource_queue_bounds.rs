// The bounded resource command queue (specification 11 section 6.9). A full queue refuses the next
// request with the stable code `resource_busy` and queues nothing. Once the scan thread drains the
// queue, requests are accepted again.

/// Runs one control request on its own thread. A refusal that blocked would then fail the bound
/// instead of hanging the test.
fn queue_request_within_bound(state: &ControlState, request: serde_json::Value) -> ControlResponse {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let state = state.clone();
    std::thread::spawn(move || {
        let _ = done_tx.send(handle_request_value(request, &state, None));
    });
    done_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("a refused request is answered without blocking")
}

#[test]
fn a_full_resource_command_queue_refuses_requests_with_resource_busy_until_it_drains() {
    let harness = crate::harness::TestHarness::from_source(CONCURRENT_CONTROL_SOURCE)
        .expect("compile control fixture");
    let gate = Arc::new(crate::scheduler::StartGate::new());
    let (state, handle) =
        concurrent_control_state(CONCURRENT_CONTROL_SOURCE, harness, Some(Arc::clone(&gate)));
    concurrent_wait_until("the scan waits at its start gate", || {
        state.resource.state() == crate::scheduler::ResourceState::Ready
    });

    // The scan thread takes no command before its gate opens, so every command stays queued.
    let mut replies = Vec::new();
    for _ in 0..crate::scheduler::RESOURCE_COMMAND_CAPACITY {
        let (tx, rx) = std::sync::mpsc::channel();
        state
            .resource
            .send_command(crate::scheduler::ResourceCommand::Snapshot { respond_to: tx })
            .expect("the queue takes commands up to its bound");
        replies.push(rx);
    }
    let (tx, _rx) = std::sync::mpsc::channel();
    assert_eq!(
        state
            .resource
            .send_command(crate::scheduler::ResourceCommand::Snapshot { respond_to: tx }),
        Err(crate::scheduler::CommandSendError::Full),
        "the command after the bound is refused"
    );

    // Requests that need the queue are refused at once, with the stable code, and change nothing.
    let paused = queue_request_within_bound(&state, json!({"id": 1, "type": "pause"}));
    assert!(!paused.ok, "a pause finds the queue full");
    assert_eq!(paused.error_code.as_deref(), Some("resource_busy"));
    let reloaded = queue_request_within_bound(
        &state,
        json!({"id": 2, "type": "bytecode.reload", "params": {"bytes": "eA=="}}),
    );
    assert_eq!(
        reloaded.error_code.as_deref(),
        Some("resource_busy"),
        "a reload finds the queue full"
    );
    assert_eq!(
        state.resource.state(),
        crate::scheduler::ResourceState::Ready,
        "a refused request changes no resource state"
    );

    // The scan thread drains the queue once its gate opens, answering every queued command.
    gate.open();
    for rx in replies {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .expect("a queued command is answered");
    }
    let paused = handle_request_value(json!({"id": 3, "type": "pause"}), &state, None);
    assert!(
        paused.ok,
        "a drained queue accepts a pause: {:?}",
        paused.error
    );
    let resumed = handle_request_value(json!({"id": 4, "type": "resume"}), &state, None);
    assert!(
        resumed.ok,
        "a drained queue accepts a resume: {:?}",
        resumed.error
    );
    concurrent_stop_and_join(handle);
}

#[test]
fn a_full_queue_refuses_scheduler_policy_changes_and_leaves_every_domain_unchanged() {
    use crate::config::ControlMode;
    use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

    let mut state = hmi_test_state(CONCURRENT_CONTROL_SOURCE);
    // Nothing drains this stub queue, so the test decides exactly what is queued.
    let (control, queued) = ResourceControl::stub(StdClock::new());
    state.resource = control;
    for _ in 0..crate::scheduler::RESOURCE_COMMAND_CAPACITY {
        let (tx, _rx) = std::sync::mpsc::channel();
        state
            .resource
            .send_command(ResourceCommand::Snapshot { respond_to: tx })
            .expect("the queue takes commands up to its bound");
    }

    // A request that changes no scheduler policy sends nothing, so the full queue cannot refuse it.
    let empty = handle_request_value(
        json!({"id": 1, "type": "config.set", "params": {}}),
        &state,
        None,
    );
    assert!(
        empty.ok,
        "an empty config.set is a no-op on a full queue: {:?}",
        empty.error
    );
    assert_eq!(
        empty.result,
        Some(json!({"updated": [], "restart_required": []}))
    );

    // A policy change finds the queue full: it is refused, and no domain commits.
    let watchdog_before = state
        .settings
        .lock()
        .expect("settings lock")
        .watchdog
        .enabled;
    let requested = !watchdog_before;
    let change = json!({
        "id": 2,
        "type": "config.set",
        "params": {
            "control.mode": "production",
            "watchdog.enabled": requested,
            "fault.policy": "restart",
            "retain.save_interval_ms": 1500
        }
    });
    let refused = queue_request_within_bound(&state, change.clone());
    assert!(!refused.ok, "a policy change finds the queue full");
    assert_eq!(refused.error_code.as_deref(), Some("resource_busy"));
    assert!(
        matches!(
            *state.control_mode.lock().expect("control mode lock"),
            ControlMode::Debug
        ),
        "the refused control mode is not committed"
    );
    assert_eq!(
        state
            .settings
            .lock()
            .expect("settings lock")
            .watchdog
            .enabled,
        watchdog_before,
        "the refused watchdog setting is not committed"
    );
    // The fixture starts with fault policy `safe_halt` and no retain save interval, so the
    // request would change both. Refusing the whole set leaves both as they were.
    {
        let settings = state.settings.lock().expect("settings lock");
        assert_eq!(
            settings.fault_policy,
            crate::watchdog::FaultPolicy::SafeHalt,
            "the refused fault policy is not committed"
        );
        assert_eq!(
            settings.retain_save_interval, None,
            "the refused retain save interval is not committed"
        );
    }

    let status = queue_request_within_bound(&state, json!({"id": 3, "type": "ads.status"}));
    assert_eq!(
        status.error_code.as_deref(),
        Some("resource_busy"),
        "an ADS status read finds the queue full"
    );

    // The refused requests queued nothing: the queue holds only the commands that filled it.
    let held: Vec<ResourceCommand> = queued.try_iter().collect();
    assert_eq!(held.len(), crate::scheduler::RESOURCE_COMMAND_CAPACITY);
    assert!(
        held.iter()
            .all(|command| matches!(command, ResourceCommand::Snapshot { .. })),
        "no refused request left a command behind"
    );

    // Once the queue has room, the same change commits and its three policies travel together.
    let accepted = handle_request_value(change, &state, None);
    assert!(
        accepted.ok,
        "a drained queue accepts the change: {:?}",
        accepted.error
    );
    assert!(matches!(
        *state.control_mode.lock().expect("control mode lock"),
        ControlMode::Production
    ));
    assert_eq!(
        state
            .settings
            .lock()
            .expect("settings lock")
            .watchdog
            .enabled,
        requested
    );
    // The whole policy set is one command that carries all three policies.
    let sent: Vec<ResourceCommand> = queued.try_iter().collect();
    assert!(
        matches!(
            sent.as_slice(),
            [ResourceCommand::UpdateSchedulerPolicies(set)]
                if set.watchdog.enabled == requested
                    && set.fault_policy == crate::watchdog::FaultPolicy::Restart
                    && set.retain_save_interval
                        == Some(crate::value::Duration::from_millis(1500))
        ),
        "the whole policy set travels as one command: {sent:?}"
    );
}

#[test]
fn the_last_free_slot_takes_the_whole_policy_set_in_one_command() {
    use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

    let mut state = hmi_test_state(CONCURRENT_CONTROL_SOURCE);
    let (control, queued) = ResourceControl::stub(StdClock::new());
    state.resource = control;
    // One slot short of the bound: the policy set takes the last free slot.
    for _ in 0..(crate::scheduler::RESOURCE_COMMAND_CAPACITY - 1) {
        let (tx, _rx) = std::sync::mpsc::channel();
        state
            .resource
            .send_command(ResourceCommand::Snapshot { respond_to: tx })
            .expect("the queue takes commands up to its bound");
    }
    let watchdog_before = state
        .settings
        .lock()
        .expect("settings lock")
        .watchdog
        .enabled;
    let response = handle_request_value(
        json!({
            "id": 1,
            "type": "config.set",
            "params": {
                "watchdog.enabled": !watchdog_before,
                "fault.policy": "restart",
                "retain.save_interval_ms": 1500
            }
        }),
        &state,
        None,
    );
    assert!(
        response.ok,
        "the last free slot accepts the whole set: {:?}",
        response.error
    );
    let sent: Vec<ResourceCommand> = queued.try_iter().collect();
    assert_eq!(
        sent.len(),
        crate::scheduler::RESOURCE_COMMAND_CAPACITY,
        "the set fills the last slot and nothing else is queued"
    );
    assert!(
        matches!(
            sent.last(),
            Some(ResourceCommand::UpdateSchedulerPolicies(set))
                if set.watchdog.enabled == !watchdog_before
                    && set.fault_policy == crate::watchdog::FaultPolicy::Restart
                    && set.retain_save_interval
                        == Some(crate::value::Duration::from_millis(1500))
        ),
        "the last command is the whole policy set: {sent:?}"
    );
}
