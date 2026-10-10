//! Port of the Test-guard tests of `control-studio-study-store.test.ts`: the journal before dispatch, the unconfirmed
//! starts, the pinned target and the Test journal's forwarding of the owner.

use std::cell::RefCell;

use trust_control::experiment::{ControlEvidenceSource, RecordingRecoveryOwner};

use super::*;

fn owner() -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".into(),
        project_id: "project".into(),
        runtime_id: "runtime".into(),
        loop_id: "loop".into(),
        loop_revision: "sha256:loop".into(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: Vec::new(),
    }
}

#[allow(clippy::unnecessary_wraps)] // It stands in for the journal's persist callback, which returns a Result.
fn persist_nothing(_owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
    Ok(())
}

#[allow(clippy::unnecessary_wraps)] // It stands in for the journal's clear callback, which returns a Result.
fn clear_nothing() -> Result<(), Failure> {
    Ok(())
}

#[test]
fn a_test_identity_is_journaled_before_dispatch_and_a_failed_journal_never_dispatches() {
    let order = RefCell::new(Vec::new());
    let started = journal_before_dispatch(
        || {
            order.borrow_mut().push("journal");
            Ok(())
        },
        || {
            order.borrow_mut().push("dispatch");
            Ok(())
        },
    );
    assert!(started.is_ok());
    assert_eq!(
        *order.borrow(),
        ["journal", "dispatch"],
        "the identity is written before anything is sent"
    );

    let sent = RefCell::new(0);
    let refused = journal_before_dispatch(
        || {
            Err(Failure::new(
                "journal full",
                Cause::Implementation,
                NextAction::None,
            ))
        },
        || {
            *sent.borrow_mut() += 1;
            Ok(())
        },
    );
    assert!(refused.is_err());
    assert_eq!(
        *sent.borrow(),
        0,
        "nothing is dispatched when the identity cannot be journalled"
    );
}

#[test]
fn an_identity_whose_start_was_not_confirmed_is_never_sent_again() {
    let starts = UnconfirmedTestStarts::new();
    starts.mark_unconfirmed("experiment-1");
    let refused = starts
        .assert_sendable("experiment-1")
        .expect_err("not sent again");
    assert!(refused.message.contains("not confirmed"), "{refused:?}");
    assert!(starts.assert_sendable("experiment-2").is_ok());
    starts.confirm("experiment-1");
    assert!(starts.assert_sendable("experiment-1").is_ok());
}

#[test]
fn the_test_page_starts_only_with_its_pinned_target_and_its_test_journal() {
    let journal = experiment_recording_journal(persist_nothing, clear_nothing);
    let target = ("sim", "tcp://127.0.0.1:4000");

    let Err(unpinned) = test_start_ports(None::<(&str, &str)>, Some(&journal)) else {
        panic!("without the target pinned at open, no start is made");
    };
    assert_eq!(
        unpinned.detail.as_deref(),
        Some(UNPINNED_RUNTIME_MESSAGE),
        "{unpinned:?}"
    );

    let unjournalled = test_start_ports(Some(target), None::<()>)
        .expect_err("without a journal, no start is made");
    assert!(unjournalled.message.contains("journal"), "{unjournalled:?}");

    let ports = test_start_ports(Some(target), Some(&journal)).expect("the ports");
    assert_eq!(ports.target, target, "the start uses the pinned target");
}

#[test]
fn the_test_journal_forwards_the_recording_owner_to_persist_and_clears_only_the_owner() {
    let events = RefCell::new(Vec::new());
    let journal = experiment_recording_journal(
        |_owner: &RecordingRecoveryOwner| {
            events.borrow_mut().push("persist");
            Ok(())
        },
        || {
            events.borrow_mut().push("clear");
            Ok(())
        },
    );
    journal.persist(&owner()).expect("persisted");
    journal.clear(&owner()).expect("cleared");
    assert_eq!(*events.borrow(), ["persist", "clear"]);
}
