//! The fixtures of the workflow tests (`workflow_tests.rs`, `workflow_fit_tests.rs`): the samples, settings, original
//! recordings and workflows of the `TypeScript` tests, and accessors of a review. Reviews and models are compared as
//! JSON (`encoded`), as the `TypeScript` compared them with `deepStrictEqual`, so a NaN in a review equals itself.

use serde::Serialize;
use serde_json::Value;

use super::{IdentificationWorkflow, QualifyRequest, DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR};
use crate::experiment::{ControlExperimentKind, SampleQuality};
use crate::identification::data::{DataSource, IdentificationSample};
use crate::identification::evidence::ModelFitEvidence;
use crate::identification::form_draft::{IdentificationFormDraft, IDENTIFICATION_FORM_KEYS};
use crate::identification::plant::IdentifiedModel;
use crate::identification::review::{IdentificationModelSelection, IdentificationReview};
use crate::identification::state::IdentificationRecording;
use crate::identification::step_response::{
    SearchGrid, StepResponseAnalysis, StepResponseSettings,
};
use crate::loop_definition::LoopDefinition;
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::signals::Quality;

/// The measurement settings every `TypeScript` test shares, with its delay and time-constant grids.
pub(super) fn held_step_settings(
    delay: SearchGrid,
    time_constant: SearchGrid,
) -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 5.0,
        final_duration_seconds: 5.0,
        maximum_window_range: 0.1,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.05,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay,
        time_constant,
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
    }
}

pub(super) fn grid(minimum: f64, maximum: f64, step: f64) -> SearchGrid {
    SearchGrid {
        minimum,
        maximum,
        step,
    }
}

/// The settings of the state tests (`settings`).
pub(super) fn settings() -> StepResponseSettings {
    held_step_settings(grid(0.0, 2.0, 0.2), grid(4.0, 6.0, 0.2))
}

/// The settings of the step-response tests: a finer delay grid and a wide time-constant grid.
pub(super) fn step_settings() -> StepResponseSettings {
    held_step_settings(grid(0.0, 2.0, 0.1), grid(0.1, 20.0, 0.1))
}

/// The settings of the worker tests, without their sensor-time grid (parked with stage V7).
pub(super) fn worker_settings() -> StepResponseSettings {
    held_step_settings(grid(0.0, 2.0, 0.5), grid(1.0, 9.0, 0.5))
}

/// A good sample in `%` and `degC`, as the `TypeScript` fixtures write them.
fn row(time_seconds: f64, input: f64, output: f64) -> IdentificationSample {
    IdentificationSample {
        time_seconds,
        input,
        output,
        quality: SampleQuality::Good,
        input_unit: Some("%".to_string()),
        output_unit: Some("degC".to_string()),
        clipped: None,
        saturated: None,
    }
}

/// A good sample in `V` and `m`.
fn volts(time_seconds: f64, input: f64, output: f64) -> IdentificationSample {
    IdentificationSample {
        input_unit: Some("V".to_string()),
        output_unit: Some("m".to_string()),
        ..row(time_seconds, input, output)
    }
}

/// `samples(change)` of the state tests: 452 samples 100 ms apart; the input is `75 + change` from 5 s to 45 s and 75
/// otherwise; the output is 35 and rises from 6 s towards `35 + 0.2·change` with a 5 s time constant.
pub(super) fn samples(change: f64) -> Vec<IdentificationSample> {
    (0_u32..452)
        .map(|index| {
            let time = f64::from(index) / 10.0;
            let input = if (50..450).contains(&index) {
                75.0 + change
            } else {
                75.0
            };
            let output = if index <= 60 {
                35.0
            } else {
                35.0 + 0.2 * change * (1.0 - (-(time - 6.0) / 5.0).exp())
            };
            row(time, input, output)
        })
        .collect()
}

/// `record(change)` of the step-response tests: the same step on a time axis that starts at −5 s, so that the step is
/// at 0 s and the response starts at 1 s.
pub(super) fn record(change: f64) -> Vec<IdentificationSample> {
    (0_u32..452)
        .map(|index| {
            let time = f64::from(index) / 10.0 - 5.0;
            let input = if (0.0..40.0).contains(&time) {
                75.0 + change
            } else {
                75.0
            };
            let output = if time <= 1.0 {
                35.0
            } else {
                35.0 + 0.2 * change * (1.0 - (-(time - 1.0) / 5.0).exp())
            };
            row(time, input, output)
        })
        .collect()
}

/// `samples(change)` of the worker tests: 252 samples 200 ms apart, the input stepped from 5 s to 50 s, the output
/// rising from 6 s.
pub(super) fn worker_samples(change: f64) -> Vec<IdentificationSample> {
    (0_u32..252)
        .map(|index| {
            let time = f64::from(index) / 5.0;
            let input = if (25..250).contains(&index) {
                75.0 + change
            } else {
                75.0
            };
            let output = if index <= 30 {
                35.0
            } else {
                35.0 + 0.2 * change * (1.0 - (-(time - 6.0) / 5.0).exp())
            };
            row(time, input, output)
        })
        .collect()
}

/// `samples(change)` of the form-draft and saved-model tests: `count` samples 100 ms apart, a step at 2 s from 50 to
/// `50 + change`, and a first-order response from 20 towards `20 + 0.5·change` with a 3 s time constant.
pub(super) fn first_order_samples(count: u32, change: f64) -> Vec<IdentificationSample> {
    (0..count)
        .map(|index| {
            let time = f64::from(index) / 10.0;
            if index < 20 {
                row(time, 50.0, 20.0)
            } else {
                let response = change * 0.5 * (1.0 - (-(time - 2.0) / 3.0).exp());
                row(time, 50.0 + change, 20.0 + response)
            }
        })
        .collect()
}

/// `identificationSamples(amplitude)` of the experiment tests: 30 samples 1 s apart in `%`, a step at 5 s.
pub(super) fn ledger_samples(amplitude: f64) -> Vec<IdentificationSample> {
    (0_u32..30)
        .map(|index| {
            let time = f64::from(index);
            let (input, output) = if index < 5 {
                (0.0, 0.0)
            } else {
                (
                    amplitude,
                    amplitude * 0.2 * (1.0 - (-(time - 5.0) / 4.0).exp()),
                )
            };
            IdentificationSample {
                output_unit: Some("%".to_string()),
                ..row(time, input, output)
            }
        })
        .collect()
}

/// The samples of the operating-point test: 301 samples 100 ms apart, the input at 2 and stepped by `amplitude` at
/// 2 s, the output at `output0` and rising by `amplitude` with a 1 s time constant.
pub(super) fn operating_point_samples(amplitude: f64, output0: f64) -> Vec<IdentificationSample> {
    (0_u32..301)
        .map(|index| {
            let stepped = index >= 20;
            let input = 2.0 + if stepped { amplitude } else { 0.0 };
            let rise = amplitude * (1.0 - (-(f64::from(index) - 20.0) * 0.1).exp());
            let output = output0 + if stepped { rise } else { 0.0 };
            volts(f64::from(index) * 0.1, input, output)
        })
        .collect()
}

/// The samples of the general-model test: 301 samples 100 ms apart, a step of `amplitude` at 2 s, and a response that
/// starts at 2.5 s towards `2·amplitude` with a 3 s time constant.
pub(super) fn delayed_samples(amplitude: f64) -> Vec<IdentificationSample> {
    (0_u32..301)
        .map(|index| {
            let input = if index >= 20 { amplitude } else { 0.0 };
            let response = 2.0 * amplitude * (1.0 - (-(f64::from(index) - 25.0) * 0.1 / 3.0).exp());
            let output = if index >= 25 { response } else { 0.0 };
            volts(f64::from(index) * 0.1, input, output)
        })
        .collect()
}

/// The 24 samples of the draft-storage tests: 100 ms apart, input 1 and then 2 from 0.8 s, output 3 and then 3.5.
pub(super) fn draft_samples() -> Vec<IdentificationSample> {
    (0_u32..24)
        .map(|index| {
            let stepped = index >= 8;
            let input = if stepped { 2.0 } else { 1.0 };
            let output = if stepped { 3.5 } else { 3.0 };
            row(f64::from(index) / 10.0, input, output)
        })
        .collect()
}

/// The fixture loop with the measurement in `degC`, as the recordings of the `TypeScript` tests report it.
pub(super) fn test_loop() -> LoopDefinition {
    let mut definition = loop_definition();
    definition.signals.process_variable.unit = "degC".to_string();
    definition
}

/// The original runtime recording of `rows` (`originalRecording`): a project sequence of the measurement and the
/// manipulated variable, one sample every 100 ms from scan 10, under the recording identifier `id`.
pub(super) fn recording_of(rows: &[IdentificationSample], id: &str) -> IdentificationRecording {
    let mut recorded = run(
        vec![
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        (0_u32..)
            .zip(rows)
            .map(|(index, measured)| {
                sample(
                    u64::from(index) + 10,
                    Some(i64::from(index) * 100_000_000),
                    &[Some(measured.output), Some(measured.input)],
                    &[Quality::Good; 2],
                )
            })
            .collect(),
    );
    recorded.recording_id = id.to_string();
    IdentificationRecording {
        kind: ControlExperimentKind::ProjectSequence,
        run: recorded,
        selected_intervals: Vec::new(),
        provenance: None,
    }
}

/// A check of `samples` from a simulated plant with no excluded interval, as the `TypeScript` tests call `qualify`.
pub(super) fn request(
    samples: Vec<IdentificationSample>,
    requested_kind: IdentificationModelSelection,
    id: Option<&str>,
    settings: Option<StepResponseSettings>,
) -> QualifyRequest {
    QualifyRequest {
        samples,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind,
        recording_id: id.map(str::to_string),
        step_response_settings: settings,
        recording: None,
        first_order_step_response_settings: None,
    }
}

/// `qualify(samples(change), [], "Simulation", "fopdt", id, settings)` of the state tests.
pub(super) fn held_step(change: f64, id: &str) -> QualifyRequest {
    request(
        samples(change),
        IdentificationModelSelection::Fopdt,
        Some(id),
        Some(settings()),
    )
}

/// The same request with the original recording of its samples (`originalRecording(change, id)`).
pub(super) fn held_step_with_recording(change: f64, id: &str) -> QualifyRequest {
    QualifyRequest {
        recording: Some(recording_of(&samples(change), id)),
        ..held_step(change, id)
    }
}

/// A held-step FOPDT request of the step-response tests.
pub(super) fn step_request(samples: Vec<IdentificationSample>, id: &str) -> QualifyRequest {
    request(
        samples,
        IdentificationModelSelection::Fopdt,
        Some(id),
        Some(step_settings()),
    )
}

/// A new workflow with the default error limit (`new IdentificationWorkflow()`).
pub(super) fn workflow() -> IdentificationWorkflow {
    IdentificationWorkflow::new(DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR)
}

/// `preview()`: the 10-point estimation step fitted as a FOPDT preview.
pub(super) fn preview() -> IdentificationWorkflow {
    let mut workflow = workflow();
    workflow.qualify(held_step(10.0, "source-estimation"));
    workflow.fit().expect("the estimation fits");
    workflow
}

/// The preview validated with the 12-point step and accepted.
pub(super) fn accepted_preview() -> IdentificationWorkflow {
    let mut workflow = preview();
    workflow.qualify(held_step(12.0, "source-validation"));
    workflow.fit().expect("the validation fits");
    workflow.accept().expect("the validated model is accepted");
    workflow
}

/// `acceptedWithOriginals()`: both steps with their original recordings, validated and accepted.
pub(super) fn accepted_with_originals() -> IdentificationWorkflow {
    let mut workflow = workflow();
    workflow.qualify(held_step_with_recording(10.0, "source-estimation"));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(held_step_with_recording(12.0, "source-validation"));
    workflow.fit().expect("the validation fits");
    workflow.accept().expect("the validated model is accepted");
    workflow
}

/// `acceptedWorkflow()` and `accepted()`: a first-order estimation of `count` samples with a 10-point step, validated
/// with a 12-point step and accepted.
pub(super) fn accepted_first_order(count: u32) -> IdentificationWorkflow {
    let mut workflow = workflow();
    workflow.qualify(request(
        first_order_samples(count, 10.0),
        IdentificationModelSelection::FirstOrder,
        Some("estimation"),
        None,
    ));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(request(
        first_order_samples(count, 12.0),
        IdentificationModelSelection::FirstOrder,
        Some("validation"),
        None,
    ));
    workflow.fit().expect("the validation fits");
    workflow.accept().expect("the validated model is accepted");
    workflow
}

/// The form draft of the `TypeScript` tests: FOPDT with the held step on, every field "5" except a blank
/// "delay.minimum".
pub(super) fn form() -> IdentificationFormDraft {
    IdentificationFormDraft {
        schema_version: 1,
        model: IdentificationModelSelection::Fopdt,
        held_step: true,
        values: IDENTIFICATION_FORM_KEYS
            .iter()
            .map(|key| {
                let text = if *key == "delay.minimum" { "" } else { "5" };
                ((*key).to_string(), text.to_string())
            })
            .collect(),
        excluded_start: String::new(),
        excluded_end: String::new(),
    }
}

/// A value as JSON.
pub(super) fn encoded<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("the value encodes")
}

/// The saved state of a workflow as JSON (`snapshot(workflow)`).
pub(super) fn snapshot(workflow: &IdentificationWorkflow) -> Value {
    encoded(&workflow.snapshot().expect("the workflow has a saved state"))
}

/// A new workflow restored from `saved`.
pub(super) fn restored(saved: &Value) -> IdentificationWorkflow {
    let mut workflow = workflow();
    workflow
        .restore(saved, &test_loop())
        .expect("the saved state restores");
    workflow
}

/// The model a review selected.
pub(super) fn selected(review: &IdentificationReview) -> &IdentifiedModel {
    match review {
        IdentificationReview::ValidationRequired(validation) => &validation.selected,
        IdentificationReview::AcceptanceRequired(acceptance) => &acceptance.selected,
    }
}

/// The fit evidence of a review.
pub(super) fn fit_evidence(review: &IdentificationReview) -> &ModelFitEvidence {
    match review {
        IdentificationReview::ValidationRequired(validation) => &validation.fit,
        IdentificationReview::AcceptanceRequired(acceptance) => &acceptance.fit,
    }
}

/// The measured-step analysis of a bounded step review.
pub(super) fn step_analysis(review: &IdentificationReview) -> Option<&StepResponseAnalysis> {
    match review {
        IdentificationReview::ValidationRequired(validation) => validation.step_response.as_ref(),
        IdentificationReview::AcceptanceRequired(acceptance) => acceptance.step_response.as_ref(),
    }
}

/// Whether a refusal names the validation, the bound or the review (`/validation|bound|review/i`).
pub(super) fn names_validation_bound_or_review(sentence: &str) -> bool {
    let sentence = sentence.to_lowercase();
    ["validation", "bound", "review"]
        .iter()
        .any(|word| sentence.contains(word))
}
