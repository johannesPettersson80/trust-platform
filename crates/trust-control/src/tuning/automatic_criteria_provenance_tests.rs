//! Tests of the generated status of the requirements after a change (B4.3): a value that the change moves stops being generated, and a value
//! that the change leaves as it was stays generated.

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::tuning::output_requirements::OutputRequirements;

/// The requirements of a study that generated `fields`, decoded as the page sends them.
fn requirements(reference: f64, settling: &str, fields: &[&str]) -> TuningRequirements {
    crate::json::decode_value(json!({
        "referenceChange": reference,
        "maximumError": 0.1,
        "settlingBand": 0.02,
        "settlingTimeSeconds": settling,
        "durationSeconds": "180 s",
        "automaticCriteria": { "modelRevision": "rev-1", "fields": fields }
    }))
    .expect("a requirements record decodes")
}

/// The requirements of one controlled output, with the given setpoint change.
fn output(reference: f64) -> OutputRequirements {
    crate::json::decode_value(json!({
        "referenceChange": reference,
        "maximumError": 0.1,
        "settlingBand": 0.02,
        "settlingTimeSeconds": "90 s"
    }))
    .expect("an output's requirements decode")
}

#[test]
fn a_generated_value_that_the_change_moves_is_no_longer_generated_and_the_others_stay() {
    let previous = requirements(1.0, "90 s", &["referenceChange", "settlingTimeSeconds"]);
    let mut next = requirements(2.0, "90 s", &["referenceChange", "settlingTimeSeconds"]);
    keep_generated_fields(&previous, &mut next);
    let kept = next
        .automatic_criteria
        .expect("the record the change sent is kept");
    assert_eq!(kept.fields, ["settlingTimeSeconds"]);
    assert_eq!(kept.model_revision, "rev-1");
}

#[test]
fn a_key_that_gives_no_value_of_the_requirements_stays_generated() {
    let previous = requirements(1.0, "90 s", &["tuningObjective"]);
    let mut next = requirements(2.0, "90 s", &["tuningObjective"]);
    keep_generated_fields(&previous, &mut next);
    let kept = next
        .automatic_criteria
        .expect("the record the change sent is kept");
    assert_eq!(kept.fields, ["tuningObjective"]);
}

#[test]
fn a_channel_value_is_generated_while_its_own_output_keeps_it() {
    let mut previous = requirements(
        1.0,
        "90 s",
        &["referenceChange:heater", "maximumError:heater"],
    );
    previous.channel_requirements = Some(BTreeMap::from([("heater".to_owned(), output(1.0))]));
    let mut next = previous.clone();
    next.channel_requirements = Some(BTreeMap::from([("heater".to_owned(), output(2.0))]));
    keep_generated_fields(&previous, &mut next);
    let kept = next
        .automatic_criteria
        .expect("the record the change sent is kept");
    assert_eq!(kept.fields, ["maximumError:heater"]);
}

#[test]
fn a_change_with_no_previous_record_keeps_the_record_it_sent() {
    let previous = TuningRequirements {
        automatic_criteria: None,
        ..requirements(1.0, "90 s", &[])
    };
    let mut next = requirements(2.0, "90 s", &["referenceChange"]);
    keep_generated_fields(&previous, &mut next);
    let kept = next
        .automatic_criteria
        .expect("the record the change sent stays as it is");
    assert_eq!(kept.fields, ["referenceChange"]);
}
