use super::*;
use crate::protocol::{
    AttachArguments, DisconnectArguments, Event, InitializeArguments, LaunchArguments, MessageType,
    Request, Response, TerminateArguments, TerminatedEventBody,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn adapter() -> DebugAdapter {
    DebugAdapter::new(DebugSession::new(Runtime::new()))
}

fn request(seq: u32, command: &str, arguments: Option<Value>) -> Request<Value> {
    Request {
        seq,
        message_type: MessageType::Request,
        command: command.to_string(),
        arguments,
    }
}

fn initialize_request(
    seq: u32,
    lines_start_at1: Option<bool>,
    columns_start_at1: Option<bool>,
) -> Request<Value> {
    request(
        seq,
        "initialize",
        Some(
            serde_json::to_value(InitializeArguments {
                lines_start_at1,
                columns_start_at1,
                ..InitializeArguments::default()
            })
            .unwrap(),
        ),
    )
}

fn launch_request(seq: u32, values: &[(&str, Value)]) -> Request<Value> {
    let additional = values
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    request(
        seq,
        "launch",
        Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    )
}

fn attach_request(seq: u32, values: &[(&str, Value)]) -> Request<Value> {
    let additional = values
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    request(
        seq,
        "attach",
        Some(serde_json::to_value(AttachArguments { additional }).unwrap()),
    )
}

fn response(value: &Value) -> Response<Value> {
    serde_json::from_value(value.clone()).expect("DAP response")
}

fn event(value: &Value) -> Event<Value> {
    serde_json::from_value(value.clone()).expect("DAP event")
}

fn initialized_event_count(events: &[Value]) -> usize {
    events
        .iter()
        .map(event)
        .filter(|event| event.event == "initialized")
        .count()
}

fn internal_messages(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .map(event)
        .filter(|event| event.event == "trustDebugInternal")
        .filter_map(|event| event.body)
        .filter_map(|body| {
            body.get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn assert_one_failed_response(outcome: &DispatchOutcome, request_seq: u32, command: &str) {
    assert_eq!(
        outcome.responses.len(),
        1,
        "rejection must have exactly one response"
    );
    let response = response(&outcome.responses[0]);
    assert!(!response.success);
    assert_eq!(response.request_seq, request_seq);
    assert_eq!(response.command, command);
}

fn initialize(adapter: &mut DebugAdapter) {
    let outcome = adapter.dispatch_request(initialize_request(1, None, None));
    assert_eq!(outcome.responses.len(), 1);
    assert!(response(&outcome.responses[0]).success);
}

fn invalid_control_launch(seq: u32) -> Request<Value> {
    launch_request(
        seq,
        &[(
            "controlEndpoint",
            Value::String("not-a-control-endpoint".into()),
        )],
    )
}

fn assert_no_launch_actions(actions: LaunchActions) {
    assert!(!actions.pause_after_launch);
    assert!(!actions.start_runner_after_launch);
}

#[test]
fn lifecycle_starts_new_and_unconfigured_without_pending_work() {
    let adapter = adapter();

    assert!(!adapter.launch_state.is_configured());
    assert!(!adapter.launch_state.has_pending_launch());
    assert_eq!(adapter.launch_state.pending_since(), None);
    assert_no_launch_actions(adapter.launch_state.pending_actions());
}

#[test]
fn initialize_records_zero_based_coordinate_conventions() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(initialize_request(7, Some(false), Some(false)));

    assert!(response(&outcome.responses[0]).success);
    assert_eq!(initialized_event_count(&outcome.events), 1);
    assert!(!adapter.coordinate.lines_start_at1());
    assert!(!adapter.coordinate.columns_start_at1());
    assert_eq!(adapter.coordinate.to_runtime_line(0), Some(0));
    assert_eq!(adapter.coordinate.to_runtime_column(0), Some(0));
    assert_eq!(adapter.coordinate.to_client_line(0), 0);
    assert_eq!(adapter.coordinate.to_client_column(0), 0);
}

#[test]
fn initialize_records_one_based_coordinate_conventions_by_default() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(initialize_request(8, None, None));

    assert!(response(&outcome.responses[0]).success);
    assert_eq!(initialized_event_count(&outcome.events), 1);
    assert!(adapter.coordinate.lines_start_at1());
    assert!(adapter.coordinate.columns_start_at1());
    assert_eq!(adapter.coordinate.to_runtime_line(1), Some(0));
    assert_eq!(adapter.coordinate.to_runtime_column(1), Some(0));
    assert_eq!(adapter.coordinate.to_runtime_line(0), None);
    assert_eq!(adapter.coordinate.to_runtime_column(0), None);
}

#[test]
fn repeated_initialize_fails_without_emitting_or_resetting() {
    let mut adapter = adapter();
    let first = adapter.dispatch_request(initialize_request(10, Some(false), Some(false)));
    assert_eq!(initialized_event_count(&first.events), 1);
    let pending = adapter.dispatch_request(invalid_control_launch(11));
    assert!(pending.responses.is_empty());
    assert!(adapter.launch_state.has_pending_launch());

    let second = adapter.dispatch_request(initialize_request(12, Some(true), Some(true)));

    assert_one_failed_response(&second, 12, "initialize");
    assert_eq!(initialized_event_count(&second.events), 0);
    assert!(!adapter.coordinate.lines_start_at1());
    assert!(!adapter.coordinate.columns_start_at1());
    assert!(
        adapter.launch_state.has_pending_launch(),
        "duplicate initialize must not erase the pending launch"
    );
}

#[test]
fn malformed_initialize_arguments_fail_closed() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(request(
        20,
        "initialize",
        Some(json!({"linesStartAt1": "yes"})),
    ));

    assert_one_failed_response(&outcome, 20, "initialize");
    assert_eq!(initialized_event_count(&outcome.events), 0);
    assert!(!adapter.launch_state.is_configured());
}

#[test]
fn launch_before_initialize_is_rejected_without_pending_state() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(invalid_control_launch(30));

    assert_one_failed_response(&outcome, 30, "launch");
    assert!(!adapter.launch_state.has_pending_launch());
}

#[test]
fn attach_before_initialize_is_rejected_without_pending_state() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(attach_request(
        31,
        &[("endpoint", Value::String("tcp://127.0.0.1:1".into()))],
    ));

    assert_one_failed_response(&outcome, 31, "attach");
    assert!(!adapter.launch_state.has_pending_launch());
    assert!(adapter.remote_session.is_none());
}

#[test]
fn configuration_done_before_initialize_is_rejected() {
    let mut adapter = adapter();

    let outcome = adapter.dispatch_request(request(32, "configurationDone", None));

    assert_one_failed_response(&outcome, 32, "configurationDone");
    assert!(!adapter.launch_state.is_configured());
}

#[test]
fn malformed_launch_arguments_fail_without_becoming_pending() {
    let mut adapter = adapter();
    initialize(&mut adapter);

    let outcome = adapter.dispatch_request(request(
        40,
        "launch",
        Some(Value::String("not-an-object".into())),
    ));

    assert_one_failed_response(&outcome, 40, "launch");
    assert!(!adapter.launch_state.has_pending_launch());
}

#[test]
fn malformed_attach_arguments_fail_without_becoming_pending() {
    let mut adapter = adapter();
    initialize(&mut adapter);

    let outcome = adapter.dispatch_request(request(41, "attach", Some(Value::Array(Vec::new()))));

    assert_one_failed_response(&outcome, 41, "attach");
    assert!(!adapter.launch_state.has_pending_launch());
    assert!(adapter.remote_session.is_none());
}

#[test]
fn first_launch_is_deferred_until_configuration_done() {
    let mut adapter = adapter();
    initialize(&mut adapter);

    let outcome = adapter.dispatch_request(invalid_control_launch(50));

    assert!(outcome.responses.is_empty());
    assert!(!adapter.launch_state.is_configured());
    assert!(adapter.launch_state.has_pending_launch());
    assert!(internal_messages(&outcome.events)
        .iter()
        .any(|message| message.contains("launch deferred until configurationDone")));
}

#[test]
fn first_attach_is_deferred_until_configuration_done() {
    let mut adapter = adapter();
    initialize(&mut adapter);

    let outcome = adapter.dispatch_request(attach_request(
        51,
        &[("endpoint", Value::String("tcp://127.0.0.1:1".into()))],
    ));

    assert!(outcome.responses.is_empty());
    assert!(!adapter.launch_state.is_configured());
    assert!(adapter.launch_state.has_pending_launch());
    assert!(internal_messages(&outcome.events)
        .iter()
        .any(|message| message.contains("attach deferred until configurationDone")));
}

#[test]
fn second_launch_fails_and_cannot_replace_first_pending_launch() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    assert!(adapter
        .dispatch_request(invalid_control_launch(60))
        .responses
        .is_empty());

    let duplicate = adapter.dispatch_request(invalid_control_launch(61));
    assert_one_failed_response(&duplicate, 61, "launch");

    let configured = adapter.dispatch_request(request(
        62,
        "configurationDone",
        Some(Value::Object(Default::default())),
    ));
    assert_eq!(configured.responses.len(), 2);
    assert_eq!(response(&configured.responses[0]).request_seq, 62);
    assert_eq!(
        response(&configured.responses[1]).request_seq,
        60,
        "configurationDone must execute the first pending launch"
    );
}

#[test]
fn competing_attach_fails_and_cannot_replace_pending_launch() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    assert!(adapter
        .dispatch_request(invalid_control_launch(70))
        .responses
        .is_empty());

    let competing = adapter.dispatch_request(attach_request(
        71,
        &[("endpoint", Value::String("tcp://127.0.0.1:1".into()))],
    ));
    assert_one_failed_response(&competing, 71, "attach");

    let configured = adapter.dispatch_request(request(72, "configurationDone", None));
    assert_eq!(configured.responses.len(), 2);
    assert_eq!(response(&configured.responses[1]).request_seq, 70);
}

#[test]
fn competing_launch_fails_and_cannot_replace_pending_attach() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    assert!(adapter
        .dispatch_request(attach_request(
            80,
            &[("endpoint", Value::String("tcp://127.0.0.1:1".into()))],
        ))
        .responses
        .is_empty());

    let competing = adapter.dispatch_request(invalid_control_launch(81));
    assert_one_failed_response(&competing, 81, "launch");

    let configured = adapter.dispatch_request(request(82, "configurationDone", None));
    assert_eq!(configured.responses.len(), 2);
    assert_eq!(response(&configured.responses[1]).request_seq, 80);
}

#[test]
fn configuration_done_responds_before_the_exact_pending_launch_response() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(90));

    let outcome = adapter.dispatch_request(request(91, "configurationDone", None));

    assert_eq!(outcome.responses.len(), 2);
    let configuration_response = response(&outcome.responses[0]);
    let launch_response = response(&outcome.responses[1]);
    assert!(configuration_response.success);
    assert_eq!(configuration_response.request_seq, 91);
    assert_eq!(configuration_response.command, "configurationDone");
    assert!(!launch_response.success);
    assert_eq!(launch_response.request_seq, 90);
    assert_eq!(launch_response.command, "launch");
    assert!(!adapter.launch_state.has_pending_launch());
    assert!(adapter.launch_state.is_configured());
}

#[test]
fn repeated_configuration_done_is_idempotent() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(100));
    let first = adapter.dispatch_request(request(101, "configurationDone", None));
    assert_eq!(first.responses.len(), 2);

    let second = adapter.dispatch_request(request(102, "configurationDone", None));

    assert_eq!(second.responses.len(), 1);
    let response = response(&second.responses[0]);
    assert!(response.success);
    assert_eq!(response.request_seq, 102);
    assert!(!adapter.launch_state.has_pending_launch());
    assert_no_launch_actions(adapter.launch_state.pending_actions());
}

#[test]
fn configuration_requests_do_not_force_a_pending_start() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(110));

    for command in [
        "initialize",
        "launch",
        "attach",
        "setBreakpoints",
        "setExceptionBreakpoints",
        "setFunctionBreakpoints",
        "setInstructionBreakpoints",
        "setDataBreakpoints",
        "configurationDone",
    ] {
        assert!(
            adapter.maybe_force_start_after_timeout(command).is_none(),
            "{command} must remain inside the configuration phase"
        );
        assert!(adapter.launch_state.has_pending_launch());
    }
}

#[test]
fn first_non_configuration_request_force_starts_the_exact_pending_request_once() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(120));

    let outcome = adapter
        .maybe_force_start_after_timeout("threads")
        .expect("compatibility start");

    assert_eq!(outcome.responses.len(), 1);
    let launch_response = response(&outcome.responses[0]);
    assert!(!launch_response.success);
    assert_eq!(launch_response.request_seq, 120);
    assert_eq!(launch_response.command, "launch");
    assert!(internal_messages(&outcome.events)
        .iter()
        .any(|message| message.contains("configurationDone missing")));
    assert!(adapter.maybe_force_start_after_timeout("threads").is_none());
}

#[test]
fn post_launch_actions_are_consumed_exactly_once() {
    let mut state = LaunchState::default();
    state.set_post_launch(LaunchActions {
        pause_after_launch: true,
        start_runner_after_launch: true,
    });

    let first = state.take_actions();
    assert!(first.pause_after_launch);
    assert!(first.start_runner_after_launch);
    assert_no_launch_actions(state.take_actions());
    assert!(state.is_configured());
}

#[test]
fn failed_launch_schedules_no_post_launch_actions() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(130));

    let outcome = adapter.dispatch_request(request(131, "configurationDone", None));

    assert!(!response(&outcome.responses[1]).success);
    assert_no_launch_actions(adapter.launch_state.pending_actions());
    assert!(adapter.runner.is_none());
    assert!(adapter.control_server.is_none());
}

#[test]
fn unsupported_request_is_failed_and_preserves_lifecycle_state() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(140));

    let outcome = adapter.dispatch_request(request(141, "unknownCommand", None));

    assert_one_failed_response(&outcome, 141, "unknownCommand");
    assert!(
        adapter.launch_state.has_pending_launch(),
        "dispatch alone must not consume pending compatibility work"
    );
    assert!(!adapter.launch_state.is_configured());
}

#[test]
fn non_request_envelope_is_ignored_without_changing_state() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    adapter.dispatch_request(invalid_control_launch(150));
    let envelope = Request {
        seq: 151,
        message_type: MessageType::Event,
        command: "configurationDone".to_string(),
        arguments: None,
    };

    let outcome = adapter.dispatch_request(envelope);

    assert!(outcome.responses.is_empty());
    assert!(outcome.events.is_empty());
    assert!(!outcome.should_exit);
    assert!(adapter.launch_state.has_pending_launch());
}

#[test]
fn disconnect_correlates_response_then_emits_one_terminated_event() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    let arguments = serde_json::to_value(DisconnectArguments {
        restart: Some(true),
        terminate_debuggee: Some(false),
    })
    .unwrap();

    let outcome = adapter.dispatch_request(request(160, "disconnect", Some(arguments)));

    assert!(outcome.should_exit);
    assert_eq!(outcome.responses.len(), 1);
    let response = response(&outcome.responses[0]);
    assert!(response.success);
    assert_eq!(response.request_seq, 160);
    let terminated = outcome
        .events
        .iter()
        .map(event)
        .find(|event| event.event == "terminated")
        .expect("terminated event");
    let body: TerminatedEventBody =
        serde_json::from_value(terminated.body.expect("terminated body")).unwrap();
    assert_eq!(body.restart, Some(true));
}

#[test]
fn terminate_correlates_response_then_emits_one_terminated_event() {
    let mut adapter = adapter();
    initialize(&mut adapter);
    let arguments = serde_json::to_value(TerminateArguments {
        restart: Some(false),
    })
    .unwrap();

    let outcome = adapter.dispatch_request(request(170, "terminate", Some(arguments)));

    assert!(outcome.should_exit);
    assert_eq!(outcome.responses.len(), 1);
    let response = response(&outcome.responses[0]);
    assert!(response.success);
    assert_eq!(response.request_seq, 170);
    assert_eq!(
        outcome
            .events
            .iter()
            .map(event)
            .filter(|event| event.event == "terminated")
            .count(),
        1
    );
    let terminated = outcome
        .events
        .iter()
        .map(event)
        .find(|event| event.event == "terminated")
        .unwrap();
    let body: TerminatedEventBody =
        serde_json::from_value(terminated.body.expect("terminated body")).unwrap();
    assert_eq!(body.restart, Some(false));
}
