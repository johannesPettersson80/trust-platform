// Spec 11 "Identify": the control state answers `identify` and reports `identifying` from the
// identify state its handle holds. Without `[runtime.identify] output` it answers
// `identify_not_configured`; with one, a resource in any state starts the blink, and a Viewer is
// refused.

/// A control state with a running resource, and the identify output `output` when there is one.
fn identify_fixture(output: Option<&str>) -> ControlState {
    identify_fixture_in(output, crate::scheduler::ResourceState::Running)
}

/// A control state with a resource in `resource`, and the identify output `output` when there is
/// one.
fn identify_fixture_in(
    output: Option<&str>,
    resource: crate::scheduler::ResourceState,
) -> ControlState {
    let (state, _commands) =
        resource_state_fixture(crate::config::ControlMode::Production, resource);
    if let Some(output) = output {
        let setting = crate::config::IdentifyOutputConfig::parse(output).expect("an output");
        let io = crate::config::IoConfig {
            drivers: Vec::new(),
            safe_state: crate::io::IoSafeState::default(),
        };
        state.identify.set(Some(
            crate::io::host_identify(&setting, &io).expect("an image output builds"),
        ));
    }
    state
}

fn identifying_in_status(state: &ControlState) -> bool {
    let status = handle_request_value(json!({"id": 99, "type": "status"}), state, None);
    assert!(status.ok, "status failed: {:?}", status.error);
    status.result_value().expect("status result")["identifying"]
        .as_bool()
        .expect("status reports identifying as a Boolean")
}

#[test]
fn the_control_state_starts_identify_and_reports_it_in_status() {
    // Specification 11, "Identify": a running resource with an identify output accepts the request,
    // answers `{"identifying": true, "duration_s": n}`, and `status` reports `identifying: true`
    // from then on; a Viewer is refused with `insufficient_role` and starts nothing. Catches a
    // control state that does not reach the identify state its handle holds, and one that lets a
    // Viewer blink an output.
    let state = identify_fixture(Some("%QX0.7"));
    assert!(!identifying_in_status(&state), "nothing was asked yet");

    let viewer = handle_request_value(
        json!({"id": 1, "type": "identify", "params": {"duration_s": 5}}),
        &state,
        Some("127.0.0.1:9000"),
    );
    assert_eq!(viewer.error_code.as_deref(), Some("insufficient_role"));
    assert!(
        !identifying_in_status(&state),
        "a refused request starts nothing"
    );

    let started = handle_request_value(
        json!({"id": 2, "type": "identify", "params": {"duration_s": 5}}),
        &state,
        None,
    );
    assert!(started.ok, "identify failed: {:?}", started.error);
    assert_eq!(
        started.result_value(),
        Some(json!({"identifying": true, "duration_s": 5}))
    );
    assert!(identifying_in_status(&state));
}

#[test]
fn a_control_state_without_an_identify_output_answers_identify_not_configured() {
    // Specification 11, "Identify": without `[runtime.identify] output` the request answers
    // `identify_not_configured` and `status` reports `identifying: false`. Catches a host that
    // answers `unsupported request` for the operation, or accepts it without an output.
    let state = identify_fixture(None);

    let response = handle_request_value(
        json!({"id": 1, "type": "identify", "params": {"duration_s": 5}}),
        &state,
        None,
    );

    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("identify_not_configured")
    );
    assert!(!identifying_in_status(&state));
}

#[test]
fn the_control_state_starts_identify_in_every_state_of_the_resource() {
    // Specification 11, "Identify", request: `identify` has no restriction on the state of the
    // resource. A resource that is paused, stopped or faulted accepts the request with the same
    // result and `status` reports `identifying: true`. Catches the old refusal
    // `resource_not_running`, which would have kept a stopped box from being found.
    for resource in [
        crate::scheduler::ResourceState::Running,
        crate::scheduler::ResourceState::Paused,
        crate::scheduler::ResourceState::Stopped,
        crate::scheduler::ResourceState::Faulted,
    ] {
        let state = identify_fixture_in(Some("%QX0.7"), resource);

        let started = handle_request_value(
            json!({"id": 1, "type": "identify", "params": {"duration_s": 5}}),
            &state,
            None,
        );

        assert!(
            started.ok,
            "identify failed in {resource:?}: {:?}",
            started.error
        );
        assert_eq!(
            started.result_value(),
            Some(json!({"identifying": true, "duration_s": 5})),
            "resource={resource:?}"
        );
        assert!(identifying_in_status(&state), "resource={resource:?}");
    }
}
