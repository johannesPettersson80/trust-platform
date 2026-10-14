//! Tests of the parameter client against the scripted fake runtime (annex E7, E11.4, E11.7). The apply is confirmed by the runtime's record
//! with the identity of the review, a program change during confirmation is refused, a cancel before dispatch is acknowledged by the owner's
//! identity, a retained operation that the record confirms is reconciled without a second write, and a retained operation whose program
//! changed stays unresolved. The loop binding of the tests is the proportional loop's (`parameterApplication.ts`, E7.1).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use trust_control::commissioning::parameter_application::{
    CancelOutcome, ParameterOwner, RuntimeBinding, RuntimeMode, RuntimeParameter,
};
use trust_control::commissioning::parameter_status::RecordState;
use trust_control::failure::NextAction;
use trust_control::loop_definition::{ParameterBinding, RealType};

use super::*;
use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};

/// The control token of the fake runtime; it is not a credential.
const TOKEN: &str = "control-studio-fake-token";

/// The binding of the tests: one gain, `Main.Parameters.Kp`, and the mode `Main.Auto`, which reads false in Manual.
fn binding() -> RuntimeBinding {
    RuntimeBinding {
        loop_id: "loop:tank".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters: vec![RuntimeParameter {
            symbol: "Main.Parameters.Kp".to_owned(),
            iec_type: RealType::Lreal,
            unit: "%/%".to_owned(),
            convention: "proportional gain".to_owned(),
            minimum: -100.0,
            maximum: 100.0,
        }],
        mode: RuntimeMode {
            symbol: "Main.Auto".to_owned(),
            manual_value: false,
        },
    }
}

/// The parameter record (annex E7.2) of the tests: the gain in force at `active`, the revision retained before it, and the last transaction.
fn record(
    program: &str,
    active: &str,
    gain: f64,
    previous: Option<(&str, f64)>,
    status: &str,
    transaction: Option<&str>,
) -> Value {
    json!({
        "supports_request_id": true,
        "transaction_id": transaction,
        "program_revision": program,
        "binding_revision": "binding:1",
        "active_revision": active,
        "values": { "Main.Parameters.Kp": gain },
        "status": status,
        "previous_revision": previous.map(|(revision, _)| revision),
        "previous_values": previous.map(|(_, value)| json!({ "Main.Parameters.Kp": value })),
        "reason": null
    })
}

/// The values of the tests' review: the gain 0.5.
fn gain_of_half() -> BTreeMap<String, f64> {
    BTreeMap::from([("Main.Parameters.Kp".to_owned(), 0.5)])
}

/// Answers the parameter requests of the tests. Before the runtime accepts an apply, the record holds the gain 1 at `active:1` under
/// `program:1`. Once `accepted` names the apply's identifier, the record holds the gain 0.5 at `active:2` under `after`, with the identifier.
fn handle(request: &Value, accepted: &Mutex<Option<String>>, after: &str) -> Option<Value> {
    match request["type"].as_str()? {
        "status" => Some(answer(request, json!({ "tasks": [] }))),
        "control.parameters.status" => {
            let accepted = accepted.lock().expect("the accepted identifier");
            let reply = match accepted.as_deref() {
                None => record("program:1", "active:1", 1.0, None, "idle", None),
                Some(id) => record(
                    after,
                    "active:2",
                    0.5,
                    Some(("active:1", 1.0)),
                    "applied",
                    Some(id),
                ),
            };
            Some(answer(request, reply))
        }
        "control.parameters.apply" => {
            *accepted.lock().expect("the accepted identifier") = request["params"]
                ["transaction_id"]
                .as_str()
                .map(str::to_owned);
            Some(answer(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        }
        _ => Some(refusal(
            request,
            "invalid_parameter_request",
            "An unexpected request.",
        )),
    }
}

#[test]
fn a_parameter_apply_is_confirmed_by_the_runtime_record_under_the_reviewed_identity() {
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| handle(request, &served, "program:1"));
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let review = transactions
        .review(&gain_of_half())
        .expect("the value is reviewed against the record");

    let confirmed = transactions
        .apply(&review, false)
        .expect("the apply is confirmed by the record");
    assert_eq!(confirmed.status, RecordState::Applied);
    assert_eq!(confirmed.active_revision, "active:2");

    let requests = fake.requests_of("control.parameters.apply");
    assert_eq!(requests.len(), 1, "the apply is sent once");
    let sent = &requests[0]["params"];
    assert_eq!(sent["transaction_id"], review.transaction_id.as_str());
    assert_eq!(sent["expected_program_revision"], "program:1");
    assert_eq!(sent["expected_active_revision"], "active:1");
    assert_eq!(sent["expected_values"]["Main.Parameters.Kp"], 1.0);
    assert_eq!(sent["values"]["Main.Parameters.Kp"], 0.5);
    assert_eq!(sent["source"], "simulation");
    assert_eq!(sent["physical_approved"], false);
}

#[test]
fn a_program_change_during_confirmation_is_refused() {
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| handle(request, &served, "program:2"));
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let review = transactions
        .review(&gain_of_half())
        .expect("the value is reviewed");

    let failure = transactions
        .apply(&review, false)
        .expect_err("the program changed while the apply was confirmed");
    assert_eq!(
        failure.message,
        "Runtime program or parameter binding changed during application."
    );
    assert_eq!(failure.next_action, NextAction::Validate);
}

#[test]
fn a_cancel_before_dispatch_is_acknowledged_by_the_owners_identity() {
    let fake = FakeRuntime::start(|request| match request["type"].as_str() {
        Some("control.parameters.cancel") => Some(answer(
            request,
            json!({
                "transaction_id": request["params"]["transaction_id"],
                "program_revision": "program:1",
                "binding_revision": "binding:1",
                "confirmed": true,
                "before_dispatch": true
            }),
        )),
        _ => Some(refusal(
            request,
            "invalid_parameter_request",
            "An unexpected request.",
        )),
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let owner = ParameterOwner {
        id: "transaction-owned".to_owned(),
        program: "program:1".to_owned(),
        binding: "binding:1".to_owned(),
    };

    let outcome = transactions
        .cancel_owner(&owner)
        .expect("the cancel is acknowledged");
    assert_eq!(outcome, CancelOutcome::BeforeDispatch);
    let cancels = fake.requests_of("control.parameters.cancel");
    assert_eq!(cancels.len(), 1);
    assert_eq!(cancels[0]["params"]["transaction_id"], "transaction-owned");
    assert_eq!(cancels[0]["params"]["loop_id"], "loop:tank");
    assert_eq!(
        fake.requests_of("control.parameters.apply"),
        [] as [serde_json::Value; 0]
    );
}

#[test]
fn a_retained_apply_the_record_confirms_is_reconciled_without_a_second_write() {
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| handle(request, &served, "program:1"));
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let review = transactions
        .review(&gain_of_half())
        .expect("the value is reviewed");
    // The retained apply reached the runtime: its record now carries the review's identity.
    *accepted.lock().expect("the accepted identifier") = Some(review.transaction_id.clone());

    let reconciliation = reconcile_parameter_operation(&mut transactions, &review, None)
        .expect("the record confirms the retained operation");
    match reconciliation {
        ParameterReconciliation::Confirmed { observed, reason } => {
            assert_eq!(observed.active_revision, "active:2");
            assert_eq!(
                reason,
                "Retained apply is confirmed by the current runtime status."
            );
        }
        other => panic!("expected a confirmation, got {other:?}"),
    }
    assert!(
        fake.requests_of("control.parameters.apply").is_empty(),
        "a retained operation is never written again"
    );
}

#[test]
fn a_retained_operation_whose_program_changed_stays_unresolved() {
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| handle(request, &served, "program:2"));
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let review = transactions
        .review(&gain_of_half())
        .expect("the value is reviewed");
    *accepted.lock().expect("the accepted identifier") = Some(review.transaction_id.clone());

    let failure = reconcile_parameter_operation(&mut transactions, &review, None)
        .expect_err("the program changed since the operation was issued");
    assert_eq!(
        failure.message,
        "The program or parameter binding changed since the operation was issued."
    );
    assert_eq!(
        fake.requests_of("control.parameters.apply"),
        [] as [serde_json::Value; 0]
    );
}

#[test]
fn the_loop_binding_becomes_the_runtime_binding_with_every_field_kept() {
    let declared: ParameterBinding = serde_json::from_value(json!({
        "loop_id": "loop:tank",
        "task_name": "MainTask",
        "parameters": [{
            "symbol": "Main.Parameters.Kp",
            "iec_type": "LREAL",
            "unit": "%/%",
            "convention": "proportional gain",
            "minimum": -100.0,
            "maximum": 100.0
        }],
        "mode": { "symbol": "Main.Auto", "manual_value": false }
    }))
    .expect("the declared binding decodes");
    assert_eq!(runtime_binding(&declared), binding());
}

#[test]
fn a_rollback_restores_the_retained_values_under_the_retained_revision() {
    // Ported from `control-studio-parameter-application.test.ts`: the rollback writes the retained values of `active:1` over `active:2`,
    // carries no values of its own, and is confirmed by the record that holds the retained values again.
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| match request["type"].as_str() {
        Some("status") => Some(answer(request, json!({ "tasks": [] }))),
        Some("control.parameters.status") => {
            let accepted = served.lock().expect("the accepted identifier");
            let reply = match accepted.as_deref() {
                None => record(
                    "program:1",
                    "active:2",
                    2.0,
                    Some(("active:1", 1.0)),
                    "applied",
                    None,
                ),
                Some(id) => record("program:1", "active:1", 1.0, None, "applied", Some(id)),
            };
            Some(answer(request, reply))
        }
        Some("control.parameters.rollback") => {
            *served.lock().expect("the accepted identifier") = request["params"]["transaction_id"]
                .as_str()
                .map(str::to_owned);
            Some(answer(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        }
        _ => Some(refusal(
            request,
            "invalid_parameter_request",
            "An unexpected request.",
        )),
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut transactions =
        ParameterTransactions::new(&mut client, binding(), ApplySource::Simulation);
    let review = transactions
        .review_rollback()
        .expect("the retained previous values are reviewed");
    assert_eq!(
        review.values,
        BTreeMap::from([("Main.Parameters.Kp".to_owned(), 1.0)])
    );

    let restored = transactions
        .rollback(&review, false)
        .expect("the rollback is confirmed by the record");
    assert_eq!(
        restored.values,
        BTreeMap::from([("Main.Parameters.Kp".to_owned(), 1.0)])
    );

    let requests = fake.requests_of("control.parameters.rollback");
    assert_eq!(requests.len(), 1);
    let sent = &requests[0]["params"];
    assert_eq!(sent["previous_revision"], "active:1");
    assert_eq!(sent["expected_active_revision"], "active:2");
    assert_eq!(sent["expected_values"]["Main.Parameters.Kp"], 2.0);
    assert!(
        sent.get("values").is_none(),
        "a rollback carries no values of its own"
    );
}
