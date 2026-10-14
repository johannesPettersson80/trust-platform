//! Tests of the plant choice after the probe (`model_replacement`): the comparison with the settling band, the first measured output of
//! a run, and the reason in words (specification 35, B5.3; A4.2 for the quality of a value).

#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use super::{first_measured_output, model_replacement, outside_settling_band};
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::identification::connected_record::{RunEvidence, ValidationChannel};
use crate::signals::Quality;
use crate::validation::isolated_run_protocol::{
    IsolatedJobState, IsolatedRunConditions, IsolatedRunRecord, IsolatedRunSample,
    IsolatedRunStatus, IsolatedRunValue,
};

/// A completed probe whose first sample measures `value` on the process symbol `LevelPV` with `quality`. The fixture loop
/// (`control_pid_loop`) measures `LevelPV` in `%`, and the fixture model operates at the output 2 (`first_order_model`).
fn probe_with(value: Option<f64>, quality: Quality) -> IsolatedRunStatus {
    let samples = vec![IsolatedRunSample {
        time_seconds: 0.0,
        sequence: 0,
        values: BTreeMap::from([("LevelPV".to_owned(), IsolatedRunValue { value, quality })]),
    }];
    IsolatedRunStatus {
        job_id: "isolated-probe".to_owned(),
        project_revision: "rev-1".to_owned(),
        status: IsolatedJobState::Completed,
        cancel_requested: false,
        reason: None,
        source_basis: "loaded-runtime-sources".to_owned(),
        io_provenance: "isolated-simulation-no-physical-drivers".to_owned(),
        captured_sample_count: 1,
        run: Some(IsolatedRunRecord {
            id: "run-0".to_owned(),
            evidence: RunEvidence::StExecuted,
            parameters: BTreeMap::new(),
            conditions: IsolatedRunConditions {
                initial_state: BTreeMap::new(),
                events: Vec::new(),
                duration_seconds: 1.0,
                sample_times_seconds: vec![0.0],
                seed: 1,
                model_revision: "model:fixture".to_owned(),
                program_revision: "rev-1".to_owned(),
                parameter_schema_revision: "schema-1".to_owned(),
                record_every: None,
                model_provenance: None,
                simulation_config_revision: None,
                feedback_relay: None,
            },
            channels: vec![ValidationChannel {
                id: "LevelPV".to_owned(),
                unit: "%".to_owned(),
            }],
            samples,
        }),
    }
}

#[test]
fn a_distance_inside_the_band_or_equal_to_it_keeps_the_configured_plant() {
    assert!(!outside_settling_band(2.01, 2.0, 0.02));
    assert!(
        !outside_settling_band(2.5, 2.0, 0.5),
        "a distance equal to the band is inside"
    );
    assert!(
        outside_settling_band(2.6, 2.0, 0.5),
        "a distance above the band is outside"
    );
    let loop_definition = control_pid_loop();
    let model = first_order_model();
    assert_eq!(
        model_replacement(
            &probe_with(Some(2.01), Quality::Good),
            &loop_definition,
            &model,
            0.02
        ),
        None
    );
    assert_eq!(
        model_replacement(
            &probe_with(Some(2.5), Quality::Good),
            &loop_definition,
            &model,
            0.5
        ),
        None
    );
}

#[test]
fn a_first_output_outside_the_band_replaces_the_configured_plant_and_names_why() {
    let reason = model_replacement(
        &probe_with(Some(5.0), Quality::Good),
        &control_pid_loop(),
        &first_order_model(),
        0.02,
    )
    .expect("the model replaces the configured plant");
    assert!(
        reason.contains("starts at 5 %"),
        "the first output: {reason}"
    );
    assert!(
        reason.contains("band of 0.02 %"),
        "the settling band: {reason}"
    );
    assert!(
        reason.contains("operating point 2 %"),
        "the operating point: {reason}"
    );
    assert!(reason.contains("every case runs on the model"), "{reason}");
}

#[test]
fn a_probe_without_a_measured_first_output_keeps_the_configured_plant() {
    let loop_definition = control_pid_loop();
    let model = first_order_model();
    let mut without_record = probe_with(Some(5.0), Quality::Good);
    without_record.run = None;
    assert_eq!(
        model_replacement(&without_record, &loop_definition, &model, 0.02),
        None,
        "a run without a record has no first output"
    );
    assert_eq!(
        model_replacement(
            &probe_with(None, Quality::Good),
            &loop_definition,
            &model,
            0.02
        ),
        None,
        "a missing value is no measurement"
    );
    assert_eq!(
        model_replacement(
            &probe_with(Some(5.0), Quality::Bad),
            &loop_definition,
            &model,
            0.02
        ),
        None,
        "a bad value is no measurement (A4.2)"
    );
}

#[test]
fn the_first_measured_output_is_read_by_the_measured_symbol_and_its_quality() {
    assert_eq!(
        first_measured_output(&probe_with(Some(5.0), Quality::Forced), "LevelPV"),
        Some(5.0),
        "a forced value is kept"
    );
    assert_eq!(
        first_measured_output(&probe_with(Some(5.0), Quality::Missing), "LevelPV"),
        None,
        "a missing value is no measurement"
    );
    assert_eq!(
        first_measured_output(&probe_with(Some(5.0), Quality::Good), "OtherPV"),
        None,
        "another symbol is not the measured output"
    );
}
