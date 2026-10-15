//! Tests of the manual identification section (B3.3, B3.6): the section keeps the identifiers of its recordings and their settings,
//! its document round-trips, its rules refuse an identifier that names no recording, and the saved state it is taken from keeps no
//! samples.

use super::*;
use crate::identification::review::IdentificationReviewInput;
use crate::identification::state::SavedIdentificationState;

/// A recording identifier of the form the study keeps (`rec-` and 32 hexadecimal digits).
const RECORDING: &str = "rec-00000000000000000000000000000001";

/// A recording the section names, checked as a first-order model without held-step settings or exclusions.
fn recording(id: &str) -> ManualRecording {
    ManualRecording {
        recording_id: id.to_string(),
        requested_kind: IdentificationModelSelection::FirstOrder,
        step_response_settings: None,
        first_order_step_response_settings: None,
        excluded: Vec::new(),
    }
}

/// A section at `phase` whose current recording is `current`.
fn section(
    phase: IdentificationPhase,
    current: Option<ManualRecording>,
) -> ManualIdentificationSection {
    ManualIdentificationSection {
        schema_version: MANUAL_SCHEMA_VERSION,
        maximum_normalized_error: 0.15,
        phase,
        estimation: None,
        current,
        form: None,
        structure_choice: None,
    }
}

#[test]
fn a_section_round_trips_as_its_study_document() {
    let value = section(IdentificationPhase::Qualified, Some(recording(RECORDING)));
    let text = value.encode().expect("the section encodes");
    let decoded = ManualIdentificationSection::decode(&text).expect("the section decodes");
    assert_eq!(decoded, value);
}

#[test]
fn a_section_refuses_an_identifier_that_names_no_recording_and_names_the_recordings_it_holds() {
    let refused = section(IdentificationPhase::Qualified, Some(recording("rec-1")));
    assert!(refused.check().is_err());
    let held = section(IdentificationPhase::Qualified, Some(recording(RECORDING)));
    assert!(held.check().is_ok());
    assert_eq!(held.recording_ids(), vec![RECORDING]);
}

#[test]
fn the_section_of_a_saved_state_keeps_the_identifiers_and_settings_and_no_samples() {
    let input = save_identification_input(&IdentificationReviewInput {
        recording: None,
        step_response_settings: None,
        first_order_step_response_settings: None,
        maximum_normalized_error: Some(0.15),
        estimation: Vec::new(),
        validation: None,
        estimation_recording_id: Some(RECORDING.to_string()),
        validation_recording_id: None,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::FirstOrder,
        declared_process: None,
        closed_loop: None,
    });
    let saved = SavedIdentificationState {
        schema_version: 1,
        maximum_normalized_error: 0.15,
        phase: IdentificationPhase::Qualified,
        current: Some(input),
        estimation: None,
        form: None,
        structure_choice: None,
    };
    let kept = ManualIdentificationSection::from_saved(&saved, None).expect("the section");
    assert_eq!(kept.current, Some(recording(RECORDING)));
    assert_eq!(kept.phase, IdentificationPhase::Qualified);
    assert_eq!(kept.estimation, None);
}

#[test]
fn a_recording_file_source_maps_to_the_plant_the_identification_names() {
    assert_eq!(
        data_source_of(EvidenceSource::PhysicalPlant),
        DataSource::Physical
    );
    assert_eq!(
        data_source_of(EvidenceSource::SimulatedPlant),
        DataSource::Simulation
    );
}
