//! Tests of the measured process noise (`processNoiseEvidence.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use super::measured_process_noise;
use crate::identification::independent_validation::recording_fingerprint;
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{build, fitted_model, Row, Spec};
use crate::loop_definition::LoopDefinition;

const CHANNELS: &[(&str, &str)] = &[("MV", "%"), ("PV", "degC")];

/// The fixture loop with the symbols and the task period of these tests: a 100 ms task unless `task_ms` says otherwise.
fn loop_with(task_ms: f64) -> LoopDefinition {
    let mut loop_definition = crate::recording::fixtures::loop_definition();
    loop_definition.controller.task_period_ms = task_ms;
    loop_definition.signals.process_variable.symbol = "PV".to_owned();
    loop_definition.signals.manipulated_variable.symbol = "MV".to_owned();
    loop_definition
}

/// `count` samples 0.1 s apart with the input at 50 and the output alternating ±0.1 about 20.
fn alternating_rows(count: u64) -> Vec<Row> {
    (0..count)
        .map(|index| {
            let swing = if index % 2 == 0 { 0.1 } else { -0.1 };
            (
                index as f64 * 0.1,
                index,
                vec![Some(50.0), Some(20.0 + swing)],
            )
        })
        .collect()
}

#[test]
fn ten_alternating_samples_give_three_sigma_of_the_output() {
    // The output has mean 20 and squares 0.01 about it, so σ = √(10·0.01/9) and the noise is 3σ = √0.1 with one
    // execution per sample (a 0.1 s spacing over a 100 ms task).
    let rows = alternating_rows(10);
    let record = build(&Spec {
        id: "steady",
        project_revision: "sha256:program",
        model_revision: "sha256:plant",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let model = fitted_model(
        vec![recording_fingerprint(&record.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let noise = measured_process_noise(&loop_with(100.0), Some(&model), &[record]);
    assert!((noise - 0.1_f64.sqrt()).abs() <= 1e-12, "noise {noise}");
}

#[test]
fn the_noise_scales_with_the_executions_each_sample_averages() {
    // A 10 ms task gives ten executions per 0.1 s sample, so the noise is √10 times the single-execution value.
    let rows = alternating_rows(10);
    let record = build(&Spec {
        id: "steady",
        project_revision: "sha256:program",
        model_revision: "sha256:plant",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let model = fitted_model(
        vec![recording_fingerprint(&record.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let noise = measured_process_noise(&loop_with(10.0), Some(&model), &[record]);
    assert!((noise - 1.0).abs() <= 1e-9, "noise {noise}");
}

#[test]
fn a_closed_loop_baseline_at_a_record_period_is_measured_over_its_window() {
    // Samples one second apart average ten executions of a 100 ms task. In Auto the input moves during the baseline, but
    // the window names thirty samples alternating ±0.01 about 45: σ = 0.01·√(30/29), noise = 3σ·√10.
    let rows: Vec<Row> = (0..40_u64)
        .map(|index| {
            let swing = if index % 2 == 1 { 0.01 } else { -0.01 };
            (
                index as f64,
                10 * (index + 1),
                vec![Some(38.0 + index as f64 / 100.0), Some(45.0 + swing)],
            )
        })
        .collect();
    let record = build(&Spec {
        id: "baseline",
        project_revision: "program",
        model_revision: "sha256:plant",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &rows,
        baseline: Some((0.0, 29.0)),
        verdict: RequirementStatus::Pass,
    });
    let model = fitted_model(
        vec![recording_fingerprint(&record.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let expected = 3.0 * 0.01 * (30.0_f64 / 29.0).sqrt() * 10.0_f64.sqrt();
    let noise = measured_process_noise(&loop_with(100.0), Some(&model), &[record]);
    assert!(
        (noise - expected).abs() <= 1e-12,
        "noise {noise} expected {expected}"
    );
}

#[test]
fn zero_noise_without_the_model_its_recording_or_ten_baseline_samples() {
    // Specification A6.4: the noise is zero without a baseline. The TypeScript returned undefined in these cases, so each
    // assertion below changed from an absent noise to zero.
    let rows = alternating_rows(10);
    let record = build(&Spec {
        id: "steady",
        project_revision: "sha256:program",
        model_revision: "sha256:plant",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let loop_definition = loop_with(100.0);
    let without_model =
        measured_process_noise(&loop_definition, None, std::slice::from_ref(&record));
    assert!(without_model.abs() <= 1e-12, "no model: {without_model}");
    let other = fitted_model(vec!["sha256:other".to_owned()], vec![1.0], vec![1.0, 1.0]);
    let without_recording = measured_process_noise(
        &loop_definition,
        Some(&other),
        std::slice::from_ref(&record),
    );
    assert!(
        without_recording.abs() <= 1e-12,
        "no recording: {without_recording}"
    );
    let short_rows = alternating_rows(9);
    let short = build(&Spec {
        id: "short",
        project_revision: "sha256:program",
        model_revision: "sha256:plant",
        kind: "setpoint_step",
        channels: CHANNELS,
        rows: &short_rows,
        baseline: None,
        verdict: RequirementStatus::Pass,
    });
    let model = fitted_model(
        vec![recording_fingerprint(&short.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let short_baseline = measured_process_noise(&loop_definition, Some(&model), &[short]);
    assert!(
        short_baseline.abs() <= 1e-12,
        "fewer than ten baseline samples: {short_baseline}"
    );
}
