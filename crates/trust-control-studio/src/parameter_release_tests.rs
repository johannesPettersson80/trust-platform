//! The experiment settings after a project test that does not finish (specification 35, C12, Parameter purpose), against the fake
//! runtime of `identify::test_support`. A test the runtime stops, with its restoration confirmed, leaves the loop's journal clear, so the
//! settings are free and `released` says so. A test whose start cannot be reconciled leaves its experiment and its recording journaled
//! for the reopened panel (A8.5, A8.6), so the settings stay frozen until it is resumed or cancelled.

use std::fs;

use trust_control::failure::NextAction;

use super::{hold_settings, REVIEW_OPERATION};
use crate::busy_loops::LoopClaims;
use crate::workflow::identify::test_support::{
    empty_journal, folder, passes, reopened, run_tank, tank_loop, FakeState, StartBehaviour, Study,
};
use crate::workflow::identify::{IdentifyRequest, RunPurpose};

#[test]
fn a_project_test_the_runtime_stops_with_its_restoration_confirmed_leaves_the_settings_free() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.end_experiments_at_start("failed", "signal_forced");
    let folder = folder("settings-after-a-stop");
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(
        &state,
        &journal,
        &mut study,
        &request,
        RunPurpose::ProjectTest,
    );
    assert!(outcome.is_err(), "a stopped experiment fails the test");
    assert!(
        reopened(&folder).is_empty(),
        "the experiment ended with its restoration confirmed, so nothing of the test stays journaled"
    );
    let claims = LoopClaims::default();
    assert!(
        hold_settings(
            &claims,
            &loop_definition,
            folder.to_str().expect("a UTF-8 folder"),
            REVIEW_OPERATION
        )
        .is_ok(),
        "the settings are released once the test's restoration is confirmed, though the test failed"
    );
    fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn a_project_test_whose_start_cannot_be_reconciled_keeps_the_settings_frozen_until_it_is_resumed_or_cancelled(
) {
    let state = FakeState::new(StartBehaviour::Unreadable, 0);
    let folder = folder("settings-after-an-unresolved-start");
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(
        &state,
        &journal,
        &mut study,
        &request,
        RunPurpose::ProjectTest,
    );
    assert!(outcome.is_err(), "the start cannot be reconciled");
    assert!(
        !reopened(&folder).is_empty(),
        "the experiment of unknown state and its recording stay journaled"
    );
    let claims = LoopClaims::default();
    let Err(frozen) = hold_settings(
        &claims,
        &loop_definition,
        folder.to_str().expect("a UTF-8 folder"),
        REVIEW_OPERATION,
    ) else {
        panic!("an experiment of unknown state freezes the settings");
    };
    assert_eq!(frozen.message, "This loop has an unfinished operation.");
    assert_eq!(frozen.next_action, NextAction::Resume);
    fs::remove_dir_all(&folder).expect("clean up");
}
