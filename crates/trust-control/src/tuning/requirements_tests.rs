//! Tests of the requirement record's checks (B4.3) and of the measurement-noise rule in `requirements.rs`.

use super::*;

fn valid() -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
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

/// The refusal that the checks give for a record they refuse.
fn refused(record: &TuningRequirements) -> RequirementRefusal {
    record.check().expect_err("the record is refused")
}

#[test]
fn a_transfer_tolerance_above_the_output_span_is_refused_under_its_field() {
    let mut record = valid();
    record.transfer_tolerance = Some(12.0);
    let refusal = record
        .check_output_span(Some(10.0))
        .expect_err("a tolerance above the output span is refused");
    assert_eq!(refusal.field, Some("transferTolerance"));
    assert_eq!(refusal.sentence, "Use 10 or less.");
}

#[test]
fn a_transfer_tolerance_at_the_span_or_with_no_span_or_no_tolerance_is_kept() {
    let mut record = valid();
    record.transfer_tolerance = Some(10.0);
    assert_eq!(record.check_output_span(Some(10.0)), Ok(()));
    record.transfer_tolerance = Some(50.0);
    assert_eq!(record.check_output_span(None), Ok(()));
    record.transfer_tolerance = None;
    assert_eq!(record.check_output_span(Some(10.0)), Ok(()));
}

#[test]
fn a_complete_record_is_accepted() {
    assert!(valid().check().is_ok());
    assert!(valid().validate().is_ok());
}

#[test]
fn the_settling_band_must_be_below_the_error_limit() {
    let mut bad = valid();
    bad.settling_band = 0.4;
    assert_eq!(
        bad.validate().expect_err("band not below error").0,
        "The error limit must be greater than the settling band."
    );
    let refusal = refused(&bad);
    assert_eq!(
        refusal.sentence,
        "The error limit must be greater than the settling band."
    );
    assert_eq!(refusal.field, Some("maximumError"));
}

#[test]
fn a_zero_setpoint_change_is_refused_under_its_field() {
    let mut bad = valid();
    bad.reference_change = 0.0;
    let refusal = refused(&bad);
    assert_eq!(refusal.sentence, "Enter a nonzero setpoint change.");
    assert_eq!(refusal.field, Some("referenceChange"));
}

#[test]
fn a_settling_band_that_is_not_positive_is_refused_under_its_field() {
    let mut bad = valid();
    bad.settling_band = 0.0;
    let refusal = refused(&bad);
    assert_eq!(refusal.sentence, "Enter a positive settling band.");
    assert_eq!(refusal.field, Some("settlingBand"));
}

#[test]
fn a_settling_time_that_is_not_positive_is_refused_under_its_field() {
    let mut bad = valid();
    bad.settling_time_seconds = 0.0;
    let refusal = refused(&bad);
    assert_eq!(refusal.sentence, "Enter a positive settling time.");
    assert_eq!(refusal.field, Some("settlingTimeSeconds"));
}

#[test]
fn an_overshoot_limit_of_zero_or_less_is_refused_under_its_field() {
    for limit in [0.0, -1.0] {
        let mut bad = valid();
        bad.maximum_overshoot_percent = Some(limit);
        let refusal = refused(&bad);
        assert_eq!(
            refusal.sentence, "Enter an overshoot limit above 0 %, or leave it blank for no limit.",
            "limit {limit}"
        );
        assert_eq!(
            refusal.field,
            Some("maximumOvershootPercent"),
            "limit {limit}"
        );
    }
}

#[test]
fn a_duration_not_longer_than_the_settling_time_is_refused_under_its_field() {
    let mut bad = valid();
    bad.duration_seconds = 45.0;
    let refusal = refused(&bad);
    assert_eq!(refusal.sentence, "Allow longer than every settling time.");
    assert_eq!(refusal.field, Some("durationSeconds"));
}

#[test]
fn a_duration_beyond_the_runtime_limit_is_refused_under_its_field() {
    let mut bad = valid();
    bad.duration_seconds = maximum_duration_seconds() + 1.0;
    let refusal = refused(&bad);
    assert_eq!(refusal.sentence, "Use 86400 s or less.");
    assert_eq!(refusal.field, Some("durationSeconds"));
}

#[test]
fn a_refused_value_is_a_settings_failure_that_names_its_field() {
    let mut bad = valid();
    bad.settling_time_seconds = 0.0;
    let failure = refused(&bad).into_failure();
    assert_eq!(failure.message, "Enter a positive settling time.");
    assert_eq!(failure.field.as_deref(), Some("settlingTimeSeconds"));
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );
}

#[test]
fn the_duration_must_outlast_the_settling_time_and_fit_the_runtime() {
    let mut short = valid();
    short.duration_seconds = 45.0;
    assert!(short.validate().is_err());
    let mut long = valid();
    long.duration_seconds = maximum_duration_seconds() + 1.0;
    assert!(long.validate().is_err());
}

#[test]
fn an_overshoot_limit_must_be_positive_and_at_most_a_thousand_percent() {
    let mut ok = valid();
    ok.maximum_overshoot_percent = Some(1000.0);
    assert!(ok.validate().is_ok());
    ok.maximum_overshoot_percent = Some(1000.5);
    assert!(ok.validate().is_err());
    ok.maximum_overshoot_percent = Some(0.0);
    assert!(ok.validate().is_err());
}

#[test]
fn the_uncertainty_fraction_is_between_zero_and_one() {
    let mut bad = valid();
    bad.gain_uncertainty_fraction = Some(1.5);
    assert_eq!(
        bad.validate().expect_err("out of range").0,
        "Enter finite disturbance/noise bounds and gain uncertainty between zero and one."
    );
    assert_eq!(
        refused(&bad).field,
        Some("gainUncertaintyFraction"),
        "the uncertainty names its field"
    );
}

#[test]
fn the_steady_error_allowance_is_the_error_limit_less_the_band() {
    let bounds = GainBounds {
        minimum: 0.0,
        maximum: 100.0,
    };
    let proportional = valid().proportional(bounds);
    assert!((proportional.maximum_steady_error - 0.3).abs() < 1e-12);
    assert_eq!(proportional.constant_input_disturbance, 0.0);
}

#[test]
fn the_response_deadline_rounds_up_to_a_task_period() {
    // 45 s at a 100 ms task is already whole periods; 45.05 s rounds up to 45.1 s.
    assert_eq!(response_deadline_seconds(45.0, 100.0), 45.0);
    assert!((response_deadline_seconds(45.05, 100.0) - 45.1).abs() < 1e-9);
}

#[test]
fn a_declared_low_pass_or_notch_needs_the_measurement_noise_band() {
    let low_pass = ProjectTuningComponent::LowPass {
        time_constant_symbol: "P1.TF".to_owned(),
        time_constants_seconds: vec![0.1, 0.5],
        order: 1,
    };
    let notch = ProjectTuningComponent::Notch {
        convention: "direct-form-minus-feedback".to_owned(),
        symbols: vec![
            "P1.B0".to_owned(),
            "P1.B1".to_owned(),
            "P1.B2".to_owned(),
            "P1.A1".to_owned(),
            "P1.A2".to_owned(),
        ],
        frequencies_hz: vec![3.0],
        quality_factors: vec![1.0],
    };
    assert!(noise_band_required(&[low_pass]));
    assert!(noise_band_required(&[notch]));
}

#[test]
fn a_loop_with_no_filter_declared_does_not_need_the_measurement_noise_band() {
    let back_calculation = ProjectTuningComponent::BackCalculation {
        gain_symbol: "P1.KT".to_owned(),
        tracking_times_seconds: vec![1.0],
    };
    let weighting = ProjectTuningComponent::ReferenceWeighting {
        proportional_symbol: "P1.B".to_owned(),
        derivative_symbol: "P1.C".to_owned(),
        proportional_weights: vec![0.5, 1.0],
        derivative_weights: vec![0.0],
    };
    assert!(!noise_band_required(&[]));
    assert!(!noise_band_required(&[back_calculation, weighting]));
}

#[test]
fn a_duration_typed_as_text_reads_as_seconds() {
    let record: TuningRequirements = serde_json::from_value(serde_json::json!({
        "referenceChange": 2.0,
        "maximumError": 0.4,
        "settlingBand": 0.1,
        "settlingTimeSeconds": "1 h 30 min",
        "durationSeconds": "2 h",
    }))
    .expect("the record reads");
    assert_eq!(record.settling_time_seconds, 5400.0);
    assert_eq!(record.duration_seconds, 7200.0);
}

#[test]
fn an_unreadable_settling_time_is_refused_under_its_field() {
    let record: TuningRequirements = serde_json::from_value(serde_json::json!({
        "referenceChange": 2.0,
        "maximumError": 0.4,
        "settlingBand": 0.1,
        "settlingTimeSeconds": "soon",
        "durationSeconds": 90.0,
    }))
    .expect("the record reads");
    assert_eq!(refused(&record).field, Some("settlingTimeSeconds"));
}

#[test]
fn an_unreadable_validation_duration_is_refused_under_its_field() {
    let record: TuningRequirements = serde_json::from_value(serde_json::json!({
        "referenceChange": 2.0,
        "maximumError": 0.4,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 45.0,
        "durationSeconds": "later",
    }))
    .expect("the record reads");
    assert_eq!(refused(&record).field, Some("durationSeconds"));
}
