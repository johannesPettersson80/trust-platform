//! Tests of the transfer client against the scripted fake runtime (annex E6, E11.2 to E11.4). Each test scripts the runtime's answers and
//! reads the requests that it received: the owner is retained before the apply is sent, a lost acknowledgement is settled by the same
//! identity and is never sent again, a refused request releases its owner, a cancel before dispatch releases the owner, a rejected and a
//! reverted transfer report the runtime's reason, and a physical apply without approval is never sent.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::commissioning_result::{TransferOwner, TransferState};
use trust_control::commissioning::transfer::{
    TransferBinding, TransferMember, TransferModeBinding, TransferValueType,
};
use trust_control::failure::{Cause, Failure, NextAction};

use super::*;
use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};

/// The control token of the fake runtime; it is not a credential.
const TOKEN: &str = "control-studio-fake-token";

fn member(symbol: &str) -> TransferMember {
    TransferMember {
        symbol: symbol.to_owned(),
        iec_type: TransferValueType::Lreal,
        unit: "%".to_owned(),
        minimum: 0.0,
        maximum: 100.0,
    }
}

/// The binding of the tests: the Manual mode, the reference and the Manual command as states, the process value as a guard, and the held
/// output.
fn binding() -> TransferBinding {
    TransferBinding {
        loop_id: "loop:tank".to_owned(),
        task_name: "MainTask".to_owned(),
        mode: TransferModeBinding {
            symbol: "P1.Manual".to_owned(),
            manual_value: true,
        },
        additional_modes: Vec::new(),
        tracking_outputs: std::collections::BTreeMap::new(),
        states: vec![member("P1.Ref"), member("P1.ManualCommand")],
        guards: vec![member("P1.PV")],
        output: member("P1.MV"),
        additional_outputs: Vec::new(),
    }
}

/// The write of the tests: both states, in Manual, with the output change allowed over the whole span.
fn write(source: ApplySource) -> TransferWrite {
    TransferWrite {
        values: std::collections::BTreeMap::from([
            ("P1.Ref".to_owned(), 0.0),
            ("P1.ManualCommand".to_owned(), 40.0),
        ]),
        manual: true,
        tolerance: 100.0,
        entry_error_gain: None,
        tracking: None,
        reviewed: None,
        source,
        physical_approved: false,
    }
}

/// The values of every member and mode of the binding, the mode as `manual` reads it.
fn values_json(manual: bool) -> Value {
    json!({ "P1.Manual": manual, "P1.Ref": 0.0, "P1.ManualCommand": 40.0, "P1.PV": 10.0, "P1.MV": 40.0 })
}

/// A transaction record (annex E6.8) of `id`, in `state`, with the given confirmation, restoration and reason.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn transaction_json(
    id: &str,
    state: &str,
    confirmation: Value,
    restored: bool,
    reason: Value,
) -> Value {
    json!({
        "id": id,
        "program_revision": "program",
        "binding_revision": "binding",
        "status": state,
        "reason": reason,
        "observed_values": values_json(true),
        "confirmation": confirmation,
        "initial_values": values_json(true),
        "applied_values": { "P1.Ref": 0.0, "P1.ManualCommand": 40.0 },
        "restored": restored
    })
}

/// The transaction of an apply that the output image confirmed.
fn applied_outcome(id: &str) -> Value {
    transaction_json(id, "applied", json!("output-image"), false, Value::Null)
}

/// The status of a loop with no transaction, in Auto (annex E6.3).
fn idle_status() -> Value {
    json!({
        "program_revision": "program",
        "binding_revision": "binding",
        "values": values_json(false),
        "status": "idle",
        "reason": null,
        "transaction": null
    })
}

/// The status of a loop whose last transaction is `transaction`: the status and reason repeat the transaction's.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn ended_status(transaction: Value) -> Value {
    json!({
        "program_revision": "program",
        "binding_revision": "binding",
        "values": values_json(true),
        "status": transaction["status"],
        "reason": transaction["reason"],
        "transaction": transaction
    })
}

/// Answers the requests of a transfer on the fake runtime. `accepted` holds the identifier of the apply the runtime accepted: once one is
/// accepted, the status reports its outcome, given by `outcome`. `apply` gives the answer to the apply request.
fn handle(
    request: &Value,
    accepted: &Mutex<Option<String>>,
    outcome: &dyn Fn(&str) -> Value,
    apply: &dyn Fn(&Value) -> Option<Value>,
) -> Option<Value> {
    match request["type"].as_str()? {
        "status" => Some(answer(request, json!({ "tasks": [] }))),
        "control.transfer.status" => {
            let accepted = accepted.lock().expect("the accepted identifier");
            let status = match accepted.as_deref() {
                None => idle_status(),
                Some(id) => ended_status(outcome(id)),
            };
            Some(answer(request, status))
        }
        "control.transfer.apply" => {
            let id = request["params"]["transaction_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            *accepted.lock().expect("the accepted identifier") = Some(id);
            apply(request)
        }
        _ => Some(refusal(
            request,
            "invalid_transfer_request",
            "An unexpected request.",
        )),
    }
}

/// The owner sink of the tests: it records each owner it retains, and the order of the retentions in `events`.
struct Sink {
    owners: Vec<TransferOwner>,
    events: Arc<Mutex<Vec<String>>>,
}

impl TransferOwnerSink for Sink {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.events
            .lock()
            .expect("the events")
            .push("retain".to_owned());
        self.owners.push(owner.clone());
        Ok(())
    }
}

fn sink(events: &Arc<Mutex<Vec<String>>>) -> Sink {
    Sink {
        owners: Vec::new(),
        events: Arc::clone(events),
    }
}

#[test]
fn the_owner_is_retained_before_the_apply_is_sent_and_its_outcome_is_confirmed() {
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let log = Arc::clone(&events);
    let fake = FakeRuntime::start(move |request| {
        handle(request, &served, &applied_outcome, &|request| {
            log.lock().expect("the events").push("apply".to_owned());
            Some(answer(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        })
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    let mut retained = sink(&events);
    let outcome = session
        .apply(&write(ApplySource::Simulation), &mut retained)
        .expect("the transfer is applied");

    assert_eq!(outcome.status, TransferState::Applied);
    assert_eq!(
        *events.lock().expect("the events"),
        vec!["retain".to_owned(), "apply".to_owned()],
        "the owner is retained before the apply is dispatched (E11.2)"
    );
    let requests = fake.requests_of("control.transfer.apply");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]["params"]["transaction_id"],
        retained.owners[0].id.as_str(),
        "the apply carries the retained identity"
    );
    assert_eq!(requests[0]["params"]["source"], "simulation");
    assert_eq!(requests[0]["params"]["physical_approved"], false);
    assert!(
        session.owned().is_none(),
        "a confirmed transfer releases its owner"
    );
    assert_eq!(session.outcomes().len(), 1);
}

#[test]
fn a_lost_acknowledgement_is_settled_by_the_same_identity_and_the_apply_is_never_sent_again() {
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    // An answer that is not a control response is an answer the client cannot read: the runtime may have accepted the apply.
    let fake = FakeRuntime::start(move |request| {
        handle(request, &served, &applied_outcome, &|_| {
            Some(json!({ "lost": true }))
        })
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    let mut retained = sink(&events);
    let outcome = session
        .apply(&write(ApplySource::Simulation), &mut retained)
        .expect("the same identity settles the unread acknowledgement");

    assert_eq!(outcome.status, TransferState::Applied);
    assert_eq!(
        fake.requests_of("control.transfer.apply").len(),
        1,
        "the apply is not repeated (E11.4)"
    );
    assert_eq!(
        fake.requests_of("control.transfer.status").len(),
        3,
        "the reviewed status, the identity read after the lost answer, and the confirmation read"
    );
}

#[test]
fn a_request_the_runtime_refused_before_accepting_it_releases_its_owner_and_a_physical_apply_without_approval_is_never_sent(
) {
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let fake = FakeRuntime::start(move |request| {
        handle(request, &served, &applied_outcome, &|request| {
            Some(refusal(
                request,
                "invalid_transfer_request",
                "The transfer request breaks check 9.",
            ))
        })
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    let mut retained = sink(&events);
    let failure = session
        .apply(&write(ApplySource::Simulation), &mut retained)
        .expect_err("the runtime refuses the request");
    assert_eq!(failure.message, "The transfer request breaks check 9.");
    assert!(
        session.owned().is_none(),
        "a refused request was never accepted, so nothing is owned"
    );
    assert_eq!(fake.requests_of("control.transfer.apply").len(), 1);

    let failure = session
        .apply(&write(ApplySource::Physical), &mut retained)
        .expect_err("a physical transfer needs approval");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Approval, NextAction::Approve)
    );
    assert_eq!(
        fake.requests_of("control.transfer.apply").len(),
        1,
        "a physical apply without approval is never sent"
    );
}

#[test]
fn a_cancel_before_dispatch_releases_the_owner_and_sends_no_apply() {
    let fake = FakeRuntime::start(|request| match request["type"].as_str() {
        Some("control.transfer.cancel") => Some(answer(
            request,
            json!({
                "transaction_id": request["params"]["transaction_id"],
                "program_revision": "program",
                "binding_revision": "binding",
                "status": "cancelled",
                "confirmed": true,
                "before_dispatch": true,
                "reason": null
            }),
        )),
        _ => Some(refusal(
            request,
            "invalid_transfer_request",
            "An unexpected request.",
        )),
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    session
        .restore_owner(TransferOwner {
            id: "transaction-owned".to_owned(),
            program: "program".to_owned(),
            binding: "binding".to_owned(),
        })
        .expect("the journal's owner is taken over");
    session.cancel().expect("the cancel is acknowledged");

    assert!(session.owned().is_none());
    let cancels = fake.requests_of("control.transfer.cancel");
    assert_eq!(cancels.len(), 1);
    assert_eq!(cancels[0]["params"]["transaction_id"], "transaction-owned");
    assert_eq!(cancels[0]["params"]["expected_program_revision"], "program");
    assert_eq!(cancels[0]["params"]["loop_id"], "loop:tank");
    assert_eq!(
        fake.requests_of("control.transfer.apply"),
        [] as [serde_json::Value; 0]
    );
}

#[test]
fn a_rejected_transfer_is_reported_with_the_runtime_reason() {
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let rejected = |id: &str| {
        transaction_json(
            id,
            "rejected",
            Value::Null,
            false,
            json!("stale program, binding, or expected values"),
        )
    };
    let fake = FakeRuntime::start(move |request| {
        handle(request, &served, &rejected, &|request| {
            Some(answer(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        })
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    let mut retained = sink(&events);
    let failure = session
        .apply(&write(ApplySource::Simulation), &mut retained)
        .expect_err("the runtime rejected the transfer");
    assert_eq!(
        failure.message,
        "stale program, binding, or expected values"
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );
    assert!(
        session.owned().is_none(),
        "the terminal outcome releases the owner"
    );
}

#[test]
fn a_transfer_the_runtime_reverted_asks_to_validate_and_keeps_the_runtime_reason() {
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let accepted = Arc::new(Mutex::new(None::<String>));
    let served = Arc::clone(&accepted);
    let reverted = |id: &str| {
        transaction_json(
            id,
            "failed",
            Value::Null,
            true,
            json!("first controller output P1.MV violates reviewed transfer tolerance"),
        )
    };
    let fake = FakeRuntime::start(move |request| {
        handle(request, &served, &reverted, &|request| {
            Some(answer(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        })
    });
    let mut client = RuntimeClient::connect(&fake.connection(Some(TOKEN))).expect("connected");
    let mut session = TransferSession::new(&mut client, binding(), "program".to_owned());
    let mut retained = sink(&events);
    let failure = session
        .apply(&write(ApplySource::Simulation), &mut retained)
        .expect_err("the runtime reverted the transfer");
    assert_eq!(
        failure.message,
        "first controller output P1.MV violates reviewed transfer tolerance."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );
    assert!(
        failure
            .detail
            .as_deref()
            .is_some_and(|detail| detail.ends_with("so the loop kept its mode and values.")),
        "the revert is said in the whole text"
    );
}
