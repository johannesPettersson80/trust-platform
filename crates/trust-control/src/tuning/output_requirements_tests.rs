//! Tests of one controlled output's own requirements (B4.3, C7): the checks they share with the record, the field and channel that each
//! refused value names, the shared values an output keeps from the record, and the decoding of `channelRequirements`.

#![allow(clippy::float_cmp)] // The values are the fixture's own numbers, which decode and compare unchanged.

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::failure::{Cause, NextAction};

/// The first output's record: a step of 1, an error of 0.3, a band of 0.1, 40 s to settle and an 80 s validation.
fn record() -> TuningRequirements {
    TuningRequirements {
        reference_change: 1.0,
        maximum_error: 0.3,
        settling_band: 0.1,
        settling_time_seconds: 40.0,
        duration_seconds: 80.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
        channel_requirements: None,
    }
}

/// The second output's own requirements: a step of 2, an error of 0.5, a band of 0.2 and 30 s to settle.
fn second() -> OutputRequirements {
    OutputRequirements {
        reference_change: 2.0,
        maximum_error: 0.5,
        settling_band: 0.2,
        settling_time_seconds: 30.0,
        maximum_overshoot_percent: None,
    }
}

/// The record with `own` as the requirements of the second output, `P1.Zone2C`.
fn with_second(own: OutputRequirements) -> TuningRequirements {
    TuningRequirements {
        channel_requirements: Some(BTreeMap::from([("P1.Zone2C".to_owned(), own)])),
        ..record()
    }
}

#[test]
fn a_record_whose_second_output_holds_its_own_limits_is_accepted() {
    assert!(with_second(second()).check().is_ok());
    assert!(with_second(second()).validate().is_ok());
}

#[test]
fn a_record_without_channel_requirements_is_checked_as_before() {
    assert_eq!(record().channel_requirements, None);
    assert!(record().check().is_ok());
}

#[test]
fn a_malformed_second_output_is_refused_under_its_field_and_channel() {
    let refusal = with_second(OutputRequirements {
        settling_band: 0.6,
        ..second()
    })
    .check()
    .expect_err("the second band is above its error limit");
    assert_eq!(
        refusal.sentence,
        "The error limit must be greater than the settling band."
    );
    assert_eq!(refusal.field, Some("maximumError"));
    assert_eq!(refusal.channel.as_deref(), Some("P1.Zone2C"));
    let failure = refusal.into_failure();
    assert_eq!(
        failure.message,
        "The error limit must be greater than the settling band."
    );
    assert_eq!(failure.field.as_deref(), Some("maximumError:P1.Zone2C"));
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );
}

#[test]
fn a_second_output_with_a_zero_setpoint_change_is_refused_under_its_channel() {
    let failure = with_second(OutputRequirements {
        reference_change: 0.0,
        ..second()
    })
    .check()
    .expect_err("a zero setpoint change")
    .into_failure();
    assert_eq!(failure.message, "Enter a nonzero setpoint change.");
    assert_eq!(failure.field.as_deref(), Some("referenceChange:P1.Zone2C"));
}

#[test]
fn a_second_output_overshoot_above_the_bound_is_refused_under_its_channel() {
    let failure = with_second(OutputRequirements {
        maximum_overshoot_percent: Some(MAXIMUM_OVERSHOOT_PERCENT + 0.5),
        ..second()
    })
    .check()
    .expect_err("an overshoot above the bound")
    .into_failure();
    assert_eq!(
        failure.field.as_deref(),
        Some("maximumOvershootPercent:P1.Zone2C")
    );
}

#[test]
fn the_duration_must_outlast_the_slowest_output_settling_time() {
    let slow = with_second(OutputRequirements {
        settling_time_seconds: 85.0,
        ..second()
    });
    assert_eq!(slow.slowest_settling_time_seconds(), 85.0);
    let refusal = slow
        .check()
        .expect_err("the duration does not outlast the second output");
    assert_eq!(refusal.sentence, "Allow longer than every settling time.");
    assert_eq!(refusal.field, Some("durationSeconds"));
    assert_eq!(
        refusal.channel, None,
        "the duration is the record's own, not one output's"
    );
}

#[test]
fn an_output_keeps_the_shared_values_of_the_record_and_takes_its_own_limits() {
    let shared = TuningRequirements {
        constant_input_disturbance: Some(3.0),
        measurement_noise_band: Some(0.1),
        ..with_second(second())
    };
    let for_second = shared.for_output(&second());
    assert_eq!(for_second.maximum_error, 0.5);
    assert_eq!(for_second.settling_time_seconds, 30.0);
    assert_eq!(for_second.duration_seconds, 80.0);
    assert_eq!(for_second.constant_input_disturbance, Some(3.0));
    assert_eq!(for_second.measurement_noise_band, Some(0.1));
    assert_eq!(for_second.channel_requirements, None);
}

#[test]
fn the_record_decodes_its_channel_requirements_and_refuses_a_misspelt_entry_field() {
    let decoded: TuningRequirements = serde_json::from_value(json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80.0,
        "channelRequirements": {
            "P1.Zone2C": {
                "referenceChange": 2.0,
                "maximumError": 0.5,
                "settlingBand": 0.2,
                "settlingTimeSeconds": 30.0
            }
        }
    }))
    .expect("the record decodes");
    assert_eq!(
        decoded.channel_requirements,
        with_second(second()).channel_requirements
    );
    let misspelt = serde_json::from_value::<TuningRequirements>(json!({
        "referenceChange": 1.0,
        "maximumError": 0.3,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 40.0,
        "durationSeconds": 80.0,
        "channelRequirements": {
            "P1.Zone2C": {
                "referenceChange": 2.0,
                "maximumError": 0.5,
                "settlingBand": 0.2,
                "settlingTimeSeconds": 30.0,
                "settlingTime": 30.0
            }
        }
    }));
    assert!(
        misspelt.is_err(),
        "a misspelt entry field is refused, not dropped"
    );
}
