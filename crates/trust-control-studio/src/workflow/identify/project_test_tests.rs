//! The end of a project test whose fit its recording cannot support (C12, B3.5), on the fake runtime of `identify::test_support`: the
//! test finishes with its recording, its samples and its restoration, and the fit's whole refusal as its note, and it keeps no model.
//! The same campaign run as an Identify is still refused by its fit.

use std::fs;

use trust_control::failure::Failure;
use trust_control::messages::CampaignRestoration;

use super::test_support::{
    assert_no_model_of_a_held_input, empty_journal, folder, passes, run_tank, tank_loop,
    whole_text, FakeState, StartBehaviour, Study, Verdict,
};
use super::{IdentifyOutcome, IdentifyRequest, IdentifyResult, RunPurpose};

/// Runs the tank's campaign as `purpose` on a fake runtime whose measurement ramps while its step is held, so the fit of the held
/// record refuses (`long_campaign_tests`, test 14). The journal is the folder `name`, which is removed once the campaign has ended.
fn held_campaign(name: &str, purpose: RunPurpose) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.ramp_while_stepped(0.01);
    let folder = folder(name);
    let journal = empty_journal(&folder);
    let mut study = Study::new(&state);
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let outcome = run_tank(&state, &journal, &mut study, &request, purpose);
    let _ = fs::remove_dir_all(&folder);
    outcome
}

/// A project test whose fit refuses finishes: its recording is kept, its samples and its restoration are answered, and the refusal
/// is its note, whole. No model is fitted, so the outcome holds no Identify result (C12, B3.5).
#[test]
fn a_project_test_whose_fit_refuses_finishes_with_its_note_and_keeps_no_model() {
    let Err(refused) = held_campaign("project-test-refused-identify", RunPurpose::Identify) else {
        panic!("the fit of the held record refuses");
    };
    assert_no_model_of_a_held_input(&refused);
    let Ok(outcome) = held_campaign("project-test-finished", RunPurpose::ProjectTest) else {
        panic!("a project test whose fit refuses still finishes");
    };
    assert!(
        matches!(outcome.result, IdentifyResult::Recorded(_)),
        "the record is kept without a fit"
    );
    assert_eq!(
        outcome.fit_refusal.as_deref(),
        Some(whole_text(&refused)),
        "the note is the fit's whole refusal"
    );
    assert_eq!(
        outcome.restoration,
        Some(CampaignRestoration::Restored),
        "the restoration the experiment confirmed"
    );
    assert!(
        !outcome.recorded_samples.is_empty(),
        "the samples of the kept recording"
    );
    assert!(!outcome.recording_id.is_empty(), "the kept recording");
}
