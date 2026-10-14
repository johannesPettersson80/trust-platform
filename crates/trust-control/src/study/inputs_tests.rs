//! Tests of the inputs a study records (specification 35, A8.4, B4.2, B4.3 and B8).

use super::*;
use crate::failure::{Cause, NextAction};
use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::study::StudySection;

fn requirements() -> TuningRequirements {
    TuningRequirements {
        reference_change: 2.0,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn settings(duration_seconds: f64) -> AcquisitionSettings {
    AcquisitionSettings {
        method: AcquisitionMethod::Automatic,
        duration_seconds,
        amplitude: 0.0,
        setpoint_signal_id: None,
        relay_direction: None,
        reuse_recordings: false,
        input_range: None,
        output_range: None,
        input_ranges: None,
        output_ranges: None,
    }
}

#[test]
fn an_empty_inputs_section_is_valid_and_is_written_as_an_empty_document() {
    let empty = InputsSection::default();
    assert!(empty.check().is_ok(), "every input is optional");
    let text = empty.encode().expect("encoded");
    assert_eq!(text, "{}", "an absent input is not written");
    assert_eq!(InputsSection::decode(&text).expect("decoded"), empty);
    assert!(!empty.needs_manual_settings());
}

#[test]
fn a_section_round_trips_through_its_document_with_every_input() {
    let section = InputsSection {
        acquisition: Some(settings(30.0)),
        requirements: Some(requirements()),
        method: Some(TuneMethod::Simc),
        choice: Some(TuneChoice::Faster),
        structure: None,
        pins: None,
        settings: None,
    };
    let text = section.encode().expect("encoded");
    assert_eq!(
        InputsSection::decode(&text).expect("decoded"),
        section,
        "the recorded inputs come back as they were written"
    );
    assert_eq!(
        section.encode().expect("again"),
        text,
        "the same inputs give the same text"
    );
}

#[test]
fn a_document_with_an_unknown_field_is_refused() {
    assert!(InputsSection::decode(r#"{"bogus":1}"#).is_err());
}

#[test]
fn the_requirements_are_checked_as_the_tune_section_checks_them() {
    let mut bad = requirements();
    bad.settling_band = 0.4;
    let refused = InputsSection {
        requirements: Some(bad),
        ..InputsSection::default()
    }
    .check()
    .expect_err("a settling band not below the error limit");
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
}

#[test]
fn an_acquisition_setting_outside_the_identification_rules_is_refused() {
    let short = InputsSection {
        acquisition: Some(settings(0.5)),
        ..InputsSection::default()
    };
    assert!(short.check().is_err(), "a duration under one second");
    let valid = InputsSection {
        acquisition: Some(settings(30.0)),
        ..InputsSection::default()
    };
    assert!(valid.check().is_ok());
}

#[test]
fn only_the_manual_method_without_its_settings_needs_them() {
    let manual_without = InputsSection {
        method: Some(TuneMethod::Manual),
        ..InputsSection::default()
    };
    assert!(manual_without.needs_manual_settings());
    let manual_with = InputsSection {
        method: Some(TuneMethod::Manual),
        settings: Some(BTreeMap::from([("ProportionalGain".to_owned(), 2.5)])),
        ..InputsSection::default()
    };
    assert!(!manual_with.needs_manual_settings());
    let formula = InputsSection {
        method: Some(TuneMethod::Simc),
        ..InputsSection::default()
    };
    assert!(
        !formula.needs_manual_settings(),
        "a formula method takes no entered settings"
    );
    let stale = InputsSection {
        method: Some(TuneMethod::Simc),
        settings: Some(BTreeMap::from([("ProportionalGain".to_owned(), 2.5)])),
        ..InputsSection::default()
    };
    assert_eq!(
        stale.manual_settings(),
        None,
        "settings entered for the manual method are not given to another method"
    );
    assert!(manual_with.manual_settings().is_some());
}

#[test]
fn the_validations_are_invalidated_only_when_the_held_requirements_change() {
    let held = requirements();
    let mut changed = requirements();
    changed.settling_time_seconds = 50.0;
    assert!(
        !invalidation(None, None).validation,
        "nothing held, nothing changed"
    );
    assert!(
        !invalidation(Some(&held), Some(&held)).validation,
        "the same requirements invalidate nothing"
    );
    assert!(
        invalidation(Some(&held), Some(&changed)).validation,
        "changed requirements invalidate the validations (A8.4)"
    );
    assert!(
        invalidation(None, Some(&held)).validation,
        "requirements recorded where none were held are a change"
    );
}

#[test]
fn the_held_requirements_are_the_recorded_ones_and_none_without_a_record() {
    let recorded = InputsSection {
        requirements: Some(requirements()),
        ..InputsSection::default()
    };
    assert_eq!(
        held_requirements(Some(&recorded), None),
        Some(&requirements())
    );
    assert_eq!(
        held_requirements(Some(&InputsSection::default()), None),
        None
    );
    assert_eq!(held_requirements(None, None), None);
}
