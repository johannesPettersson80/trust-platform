// `config.set` performs every fallible step before it commits (specification 11 section 6.8.1). A
// poisoned lock fails the request before any setting, control token, control mode, debug switch or
// scheduler update changes. The handler is called directly, so the request-level role check, which
// also reads the control token, does not decide the outcome.

/// Poisons `lock` by panicking on another thread while its guard is held.
fn poison_config_test_lock<T: Send + 'static>(lock: std::sync::Arc<std::sync::Mutex<T>>) {
    let joined = std::thread::spawn(move || {
        let _guard = lock.lock().expect("the lock is not poisoned yet");
        panic!("poison the lock for the config.set commit test");
    })
    .join();
    assert!(
        joined.is_err(),
        "the poisoning thread panicked while it held the lock"
    );
}

/// Every domain `config.set` can commit. A poisoned lock is read through its inner value, so the
/// test sees exactly what a failed request left behind.
#[derive(Debug, PartialEq)]
struct ConfigDomains {
    settings: String,
    auth_token: Option<smol_str::SmolStr>,
    control_mode: crate::config::ControlMode,
    debug_enabled: bool,
}

fn config_domains(state: &ControlState) -> ConfigDomains {
    use std::sync::atomic::Ordering;
    use std::sync::PoisonError;

    ConfigDomains {
        settings: format!(
            "{:?}",
            *state
                .settings
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
        ),
        auth_token: state
            .auth_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
        control_mode: *state
            .control_mode
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        debug_enabled: state.debug_enabled.load(Ordering::Relaxed),
    }
}

/// A state whose resource queue nothing drains, so the test sees every scheduler update.
fn config_lock_state() -> (
    ControlState,
    std::sync::mpsc::Receiver<crate::scheduler::ResourceCommand>,
) {
    let mut state = hmi_test_state(CONCURRENT_CONTROL_SOURCE);
    let (control, queued) =
        crate::scheduler::ResourceControl::stub(crate::scheduler::StdClock::new());
    state.resource = control;
    (state, queued)
}

fn watchdog_enabled(state: &ControlState) -> bool {
    state
        .settings
        .lock()
        .expect("settings lock")
        .watchdog
        .enabled
}

#[test]
fn a_poisoned_control_mode_lock_fails_before_any_domain_commits() {
    let (state, queued) = config_lock_state();
    let before = config_domains(&state);
    let discovery_enabled = state
        .settings
        .lock()
        .expect("settings lock")
        .discovery
        .enabled;
    poison_config_test_lock(std::sync::Arc::clone(&state.control_mode));

    let response = super::config_handlers::handle_config_set(
        1,
        Some(json!({
            "control.auth_token": "rotated-token",
            "control.debug_enabled": false,
            "control.mode": "production",
            "watchdog.enabled": !watchdog_enabled(&state),
            "discovery.enabled": !discovery_enabled,
        })),
        &state,
    );

    assert!(
        !response.ok,
        "a poisoned control-mode lock refuses the request"
    );
    assert_eq!(response.error.as_deref(), Some("control mode unavailable"));
    assert_eq!(
        config_domains(&state),
        before,
        "the failed request commits no domain"
    );
    assert!(
        queued.try_iter().next().is_none(),
        "the failed request queues no scheduler update"
    );
}

#[test]
fn a_poisoned_auth_token_lock_fails_before_any_domain_commits() {
    let (state, queued) = config_lock_state();
    let before = config_domains(&state);
    poison_config_test_lock(std::sync::Arc::clone(&state.auth_token));

    let response = super::config_handlers::handle_config_set(
        1,
        Some(json!({
            "control.auth_token": "rotated-token",
            "control.debug_enabled": false,
            "control.mode": "production",
            "watchdog.enabled": !watchdog_enabled(&state),
        })),
        &state,
    );

    assert!(
        !response.ok,
        "a poisoned auth-token lock refuses the request"
    );
    assert_eq!(response.error.as_deref(), Some("auth token unavailable"));
    assert_eq!(
        config_domains(&state),
        before,
        "the failed request commits no domain"
    );
    assert!(
        queued.try_iter().next().is_none(),
        "the failed request queues no scheduler update"
    );
}

#[test]
fn a_request_that_succeeds_commits_every_domain_it_names() {
    let (state, queued) = config_lock_state();
    let requested = !watchdog_enabled(&state);

    let response = super::config_handlers::handle_config_set(
        1,
        Some(json!({
            "control.auth_token": "rotated-token",
            "control.debug_enabled": false,
            "control.mode": "production",
            "watchdog.enabled": requested,
        })),
        &state,
    );

    assert!(
        response.ok,
        "a request with healthy locks is accepted: {:?}",
        response.error
    );
    let after = config_domains(&state);
    assert_eq!(after.auth_token.as_deref(), Some("rotated-token"));
    assert_eq!(after.control_mode, crate::config::ControlMode::Production);
    assert!(!after.debug_enabled);
    assert_eq!(watchdog_enabled(&state), requested);
    let sent: Vec<crate::scheduler::ResourceCommand> = queued.try_iter().collect();
    assert!(
        matches!(
            sent.as_slice(),
            [crate::scheduler::ResourceCommand::UpdateSchedulerPolicies(set)]
                if set.watchdog.enabled == requested
        ),
        "the accepted policy set is queued as one command: {sent:?}"
    );
}
