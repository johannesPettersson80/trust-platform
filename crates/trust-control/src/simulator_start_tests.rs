//! Specification 35, A2.8: the engine's decision for the page's Start simulator request, and the reply to the lifecycle service's
//! outcome. The extension's promise and the lifecycle service are the extension's; these tests pin what the engine decides.

use serde_json::json;

use super::*;

fn request() -> SimulatorStartRequest {
    SimulatorStartRequest {
        loop_runtime_id: "simulator".to_string(),
        requested_runtime_id: Some("simulator".to_string()),
        requested_runtime_mode: Some("simulate".to_string()),
        runtime_mode: "simulate".to_string(),
        running: false,
        starting: false,
        pending: PendingStart::Idle,
    }
}

fn refusal_of(answer: &SimulatorStartAnswer) -> &SimulatorStartFailure {
    assert_eq!(answer.decision, SimulatorStartDecision::Reply);
    answer
        .reply
        .as_ref()
        .and_then(|reply| reply.failure.as_ref())
        .expect("a refusal carries its failure")
}

#[test]
fn an_idle_simulator_that_is_not_running_is_started_by_the_extension() {
    let answer = decide(&request());
    assert_eq!(answer.decision, SimulatorStartDecision::Start);
    assert_eq!(answer.reply, None);
}

#[test]
fn a_request_for_another_runtime_or_mode_is_refused_as_a_changed_scope() {
    let mut other_runtime = request();
    other_runtime.requested_runtime_id = Some("plc".to_string());
    let refused = decide(&other_runtime);
    assert_eq!(
        refusal_of(&refused).kind,
        SimulatorStartFailureKind::InvalidScope
    );
    assert_eq!(refusal_of(&refused).message, SCOPE_CHANGED);

    let mut online = request();
    online.requested_runtime_mode = Some("online".to_string());
    assert_eq!(
        refusal_of(&decide(&online)).kind,
        SimulatorStartFailureKind::InvalidScope
    );

    let mut unnamed = request();
    unnamed.requested_runtime_id = None;
    assert_eq!(
        refusal_of(&decide(&unnamed)).kind,
        SimulatorStartFailureKind::InvalidScope
    );
}

#[test]
fn a_changed_scope_is_refused_before_a_start_in_flight_is_joined() {
    let mut changed = request();
    changed.requested_runtime_mode = Some("online".to_string());
    changed.pending = PendingStart::SameScope;
    assert_eq!(
        refusal_of(&decide(&changed)).kind,
        SimulatorStartFailureKind::InvalidScope
    );
}

#[test]
fn a_start_for_the_same_scope_joins_the_start_in_flight() {
    let mut joining = request();
    joining.pending = PendingStart::SameScope;
    let answer = decide(&joining);
    assert_eq!(answer.decision, SimulatorStartDecision::Join);
    assert_eq!(answer.reply, None);
}

#[test]
fn a_start_while_another_scope_starts_is_refused_and_asks_to_wait() {
    let mut other = request();
    other.pending = PendingStart::OtherScope;
    let refused = decide(&other);
    assert_eq!(
        refusal_of(&refused).kind,
        SimulatorStartFailureKind::AlreadyStarting
    );
    assert_eq!(refusal_of(&refused).message, ANOTHER_START);
}

#[test]
fn a_runtime_that_is_not_the_simulator_is_refused_before_its_state_is_read() {
    let mut physical = request();
    physical.runtime_mode = "online".to_string();
    physical.running = true;
    let refused = decide(&physical);
    assert_eq!(
        refusal_of(&refused).kind,
        SimulatorStartFailureKind::NotSimulator
    );
    assert_eq!(refusal_of(&refused).message, NOT_SIMULATOR);
}

#[test]
fn a_running_simulator_answers_at_once_that_it_runs() {
    let mut running = request();
    running.running = true;
    let answer = decide(&running);
    assert_eq!(answer.decision, SimulatorStartDecision::Reply);
    let reply = answer.reply.expect("a running simulator answers");
    assert!(reply.ok);
    assert_eq!(reply.message.as_deref(), Some(ALREADY_RUNNING));
    assert_eq!(reply.failure, None);
}

#[test]
fn a_start_in_progress_is_refused_with_its_sentence() {
    let mut starting = request();
    starting.starting = true;
    let refused = decide(&starting);
    assert_eq!(
        refusal_of(&refused).kind,
        SimulatorStartFailureKind::AlreadyStarting
    );
    assert_eq!(refusal_of(&refused).message, IN_PROGRESS);
}

#[test]
fn a_started_simulator_reports_the_lifecycle_sentence() {
    let reply = outcome(&SimulatorStartOutcome {
        ok: true,
        message: Some("Simulator running.".to_string()),
        failure_message: None,
    });
    assert!(reply.ok);
    assert_eq!(reply.message.as_deref(), Some("Simulator running."));
    assert_eq!(reply.failure, None);
}

#[test]
fn a_started_simulator_without_a_sentence_says_that_it_started() {
    let reply = outcome(&SimulatorStartOutcome {
        ok: true,
        message: Some(String::new()),
        failure_message: None,
    });
    assert_eq!(reply.message.as_deref(), Some(STARTED));
}

#[test]
fn a_failed_start_is_start_failed_with_the_lifecycle_sentence() {
    let reply = outcome(&SimulatorStartOutcome {
        ok: false,
        message: None,
        failure_message: Some("Compile failed; inspect Problems.".to_string()),
    });
    assert!(!reply.ok);
    assert_eq!(reply.message, None);
    let failure = reply.failure.expect("a failed start carries its failure");
    assert_eq!(failure.kind, SimulatorStartFailureKind::StartFailed);
    assert_eq!(failure.message, "Compile failed; inspect Problems.");
}

#[test]
fn a_failed_start_without_a_sentence_says_that_the_simulator_did_not_start() {
    let reply = outcome(&SimulatorStartOutcome {
        ok: false,
        message: None,
        failure_message: None,
    });
    assert_eq!(reply.failure.expect("a failure").message, NOT_STARTED);
}

#[test]
fn the_request_and_the_answer_keep_the_names_the_extension_sends_and_reads() {
    let request: SimulatorStartRequest = serde_json::from_value(json!({
        "loopRuntimeId": "simulator", "requestedRuntimeId": "simulator", "requestedRuntimeMode": "simulate",
        "runtimeMode": "simulate", "running": false, "starting": false, "pending": "sameScope"
    }))
    .expect("the extension's request decodes");
    assert_eq!(request.pending, PendingStart::SameScope);
    let json = serde_json::to_value(decide(&request)).expect("the answer serializes");
    assert_eq!(json, json!({ "decision": "join" }));
    let refused = serde_json::to_value(decide(&SimulatorStartRequest {
        pending: PendingStart::Idle,
        requested_runtime_mode: Some("online".to_string()),
        ..request
    }))
    .expect("the refusal serializes");
    assert_eq!(refused["decision"], "reply");
    assert_eq!(refused["reply"]["ok"], false);
    assert_eq!(refused["reply"]["failure"]["kind"], "invalid_scope");
}
