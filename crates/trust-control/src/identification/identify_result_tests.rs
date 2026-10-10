#![allow(clippy::float_cmp)] // Plotted times and values are copied, so compared exactly.

//! Tests of the result of a stored identification (specification 35, A8.4 and B3.8; Identify page specification, "Identified
//! model and plot").

use serde_json::json;
use trust_control_math::identify::LowOrderModelKind;

use super::*;
use crate::identification::evidence::tests::fitted_model;
use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint, OperatingRange};
use crate::identification::experiment_trace::{IdentificationExperiment, TracePoint};
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::plant::IdentifiedModel;
use crate::identification::presentation::identified_model_presentation;
use crate::model::decode_general_model;
use crate::recording::EvidenceSource;
use crate::stages::stage_availability;
use crate::study::identify::IdentifyMethod;

/// The recording file the section's model was fitted from.
const RECORDING_ID: &str = "rec-00112233445566778899aabbccddeeff";

/// A loop whose setpoint `Plant.Target` and measurement `Plant.Position` are in m, driven by `Plant.Command` in V.
pub(crate) fn plant_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": "sha256:loop-1",
        "projectId": "project", "runtimeId": "runtime",
        "controller": { "instance": "Plant.Controller", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0 },
        "signals": {
            "setpoint": { "symbol": "Plant.Target", "iecType": "LREAL", "direction": "read", "unit": "m",
                "range": { "minimum": -10.0, "maximum": 10.0 } },
            "processVariable": { "symbol": "Plant.Position", "iecType": "LREAL", "direction": "read", "unit": "m",
                "range": { "minimum": -10.0, "maximum": 10.0 } },
            "manipulatedVariable": { "symbol": "Plant.Command", "iecType": "LREAL", "direction": "write", "unit": "V",
                "range": { "minimum": -10.0, "maximum": 10.0 } }
        },
        "limits": { "outputMinimum": -10.0, "outputMaximum": 10.0 },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 }
    }))
    .expect("the plant loop")
}

/// A trace of the experiment: its symbol, its unit and its value at 0, 1, 2 and 3 s.
fn trace(id: &str, unit: &str, values: [Option<f64>; 4]) -> ExperimentTrace {
    ExperimentTrace {
        id: id.to_owned(),
        unit: unit.to_owned(),
        points: [0.0, 1.0, 2.0, 3.0]
            .into_iter()
            .zip(values)
            .map(|(time_seconds, value)| TracePoint {
                time_seconds,
                value,
            })
            .collect(),
    }
}

/// The fit of the section: from 1 s, after the wait before the steady baseline, the measured and predicted position.
fn fit() -> ModelFitEvidence {
    let point = |time_seconds: f64, measured: f64, predicted: f64| ModelFitPoint {
        time_seconds,
        measured,
        predicted,
        residual: measured - predicted,
    };
    ModelFitEvidence {
        points: vec![
            point(1.0, 0.0, 0.0),
            point(2.0, 0.2, 0.19),
            point(3.0, 0.4, 0.39),
        ],
        root_mean_square_error: 0.01,
        output_unit: "m".to_owned(),
        stable: true,
        final_change_fraction: None,
        delay_seconds: 1.0,
        operating_range: OperatingRange {
            input_minimum: 0.0,
            input_maximum: 1.0,
            output_minimum: 0.0,
            output_maximum: 0.4,
        },
        warnings: Vec::new(),
        step_response: None,
        step_response_measurement: None,
    }
}

/// The provisional result of a step of `Plant.Command` from 0 to 1 V at 2 s: a first-order model with delay (gain 0.5 m per
/// V, time constant 2 s, delay 1 s, normalized error 0.04) whose experiment lost the position at 1 s.
fn first_order_section() -> IdentifySection {
    let identified = IdentifiedModel {
        time_constant_seconds: Some(2.0),
        dead_time_seconds: Some(1.0),
        normalized_error: 0.04,
        ..fitted_model(LowOrderModelKind::FirstOrderDeadTime, 0.5)
    };
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:recording"], "delaySeconds": 1.0,
        "inputs": [{ "id": "command", "unit": "V", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "outputs": [{ "id": "position", "unit": "m", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "numerator": [0.5], "denominator": [2.0, 1.0]
    }))
    .expect("a fitted model");
    let presentation = ComparisonPresentation {
        presentation: identified_model_presentation(&identified, &fit(), "V", "m"),
        experiment: Some(IdentificationExperiment {
            recording_id: Some("recording-1".to_owned()),
            inputs: vec![trace(
                "Plant.Command",
                "V",
                [Some(0.0), Some(0.0), Some(1.0), Some(1.0)],
            )],
            outputs: vec![trace(
                "Plant.Position",
                "m",
                [Some(0.0), None, Some(0.2), Some(0.4)],
            )],
        }),
        output_id: Some("position".to_owned()),
        input_channels: None,
    };
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        identify_signals(&plant_loop(), ControlExperimentKind::MvStep),
        RECORDING_ID,
        None,
        model,
        presentation,
    )
}

fn plot_trace(role: PlotRole, symbol: &str, unit: &str, values: &[Option<f64>]) -> PlotTrace {
    PlotTrace {
        role,
        symbol: symbol.to_owned(),
        unit: unit.to_owned(),
        values: values.to_vec(),
    }
}

#[test]
fn the_result_reads_the_stored_presentation_and_labels_a_provisional_model() {
    let section = first_order_section();
    let stages = stage_availability(&StageEvidence {
        has_model: true,
        ..StageEvidence::default()
    });
    let result = identify_result(&section, stages.clone());
    assert_eq!(result.model_type, "First-order model with delay");
    assert_eq!(result.equation, "G(s) = 0.5 e^(-1s) / (2s + 1)");
    assert_eq!(
        result.key_numbers,
        "Gain 0.5 m per V, time constant 2 s, delay 1 s"
    );
    assert!(result.provisional);
    assert_eq!(
        result.text,
        "First-order model with delay · Provisional\nG(s) = 0.5 e^(-1s) / (2s + 1)\nGain 0.5 m per V, time constant 2 s, delay 1 s"
    );
    assert_eq!(result.recording_id, RECORDING_ID);
    assert_eq!(result.stages, stages);

    let validated = IdentifySection {
        provisional: false,
        ..first_order_section()
    };
    let result = identify_result(&validated, Vec::new());
    assert!(!result.provisional);
    assert_eq!(
        result.text,
        "First-order model with delay\nG(s) = 0.5 e^(-1s) / (2s + 1)\nGain 0.5 m per V, time constant 2 s, delay 1 s"
    );
}

#[test]
fn the_plot_shows_the_applied_input_the_measurement_and_the_prediction_on_the_recordings_times() {
    let plot = identify_result(&first_order_section(), Vec::new()).plot;
    assert_eq!(plot.time_seconds, [0.0, 1.0, 2.0, 3.0]);
    assert_eq!(
        plot.traces,
        [
            plot_trace(
                PlotRole::PlantInput,
                "Plant.Command",
                "V",
                &[Some(0.0), Some(0.0), Some(1.0), Some(1.0)]
            ),
            plot_trace(
                PlotRole::Measurement,
                "Plant.Position",
                "m",
                &[Some(0.0), None, Some(0.2), Some(0.4)]
            ),
            plot_trace(
                PlotRole::Prediction,
                "position",
                "m",
                &[None, Some(0.0), Some(0.19), Some(0.39)]
            ),
        ],
        "the input and the measurement keep their recorded gap; the prediction starts where the fit does"
    );
}

#[test]
fn without_the_experiments_traces_the_plot_shows_the_measured_values_of_the_fit() {
    let mut section = first_order_section();
    section.presentation.experiment = None;
    section.presentation.output_id = None;
    let plot = identify_result(&section, Vec::new()).plot;
    assert_eq!(plot.time_seconds, [1.0, 2.0, 3.0]);
    assert_eq!(
        plot.traces,
        [
            plot_trace(
                PlotRole::Measurement,
                "Plant.Position",
                "m",
                &[Some(0.0), Some(0.2), Some(0.4)]
            ),
            plot_trace(
                PlotRole::Prediction,
                "position",
                "m",
                &[Some(0.0), Some(0.19), Some(0.39)]
            ),
        ],
        "the prediction is named by the model's output channel"
    );
}

#[test]
fn an_identification_names_the_setpoint_only_for_a_setpoint_step() {
    let loop_definition = plant_loop();
    assert_eq!(
        identify_signals(&loop_definition, ControlExperimentKind::MvStep),
        IdentifySignals {
            inputs: vec!["Plant.Command".to_owned()],
            outputs: vec!["Plant.Position".to_owned()],
            setpoint: None,
        }
    );
    assert_eq!(
        identify_signals(&loop_definition, ControlExperimentKind::SetpointStep).setpoint,
        Some("Plant.Target".to_owned())
    );
    let mut open_loop = plant_loop();
    open_loop.signals.setpoint = None;
    assert_eq!(
        identify_signals(&open_loop, ControlExperimentKind::SetpointStep).setpoint,
        None,
        "a loop without a setpoint names none"
    );
}

// Specification 35, A8.4: a model counts as the study's model only while it is current for the loop.
#[test]
fn the_study_has_a_model_only_when_its_identify_section_is_current() {
    let loop_definition = plant_loop();
    assert_eq!(
        study_evidence(&loop_definition, None, false),
        StageEvidence::default(),
        "a bound loop without evidence"
    );
    let recorded = study_evidence(&loop_definition, None, true);
    assert!(recorded.loop_bound && recorded.has_recording && !recorded.has_model);

    let section = first_order_section();
    let current = study_evidence(&loop_definition, Some(&section), true);
    assert!(current.has_model && current.has_recording);

    let mut moved = plant_loop();
    moved.signals.process_variable.symbol = "Plant.Level".into();
    assert!(
        !study_evidence(&moved, Some(&section), true).has_model,
        "a model of another measured output is not current"
    );
}
