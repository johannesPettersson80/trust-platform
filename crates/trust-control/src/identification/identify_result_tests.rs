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
use crate::messages::TuneRequest;
use crate::model::decode_general_model;
use crate::recording::EvidenceSource;
use crate::stages::{stage_availability, Stage};
use crate::study::identify::IdentifyMethod;
use crate::study::tune::{tune, TuneInputs};
use crate::tuning::automatic_criteria::{AutomaticCriteria, REQUIREMENT_KEYS};

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
        normalized_error: Some(0.04),
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
    let evidence = StageEvidence {
        has_model: true,
        ..StageEvidence::default()
    };
    let result = identify_result(&section, evidence, None);
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
    assert_eq!(result.recording_id.as_deref(), Some(RECORDING_ID));
    assert_eq!(result.stages, stage_availability(&evidence));
    assert_eq!(result.stage_evidence, evidence);

    let validated = IdentifySection {
        provisional: false,
        ..first_order_section()
    };
    let result = identify_result(&validated, StageEvidence::default(), None);
    assert!(!result.provisional);
    assert_eq!(
        result.text,
        "First-order model with delay\nG(s) = 0.5 e^(-1s) / (2s + 1)\nGain 0.5 m per V, time constant 2 s, delay 1 s"
    );
}

#[test]
fn the_plot_shows_the_applied_input_the_measurement_and_the_prediction_on_the_recordings_times() {
    let plot = identify_result(&first_order_section(), StageEvidence::default(), None).plot;
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
fn a_fit_without_its_experiment_names_its_measurement_by_the_recorded_channel_it_read() {
    // B3.6: a fit of a recorded channel that the loop does not bind names its measurement by that channel, in the plot of its
    // first answer and of `controlStudio/open` alike (`read_channels`).
    let mut section = first_order_section();
    section.presentation.experiment = None;
    section.recorded_channels = Some(IdentifySignals {
        inputs: vec!["Plant.Command".to_owned()],
        outputs: vec!["Plant.Recorded".to_owned()],
        setpoint: None,
    });
    let measurement = |section: &IdentifySection| -> Option<String> {
        identify_result(section, StageEvidence::default(), None)
            .plot
            .traces
            .into_iter()
            .find(|trace| trace.role == PlotRole::Measurement)
            .map(|trace| trace.symbol)
    };
    assert_eq!(measurement(&section).as_deref(), Some("Plant.Recorded"));
    section.recorded_channels = None;
    assert_eq!(measurement(&section).as_deref(), Some("Plant.Position"));
}

#[test]
fn without_the_experiments_traces_the_plot_shows_the_measured_values_of_the_fit() {
    let mut section = first_order_section();
    section.presentation.experiment = None;
    section.presentation.output_id = None;
    let plot = identify_result(&section, StageEvidence::default(), None).plot;
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
    let current = study_evidence(
        &loop_definition,
        Some(&CurrentModel::identified(section.clone())),
        true,
    );
    assert!(current.has_model && current.has_recording);

    let mut moved = plant_loop();
    moved.signals.process_variable.symbol = "Plant.Level".into();
    assert!(
        !study_evidence(&moved, Some(&CurrentModel::identified(section)), true).has_model,
        "a model of another measured output is not current"
    );
}

// Specification 35, C12: a loop without a controller law keeps its Identify evidence, and its Tune stage is Not applicable.
#[test]
fn a_loop_without_a_controller_law_gives_the_study_no_controller_stage() {
    let mut observed = plant_loop();
    observed.controller.law = None;
    observed.controller.design = Some(crate::loop_definition::Design::Project);
    observed.topology = Some(crate::loop_definition::Topology::OpenLoop);
    let evidence = study_evidence(
        &observed,
        Some(&CurrentModel::identified(first_order_section())),
        true,
    );
    assert!(evidence.has_model && evidence.has_recording && !evidence.controller_stages_apply);
    let tune = stage_availability(&evidence)
        .into_iter()
        .find(|item| item.stage == Stage::Tune)
        .expect("every stage");
    assert_eq!(tune.label, "Not applicable");
    assert!(study_evidence(&plant_loop(), None, false).controller_stages_apply);
}

// Specification 35, B5.2 and B4.3: the answer names the model's revision, the one Validate's model check names, and carries the
// requirements it is given.
#[test]
fn the_answer_names_the_model_revision_and_carries_the_requirements_it_is_given() {
    let section = first_order_section();
    let requirements = TuningRequirements {
        reference_change: 1.0,
        maximum_error: 0.2,
        settling_band: 0.05,
        settling_time_seconds: 20.0,
        duration_seconds: 40.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: Some(AutomaticCriteria {
            model_revision: "model:fitted:1".to_owned(),
            fields: vec!["referenceChange".to_owned()],
        }),
    };
    let result = identify_result(
        &section,
        StageEvidence::default(),
        Some(requirements.clone()),
    );
    assert_eq!(result.model_revision, "model:fitted:1");
    assert_eq!(result.requirements, Some(requirements));
    let wire = serde_json::to_value(identify_result(&section, StageEvidence::default(), None))
        .expect("the answer is written");
    assert_eq!(wire["modelRevision"], "model:fitted:1");
    assert!(
        wire.get("requirements").is_none(),
        "no requirements are written when there are none"
    );
}

// Specification 35, B4.3 and B4.7: where Automatic cannot form the test, the answer names no requirements.
#[test]
fn the_answer_names_no_requirements_where_automatic_cannot_form_them() {
    // The first-order fixture has no operating point, which Automatic needs to generate a test.
    let answer = identify_answer(
        &Study::new("tank", "sha256:loop-1"),
        &plant_loop(),
        &CurrentModel::identified(first_order_section()),
        StageEvidence::default(),
    )
    .expect("the answer");
    assert_eq!(answer.model_revision, "model:fitted:1");
    assert_eq!(answer.requirements, None);
}

// Specification 35, A8.6: a study with no Identify section holds no identification, as it does after Reset.
#[test]
fn the_study_holds_no_identification_without_an_identify_section() {
    let study = Study::new("tank", "sha256:loop-1");
    assert!(
        study_identification(&study, &plant_loop(), StageEvidence::default())
            .expect("the study reads")
            .is_none()
    );
}

// Specification 35, B4.3: the answer carries the record Tune designs to for the study's model, with Automatic's values listed.
#[test]
fn the_answer_carries_the_record_automatic_forms_for_the_study_model() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    let current =
        CurrentModel::identified(crate::study::tune::model::tests::identify_section(None));
    let evidence = study_evidence(&loop_definition, Some(&current), true);
    let answer = identify_answer(
        &Study::new(&loop_definition.id, &loop_definition.revision),
        &loop_definition,
        &current,
        evidence,
    )
    .expect("the answer");
    assert_eq!(answer.model_revision, "sha256:fopdt-model");
    // The identification leaves a model and no candidate or validation current (A8.4), and the answer says so.
    assert_eq!(answer.stage_evidence, evidence);
    assert!(answer.stage_evidence.has_model);
    assert!(!answer.stage_evidence.has_candidate);
    assert!(!answer.stage_evidence.validation_has_result);
    let requirements = answer.requirements.expect("Automatic forms the record");
    let criteria = requirements
        .automatic_criteria
        .expect("the generated values are marked");
    assert_eq!(criteria.model_revision, "sha256:fopdt-model");
    for key in REQUIREMENT_KEYS {
        assert!(
            criteria.fields.iter().any(|field| field == key),
            "{key} is marked Automatic"
        );
    }
}

// Specification 35, B4.5 and A8.6: the candidate a study keeps answers its settings and no prediction; without a model or a Tune
// section it answers none.
#[test]
fn a_kept_candidate_answers_its_settings_and_no_prediction() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop");
    let identify =
        CurrentModel::identified(crate::study::tune::model::tests::identify_section(None));
    let request = TuneRequest {
        project_root: "/projects/control-pid-commissioning".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-pid-commissioning".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: None,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    };
    let section = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &request,
        relay: None,
    })
    .expect("Automatic designs a candidate")
    .section;
    let evidence = study_evidence(&loop_definition, Some(&identify), true);
    let answer = kept_candidate(&loop_definition, Some(&identify), Some(&section), evidence)
        .expect("the study keeps a candidate");
    assert_eq!(answer.candidate, section.candidate);
    assert_eq!(
        answer.settings,
        record_settings(&section.candidate, &mapped_parameters(&loop_definition))
    );
    assert_eq!(answer.reason, section.reason);
    assert!(answer.response.is_none(), "the study keeps no prediction");
    assert!(answer.prediction_unavailable.is_some());
    assert!(matches!(
        answer.outcome,
        CandidateOutcome::Unpredicted { .. }
    ));
    assert!(answer.margins.is_none() && answer.comparison.is_empty());
    assert_eq!(answer.stages, stage_availability(&evidence));
    assert!(kept_candidate(&loop_definition, None, Some(&section), evidence).is_none());
    assert!(kept_candidate(&loop_definition, Some(&identify), None, evidence).is_none());
}
