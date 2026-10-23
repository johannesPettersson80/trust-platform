// Reload withdrawal (specification 11 section 6.7.1). A reload that the scan thread has not taken
// within the wait is withdrawn and never applied; a reload the scan thread took reports its own
// result. The waits are injected, so no test waits for the production bound.

const RELOAD_WITHDRAWAL_BEFORE: &str = r#"
VAR_GLOBAL
    marker : DINT := DINT#1;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

const RELOAD_WITHDRAWAL_AFTER: &str = r#"
VAR_GLOBAL
    marker : DINT := DINT#2;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

/// Reads the `marker` global through a snapshot command, which the scan thread answers in arrival
/// order. The answer therefore follows every command queued before it.
fn reload_withdrawal_marker(state: &ControlState) -> crate::value::Value {
    let (tx, rx) = std::sync::mpsc::channel();
    state
        .resource
        .send_command(crate::scheduler::ResourceCommand::Snapshot { respond_to: tx })
        .expect("command channel open");
    let snapshot = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("snapshot reply within the bound");
    snapshot
        .storage
        .get_global("marker")
        .cloned()
        .expect("the marker global exists")
}

#[test]
fn a_reload_withdrawn_before_the_resource_takes_it_is_never_applied() {
    let harness = crate::harness::TestHarness::from_source(RELOAD_WITHDRAWAL_BEFORE)
        .expect("compile the current program");
    let bytes = crate::harness::bytecode_bytes_from_source(RELOAD_WITHDRAWAL_AFTER)
        .expect("compile the replacement");
    let gate = Arc::new(crate::scheduler::StartGate::new());
    let (state, handle) =
        concurrent_control_state(RELOAD_WITHDRAWAL_BEFORE, harness, Some(Arc::clone(&gate)));
    concurrent_wait_until("the scan waits at its start gate", || {
        state.resource.state() == crate::scheduler::ResourceState::Ready
    });

    // The scan thread takes no command before its gate opens, so the reload is still queued when
    // the wait ends and is withdrawn.
    let wait = super::program_handlers::ReloadWait {
        take: std::time::Duration::from_millis(50),
        commit: std::time::Duration::from_millis(50),
    };
    let timed_out = super::program_handlers::reload_on_resource(1, &state, bytes, wait);
    assert!(!timed_out.ok, "a reload the resource did not take fails");
    assert_eq!(timed_out.error.as_deref(), Some("reload timeout"));
    assert_eq!(timed_out.error_code, None);

    // The withdrawn reload is still in the queue. Once the scan thread drains it, the marker keeps
    // its value: a reload that was applied would have reset it to 2.
    gate.open();
    assert_eq!(
        reload_withdrawal_marker(&state),
        crate::value::Value::DInt(1),
        "a withdrawn reload is never applied"
    );
    assert_eq!(
        state.resource.state(),
        crate::scheduler::ResourceState::Running,
        "the scan keeps running on the program it had"
    );
    concurrent_stop_and_join(handle);
}

#[test]
fn a_reload_the_resource_takes_reports_its_real_result() {
    let harness = crate::harness::TestHarness::from_source(RELOAD_WITHDRAWAL_BEFORE)
        .expect("compile the current program");
    let bytes = crate::harness::bytecode_bytes_from_source(RELOAD_WITHDRAWAL_AFTER)
        .expect("compile the replacement");
    let (state, handle) = concurrent_control_state(RELOAD_WITHDRAWAL_BEFORE, harness, None);
    concurrent_wait_until("the scan runs", || {
        state.resource.state() == crate::scheduler::ResourceState::Running
    });

    let reloaded = super::program_handlers::reload_on_resource(
        1,
        &state,
        bytes,
        super::program_handlers::RELOAD_WAIT,
    );
    assert!(
        reloaded.ok,
        "a reload taken at once is applied: {:?}",
        reloaded.error
    );
    assert_eq!(reloaded.result, Some(json!({"status": "reloaded"})));
    assert_eq!(
        reload_withdrawal_marker(&state),
        crate::value::Value::DInt(2),
        "the taken reload is applied"
    );
    concurrent_stop_and_join(handle);
}

#[test]
fn a_reload_taken_before_its_bound_reports_the_result_not_a_timeout() {
    // The scan thread took the request, so the wait cannot withdraw it. The result it reports is
    // the answer, even though it arrives after the take bound has passed.
    let claim = crate::scheduler::ReloadClaim::default();
    assert!(claim.take(), "the scan thread takes the pending reload");
    let (reply_tx, reply_rx) =
        std::sync::mpsc::channel::<Result<crate::RuntimeMetadata, crate::error::RuntimeError>>();
    let scan_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = reply_tx.send(Err(crate::error::RuntimeError::ControlError(
            "commit result".into(),
        )));
    });
    let wait = super::program_handlers::ReloadWait {
        take: std::time::Duration::from_millis(10),
        commit: std::time::Duration::from_secs(5),
    };
    let outcome = super::program_handlers::await_reload(&claim, &reply_rx, wait);
    scan_thread.join().expect("scan thread");
    match outcome {
        super::program_handlers::ReloadOutcome::Applied(result) => assert_eq!(
            result.err(),
            Some(crate::error::RuntimeError::ControlError(
                "commit result".into()
            )),
            "the taken reload reports its own result"
        ),
        other => panic!("a taken reload reports its own result, found {other:?}"),
    }
    assert!(!claim.withdraw(), "a taken reload is never withdrawn");
}

#[test]
fn a_taken_reload_without_a_result_within_the_commit_bound_is_unknown() {
    let claim = crate::scheduler::ReloadClaim::default();
    assert!(claim.take(), "the scan thread takes the pending reload");
    // The sender stays alive, so the wait times out rather than seeing a closed channel.
    let (reply_tx, reply_rx) =
        std::sync::mpsc::channel::<Result<crate::RuntimeMetadata, crate::error::RuntimeError>>();
    let wait = super::program_handlers::ReloadWait {
        take: std::time::Duration::from_millis(10),
        commit: std::time::Duration::from_millis(20),
    };
    let outcome = super::program_handlers::await_reload(&claim, &reply_rx, wait);
    assert!(
        matches!(outcome, super::program_handlers::ReloadOutcome::Unknown),
        "a taken reload with no result is unknown, found {outcome:?}"
    );
    assert!(!claim.withdraw(), "a taken reload is never withdrawn");
    drop(reply_tx);
}

#[test]
fn a_reload_the_scheduler_drops_unapplied_is_reported_as_stopped() {
    let claim = crate::scheduler::ReloadClaim::default();
    let (reply_tx, reply_rx) =
        std::sync::mpsc::channel::<Result<crate::RuntimeMetadata, crate::error::RuntimeError>>();
    // The scheduler dropped the queued command without taking it.
    drop(reply_tx);
    let wait = super::program_handlers::ReloadWait {
        take: std::time::Duration::from_secs(1),
        commit: std::time::Duration::from_secs(1),
    };
    let outcome = super::program_handlers::await_reload(&claim, &reply_rx, wait);
    assert!(
        matches!(outcome, super::program_handlers::ReloadOutcome::Stopped),
        "a dropped, untaken reload is reported as stopped, found {outcome:?}"
    );
    assert!(!claim.take(), "a withdrawn reload is never taken");
}

#[test]
fn a_reload_claim_is_taken_or_withdrawn_exactly_once() {
    let withdrawn = crate::scheduler::ReloadClaim::default();
    assert!(withdrawn.withdraw(), "the first withdrawal wins");
    assert!(!withdrawn.take(), "a withdrawn reload is never taken");
    assert!(!withdrawn.withdraw(), "a reload is withdrawn only once");

    let taken = crate::scheduler::ReloadClaim::default();
    assert!(taken.take(), "the first take wins");
    assert!(!taken.withdraw(), "a taken reload is never withdrawn");
    assert!(!taken.take(), "a reload is taken only once");
}
