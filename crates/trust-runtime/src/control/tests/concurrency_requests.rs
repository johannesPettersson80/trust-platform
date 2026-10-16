// Concurrency through the control request boundary (specification 11 sections 6.6, 6.8.1 and 6.9).
// Real clients send control requests while a real scan thread runs. Each request gets exactly one
// stable reply, and restarts accepted before the scan takes them coalesce, keeping a cold one.

const CONCURRENT_CONTROL_SOURCE: &str = r#"
VAR_GLOBAL
    cycles : DINT := DINT#0;
END_VAR

PROGRAM Main
    cycles := cycles + DINT#1;
END_PROGRAM
"#;

const RETAINED_RESTART_SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    retained : DINT := DINT#0;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

fn concurrent_wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// A control state whose resource is a real scan thread. A start gate holds the thread before its
/// first scan when one is given.
fn concurrent_control_state(
    source: &str,
    harness: crate::harness::TestHarness,
    gate: Option<Arc<crate::scheduler::StartGate>>,
) -> (
    ControlState,
    crate::scheduler::ResourceHandle<crate::scheduler::StdClock>,
) {
    let mut state = hmi_test_state(source);
    *state.control_mode.lock().expect("control mode lock") = crate::config::ControlMode::Production;
    let mut runner = crate::scheduler::ResourceRunner::new(
        harness.into_runtime(),
        crate::scheduler::StdClock::new(),
        crate::value::Duration::from_millis(1),
    )
    .with_restart_signal(Arc::clone(&state.pending_restart));
    if let Some(gate) = gate {
        runner = runner.with_start_gate(gate);
    }
    let handle = runner.spawn("control-requests").expect("spawn resource");
    state.resource = handle.control();
    (state, handle)
}

fn concurrent_stop_and_join(
    mut handle: crate::scheduler::ResourceHandle<crate::scheduler::StdClock>,
) {
    handle.stop();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let joiner = std::thread::spawn(move || {
        let result = handle.join();
        let _ = done_tx.send(());
        result
    });
    done_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the resource thread ends within the bound");
    joiner
        .join()
        .expect("joiner thread")
        .expect("the resource thread ends without panicking");
}

#[test]
fn concurrent_control_requests_each_get_one_stable_reply_while_the_scan_runs() {
    use base64::Engine as _;
    let harness = crate::harness::TestHarness::from_source(CONCURRENT_CONTROL_SOURCE)
        .expect("compile control fixture");
    let bytes = crate::harness::bytecode_bytes_from_source(CONCURRENT_CONTROL_SOURCE)
        .expect("compile replacement bytecode");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let (state, handle) = concurrent_control_state(CONCURRENT_CONTROL_SOURCE, harness, None);
    concurrent_wait_until("the scan runs", || {
        state.resource.state() == crate::scheduler::ResourceState::Running
    });

    let mut clients = Vec::new();
    for client in 0..2_u64 {
        let state = state.clone();
        clients.push(std::thread::spawn(move || {
            for round in 0..30_u64 {
                let id = client * 1_000 + round;
                let response =
                    handle_request_value(json!({"id": id, "type": "status"}), &state, None);
                assert!(
                    response.ok,
                    "a status request is answered: {:?}",
                    response.error
                );
            }
        }));
    }
    {
        let state = state.clone();
        clients.push(std::thread::spawn(move || {
            for round in 0..15_u64 {
                let paused = handle_request_value(
                    json!({"id": 2_000 + round * 2, "type": "pause"}),
                    &state,
                    None,
                );
                assert!(
                    paused.ok,
                    "pause on a healthy resource is answered: {:?}",
                    paused.error
                );
                let resumed = handle_request_value(
                    json!({"id": 2_001 + round * 2, "type": "resume"}),
                    &state,
                    None,
                );
                assert!(
                    resumed.ok,
                    "resume on a healthy resource is answered: {:?}",
                    resumed.error
                );
            }
        }));
    }
    {
        let state = state.clone();
        clients.push(std::thread::spawn(move || {
            for round in 0..10_u64 {
                let response = handle_request_value(
                    json!({"id": 3_000 + round, "type": "restart", "params": {"mode": "warm"}}),
                    &state,
                    None,
                );
                assert!(
                    response.ok,
                    "a warm restart is queued while the scan runs: {:?}",
                    response.error
                );
            }
        }));
    }
    {
        let state = state.clone();
        clients.push(std::thread::spawn(move || {
            for round in 0..5_u64 {
                let response = handle_request_value(
                    json!({"id": 4_000 + round, "type": "bytecode.reload", "params": {"bytes": encoded.clone()}}),
                    &state,
                    None,
                );
                assert!(
                    response.ok,
                    "an accepted replacement is answered: {:?}",
                    response.error
                );
            }
        }));
    }
    for client in clients {
        client.join().expect("control client thread");
    }

    // Every pause was followed by its resume, so the last command leaves the scan running.
    concurrent_wait_until("the scan runs after the clients finish", || {
        state.resource.state() == crate::scheduler::ResourceState::Running
    });
    let status = handle_request_value(json!({"id": 9_001, "type": "status"}), &state, None);
    assert!(
        status.ok,
        "status is answered at the end: {:?}",
        status.error
    );
    concurrent_stop_and_join(handle);
}

#[test]
fn pending_restarts_coalesce_and_a_cold_request_keeps_precedence() {
    let mut harness = crate::harness::TestHarness::from_source(RETAINED_RESTART_SOURCE)
        .expect("compile restart fixture");
    harness.set_input("retained", crate::value::Value::DInt(5));
    let gate = Arc::new(crate::scheduler::StartGate::new());
    let (state, handle) =
        concurrent_control_state(RETAINED_RESTART_SOURCE, harness, Some(Arc::clone(&gate)));
    concurrent_wait_until("the scan waits at its start gate", || {
        state.resource.state() == crate::scheduler::ResourceState::Ready
    });

    // The scan thread takes restart requests only once the gate opens, so both are pending together.
    let cold = handle_request_value(
        json!({"id": 1, "type": "restart", "params": {"mode": "cold"}}),
        &state,
        None,
    );
    assert!(cold.ok, "the cold restart is accepted: {:?}", cold.error);
    let warm = handle_request_value(
        json!({"id": 2, "type": "restart", "params": {"mode": "warm"}}),
        &state,
        None,
    );
    assert!(warm.ok, "the warm restart is accepted: {:?}", warm.error);

    gate.open();
    concurrent_wait_until("the scan takes the pending restart", || {
        state
            .pending_restart
            .lock()
            .expect("restart signal lock")
            .is_none()
    });
    let (tx, rx) = std::sync::mpsc::channel();
    state
        .resource
        .send_command(crate::scheduler::ResourceCommand::Snapshot { respond_to: tx })
        .expect("command channel open");
    let snapshot = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("snapshot reply within the bound");
    // The two accepted restarts coalesce into one cold restart: the later warm request does not
    // downgrade the pending cold one, so the retained value is reset (spec 11 section 6.9).
    assert_eq!(
        snapshot.storage.get_global("retained"),
        Some(&crate::value::Value::DInt(0)),
        "an accepted cold restart is not lost behind a later warm restart"
    );
    concurrent_stop_and_join(handle);
}

#[test]
fn a_later_cold_request_upgrades_a_pending_warm_restart() {
    // Specification 11 section 6.9: a cold request takes precedence over a warm one, in either
    // arrival order. Each accepted request is answered once, with the same queued reply.
    let state = hmi_test_state(CONCURRENT_CONTROL_SOURCE);
    let cases = [
        ("warm", "cold", crate::RestartMode::Cold),
        ("cold", "warm", crate::RestartMode::Cold),
        ("warm", "warm", crate::RestartMode::Warm),
    ];
    for (first, second, expected) in cases {
        *state.pending_restart.lock().expect("restart signal lock") = None;
        for (id, mode) in [(1_u64, first), (2_u64, second)] {
            let response = handle_request_value(
                json!({"id": id, "type": "restart", "params": {"mode": mode}}),
                &state,
                None,
            );
            assert_eq!(
                response.result,
                Some(json!({"status": "restart queued"})),
                "{first} then {second}: the {mode} request is answered"
            );
        }
        assert_eq!(
            *state.pending_restart.lock().expect("restart signal lock"),
            Some(expected),
            "{first} then {second}: one pending restart with the expected mode"
        );
    }
}
