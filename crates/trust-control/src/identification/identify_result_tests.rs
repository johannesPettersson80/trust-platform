#![allow(clippy::float_cmp)] // Plotted times and values are copied, so compared exactly.

//! Tests of the result of a stored identification (specification 35, A8.4 and B3.8; Identify page specification, "Identified
//! model and plot").

use std::collections::BTreeMap;

use serde_json::json;
use trust_control_math::identify::LowOrderModelKind;

use super::*;
use crate::identification::evidence::tests::fitted_model;
use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint, OperatingRange};
use crate::identification::experiment_trace::{IdentificationExperiment, TracePoint};
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::plant::IdentifiedModel;
use crate::identification::presentation::identified_model_presentation;
use crate::identification::project_test::SETTINGS_RELEASED;
use crate::loop_definition::{Direction, LoopSignal, Range};
use crate::messages::TuneRequest;
use crate::model::decode_general_model;
use crate::recording::channels::ChannelQuality;
use crate::recording::fixtures::{binding, loop_with_disturbance, run, sample};
use crate::recording::EvidenceSource;
use crate::recording::RecordedSample;
use crate::signals::{Quality, SignalBinding};
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
        model_structure: None,
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

/// The presentation a fit keeps (B10.2): the tests change what the page shows through it.
fn presentation_mut(section: &mut IdentifySection) -> &mut ComparisonPresentation {
    section
        .presentation
        .as_mut()
        .expect("a fit keeps its presentation")
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
fn each_identify_answer_states_its_status_sentence() {
    // B3.5, B3.8, B9.9: the Identify page's status line is the engine's sentence for the model it shows, and the page writes none. A model
    // that is not provisional is "Model identified."; a provisional one says so. The sentences are the specification's words.
    let provisional = identify_result(&first_order_section(), StageEvidence::default(), None);
    assert!(provisional.provisional);
    assert_eq!(provisional.status_sentence, "Provisional model identified.");
    let validated = identify_result(
        &IdentifySection {
            provisional: false,
            ..first_order_section()
        },
        StageEvidence::default(),
        None,
    );
    assert!(!validated.provisional);
    assert_eq!(validated.status_sentence, "Model identified.");

    // A supplied model is answered the same way, by its validation: unvalidated is provisional (B3.5, B3.7).
    let supplied = |validation: &str| {
        let model = decode_general_model(&json!({
            "kind": "transfer-function", "revision": "model:provided:1", "source": "provided",
            "validation": validation, "evidence": ["sha256:recording"], "delaySeconds": 0.0,
            "inputs": [{ "id": "command", "unit": "V", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
            "outputs": [{ "id": "position", "unit": "m", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
            "numerator": [0.5], "denominator": [2.0, 1.0]
        }))
        .expect("a supplied model");
        supplied_result(&model, StageEvidence::default(), None)
    };
    assert_eq!(
        supplied("unvalidated").status_sentence,
        "Provisional model identified."
    );
    assert_eq!(supplied("validated").status_sentence, "Model identified.");
    let wire = serde_json::to_value(&provisional).expect("the answer is written");
    assert_eq!(wire["statusSentence"], "Provisional model identified.");
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
    // first answer and of `controlStudio/open` alike (`read_channels`). Its prediction carries the same name, not the model's id.
    let mut section = first_order_section();
    presentation_mut(&mut section).experiment = None;
    section.recorded_channels = Some(IdentifySignals {
        inputs: vec!["Plant.Command".to_owned()],
        outputs: vec!["Plant.Recorded".to_owned()],
        setpoint: None,
    });
    let named = |section: &IdentifySection| -> Vec<(PlotRole, String)> {
        identify_result(section, StageEvidence::default(), None)
            .plot
            .traces
            .into_iter()
            .map(|trace| (trace.role, trace.symbol))
            .collect()
    };
    assert_eq!(
        named(&section),
        [
            (PlotRole::Measurement, "Plant.Recorded".to_owned()),
            (PlotRole::Prediction, "Plant.Recorded".to_owned()),
        ],
        "both traces of the fit carry the recorded channel's name"
    );
    section.recorded_channels = None;
    assert_eq!(
        named(&section),
        [
            (PlotRole::Measurement, "Plant.Position".to_owned()),
            (PlotRole::Prediction, "Plant.Position".to_owned()),
        ],
        "a fit of the loop's own channel names both traces by it"
    );
}

#[test]
fn a_fit_that_read_no_output_channel_names_both_traces_by_the_model_output() {
    // The fallback of the name: the model's output, when the fit read no measured channel.
    let mut section = first_order_section();
    presentation_mut(&mut section).experiment = None;
    presentation_mut(&mut section).output_id = None;
    section.recorded_channels = Some(IdentifySignals {
        inputs: vec!["Plant.Command".to_owned()],
        outputs: Vec::new(),
        setpoint: None,
    });
    let plot = identify_result(&section, StageEvidence::default(), None).plot;
    assert!(
        plot.traces.iter().all(|trace| trace.symbol == "position"),
        "{:?}",
        plot.traces
    );
}

#[test]
fn without_the_experiments_traces_the_plot_shows_the_measured_values_of_the_fit() {
    let mut section = first_order_section();
    presentation_mut(&mut section).experiment = None;
    presentation_mut(&mut section).output_id = None;
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
                "Plant.Position",
                "m",
                &[Some(0.0), Some(0.19), Some(0.39)]
            ),
        ],
        "the prediction carries the measured trace's name: the loop's channel the fit read"
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
        channel_requirements: None,
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

/// The state model of a PID group's two heaters and two temperatures that its campaign fitted from one recording (C7): discrete,
/// sample time 0.1 s, two states, no delay.
fn group_state_model() -> GeneralModel {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": 0.0, "maximum": 100.0 } });
    decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "model:fitted:group", "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:estimation"], "delaySeconds": 0.0,
        "sampleTimeSeconds": 0.1,
        "inputs": [channel("heater-1", "%"), channel("heater-2", "%")],
        "outputs": [channel("vessel-1", "°C"), channel("vessel-2", "°C")],
        "states": [channel("x1", "°C"), channel("x2", "°C")],
        "a": [[0.9, 0.0], [0.0, 0.8]], "b": [[0.1, 0.02], [0.03, 0.2]],
        "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]]
    }))
    .expect("a group state model")
}

// Specification 35, C7, B10.2 and A8.3: a PID group's campaign keeps a state model of its channels without a comparison presentation, and
// the model was fitted from the campaign's recording. Its answer names that recording, as every fit's does, and is provisional, as a fit
// from one recording is; it is shown in a state model's words, with no plot. Catches the state model answered as a supplied model, which
// names no recording, so the page can neither show nor fit again the recording both steps were fitted from.
#[test]
fn a_state_model_fit_names_the_recording_it_was_fitted_from() {
    let section = IdentifySection::new_state_model(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        IdentifySignals {
            inputs: vec!["P1.Heater".into(), "P2.Heater".into()],
            outputs: vec!["P1.Temperature".into(), "P2.Temperature".into()],
            setpoint: None,
        },
        RECORDING_ID,
        group_state_model(),
    );
    let result = identify_result(&section, StageEvidence::default(), None);
    assert_eq!(result.recording_id.as_deref(), Some(RECORDING_ID));
    assert!(result.provisional);
    assert_eq!(
        result.model_type,
        "State-space model, discrete time, sample time 0.1 s"
    );
    assert_eq!(
        result.equation,
        "x(k+1) = A x(k) + B u(k), y(k) = C x(k) + D u(k), with 2 states"
    );
    assert_eq!(result.model_revision, "model:fitted:group");
    assert!(
        result.plot.time_seconds.is_empty() && result.plot.traces.is_empty(),
        "a state model keeps no comparison presentation to plot (B10.2)"
    );
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

// Specification 35, C12 and B3.5: a project test whose fit refuses finishes with its recording and its restoration, and the refusal as
// its note. The study keeps no model, so the answer names none and says that the study has no model.
#[test]
fn a_project_test_answer_names_no_model_and_carries_the_refusal_as_its_note() {
    let loop_definition = plant_loop();
    let answer = project_test_result(
        &loop_definition,
        &Study::new(&loop_definition.id, &loop_definition.revision),
        RECORDING_ID,
        Some(CampaignRestoration::Restored),
        Vec::new(),
        "The recording is missing a nonzero step.".to_string(),
    )
    .expect("the answer");
    assert_eq!(answer.recording_id, RECORDING_ID);
    assert_eq!(answer.restoration, Some(CampaignRestoration::Restored));
    assert_eq!(
        answer.settings_released.as_deref(),
        Some(SETTINGS_RELEASED),
        "a confirmed restoration releases the experiment settings (C12)"
    );
    assert_eq!(
        answer.fit_refusal,
        "The recording is missing a nonzero step."
    );
    assert!(!answer.stage_evidence.has_model, "no model is kept");
    assert!(answer.stage_evidence.has_recording, "the recording is kept");
    let json = serde_json::to_value(&answer).expect("the answer as JSON");
    assert_eq!(
        json["fitRefusal"],
        "The recording is missing a nonzero step."
    );
    assert_eq!(json["recordedSamples"], json!([]));
    assert_eq!(json["settingsReleased"], json!(SETTINGS_RELEASED));
    assert!(json.get("modelType").is_none(), "{json}");
}

// Specification 35, C12: the experiment settings' ownership is released only after the terminal restoration, so a test that kept no
// restoration says nothing of releasing them.
#[test]
fn a_project_test_without_a_restoration_does_not_release_the_settings() {
    let loop_definition = plant_loop();
    let answer = project_test_result(
        &loop_definition,
        &Study::new(&loop_definition.id, &loop_definition.revision),
        RECORDING_ID,
        None,
        Vec::new(),
        "The recording is missing a nonzero step.".to_string(),
    )
    .expect("the answer");
    assert_eq!(answer.settings_released, None);
    let json = serde_json::to_value(&answer).expect("the answer as JSON");
    assert!(json.get("settingsReleased").is_none(), "{json}");
}

// Specification 35, C12: a finished project test says how it ended in the engine's words: "Project test completed.", with "Outputs
// restored." only when the runtime confirmed the restoration of what it wrote. A test that wrote nothing to restore, or kept no
// restoration, says only that it completed; a page that composed the sentence itself would say more than the engine confirmed.
#[test]
fn project_test_result_states_its_outcome_sentence() {
    let loop_definition = plant_loop();
    let outcome = |restoration: Option<CampaignRestoration>| {
        project_test_result(
            &loop_definition,
            &Study::new(&loop_definition.id, &loop_definition.revision),
            RECORDING_ID,
            restoration,
            Vec::new(),
            "The recording is missing a nonzero step.".to_string(),
        )
        .expect("the answer")
        .test_outcome
    };
    assert_eq!(
        outcome(Some(CampaignRestoration::Restored)).as_deref(),
        Some("Project test completed. Outputs restored.")
    );
    assert_eq!(
        outcome(Some(CampaignRestoration::NotRequired)).as_deref(),
        Some("Project test completed.")
    );
    assert_eq!(outcome(None).as_deref(), Some("Project test completed."));
    let answer = project_test_result(
        &loop_definition,
        &Study::new(&loop_definition.id, &loop_definition.revision),
        RECORDING_ID,
        Some(CampaignRestoration::Restored),
        Vec::new(),
        "The recording is missing a nonzero step.".to_string(),
    )
    .expect("the answer");
    let json = serde_json::to_value(&answer).expect("the answer as JSON");
    assert_eq!(
        json["testOutcome"],
        json!("Project test completed. Outputs restored.")
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
        pins: None,
    })
    .expect("Automatic designs a candidate")
    .section;
    let evidence = study_evidence(&loop_definition, Some(&identify), true);
    let answer = kept_candidate(&loop_definition, Some(&identify), Some(&section), evidence)
        .expect("the study keeps a candidate");
    assert_eq!(answer.candidate, section.candidate);
    assert_eq!(
        answer.settings,
        candidate_settings(
            &section
                .law_candidate()
                .expect("the section keeps its candidate"),
            &mapped_parameters(&loop_definition),
        )
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

#[test]
fn a_kept_recording_is_drawn_as_one_chart_row_per_sample_with_each_signal_in_its_own_place() {
    // The feed flow is the additional signal of the fixture loop; the setpoint, the measurement and the output are its channels.
    let loop_definition = loop_with_disturbance();
    let bindings = vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
        binding("feed-flow", "%QL8", "LREAL"),
    ];
    let good = [Quality::Good; 4];
    let samples = vec![
        sample(
            7,
            Some(1_000_000_000),
            &[Some(50.0), Some(49.0), Some(30.0), Some(2.0)],
            &good,
        ),
        sample(
            8,
            Some(1_100_000_000),
            &[Some(50.0), Some(49.5), Some(31.0), None],
            &good,
        ),
    ];
    let rows = recorded_chart_samples(&loop_definition, &run(bindings, samples));
    assert_eq!(rows.len(), 2, "one row per recorded sample");
    assert_eq!(rows[0].scan, 7);
    assert_eq!(
        rows[0].elapsed_seconds, 0.0,
        "the first sample is the origin of the process time (A3.2)"
    );
    assert_eq!(
        rows[1].elapsed_seconds, 0.1,
        "the next sample is 0.1 s later"
    );
    assert_eq!((rows[0].sp, rows[0].pv), (Some(50.0), Some(49.0)));
    assert_eq!(
        rows[0].applied_mv,
        Some(30.0),
        "without an actuator the output the process received is the manipulated variable"
    );
    assert_eq!(
        rows[0].requested_mv, None,
        "a loop without an actuator has no separate request"
    );
    assert_eq!(rows[0].additional[0].id, "feed-flow");
    assert_eq!(rows[0].additional[0].value, Some(2.0));
    assert_eq!(rows[0].quality, ChannelQuality::Good);
    assert_eq!(
        rows[1].additional[0].value, None,
        "a missing value stays missing; it is not interpolated (B10.3)"
    );
    assert_eq!(rows[1].quality, ChannelQuality::Bad);
}

/// The values a sample's plot draws, by identifier, as a map: `None` is a gap.
fn drawn(values: &[(&str, Option<f64>)]) -> BTreeMap<String, Option<f64>> {
    values
        .iter()
        .map(|(id, value)| ((*id).to_owned(), *value))
        .collect()
}

/// The bindings of the fixture loop with a disturbance, in the order of its samples: setpoint, measurement, output and feed flow.
fn four_bindings() -> Vec<SignalBinding> {
    vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
        binding("feed-flow", "%QL8", "LREAL"),
    ]
}

/// A sample of those four bindings at `scan`, 0.1 s after the sample before it.
fn four(scan: u64, values: [Option<f64>; 4], quality: [Quality; 4]) -> RecordedSample {
    let time = i64::try_from(scan).expect("a small scan") * 100_000_000;
    sample(scan, Some(time), &values, &quality)
}

#[test]
fn recorded_chart_samples_name_each_value_by_its_signal_id() {
    // Wrong behaviour caught: the page mapping each signal onto a field of the sample (`sp`, `pv`, `appliedMv`, `requestedMv`,
    // `additional`). With an actuator the sample names the output the process received `mv` (the actuator's position, 28.5) and the
    // command the controller asked for `requestedMv` (30), apart from the setpoint, the measurement and the feed flow (B6.1, B9.6).
    let mut loop_definition = loop_with_disturbance();
    loop_definition.signals.actuator = Some(LoopSignal {
        symbol: "Valve.Position".to_owned(),
        address: Some("%IL4".to_owned()),
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    });
    let mut bindings = four_bindings();
    bindings.insert(3, binding("actuator", "%IL4", "LREAL"));
    let samples = vec![sample(
        7,
        Some(1_000_000_000),
        &[Some(50.0), Some(49.0), Some(30.0), Some(28.5), Some(2.0)],
        &[Quality::Good; 5],
    )];
    let rows = recorded_chart_samples(&loop_definition, &run(bindings, samples));
    assert_eq!(
        rows[0].plotted,
        drawn(&[
            ("sp", Some(50.0)),
            ("pv", Some(49.0)),
            ("mv", Some(28.5)),
            ("requestedMv", Some(30.0)),
            ("feed-flow", Some(2.0)),
        ])
    );

    // Without an actuator the manipulated variable is the output the process received, and there is no separate request.
    let samples = vec![four(
        7,
        [Some(50.0), Some(49.0), Some(30.0), Some(2.0)],
        [Quality::Good; 4],
    )];
    let rows = recorded_chart_samples(&loop_with_disturbance(), &run(four_bindings(), samples));
    assert_eq!(
        rows[0].plotted,
        drawn(&[
            ("sp", Some(50.0)),
            ("pv", Some(49.0)),
            ("mv", Some(30.0)),
            ("feed-flow", Some(2.0)),
        ])
    );
}

#[test]
fn recorded_chart_samples_mark_the_samples_to_plot() {
    // Wrong behaviour caught: the page drawing a value only when the whole sample is good (a bad feed flow then removed the good
    // measurement) or drawing a forced or missing value. B9.6: a missing, bad or forced value is a gap in its own trace and removes no
    // good value of another signal; B10.3: a missing value stays missing and is not interpolated.
    use Quality::{Forced, Good, Missing};
    let mut measurement_forced = four(
        3,
        [Some(50.0), Some(49.7), Some(32.0), Some(2.0)],
        [Good, Forced, Good, Good],
    );
    measurement_forced.forced[1] = true;
    let samples = vec![
        four(
            1,
            [Some(50.0), Some(49.0), Some(30.0), Some(2.0)],
            [Good; 4],
        ),
        four(
            2,
            [Some(50.0), Some(49.5), Some(31.0), None],
            [Good, Good, Good, Missing],
        ),
        measurement_forced,
        four(
            4,
            [None, Some(49.8), Some(33.0), Some(2.0)],
            [Missing, Good, Good, Good],
        ),
        // The runtime reports the setpoint's quality forced while its forced flag stays down: still not a good value (E4.4).
        four(
            5,
            [Some(50.0), Some(49.9), Some(34.0), Some(2.0)],
            [Forced, Good, Good, Good],
        ),
    ];
    let rows = recorded_chart_samples(&loop_with_disturbance(), &run(four_bindings(), samples));
    let row = |values: [Option<f64>; 4]| {
        drawn(&[
            ("sp", values[0]),
            ("pv", values[1]),
            ("mv", values[2]),
            ("feed-flow", values[3]),
        ])
    };
    assert_eq!(
        rows[0].plotted,
        row([Some(50.0), Some(49.0), Some(30.0), Some(2.0)]),
        "a good sample draws every value"
    );
    assert_eq!(
        rows[1].plotted,
        row([Some(50.0), Some(49.5), Some(31.0), None]),
        "the missing feed flow is a gap in its own trace; the good setpoint, measurement and output stay"
    );
    assert_eq!(
        rows[1].quality,
        ChannelQuality::Bad,
        "the sample as a whole is still bad: the CSV export and the baseline noise read that quality"
    );
    assert_eq!(
        rows[2].plotted,
        row([Some(50.0), None, Some(32.0), Some(2.0)]),
        "a forced measurement is a gap in its own trace"
    );
    assert_eq!(
        rows[3].plotted,
        row([None, Some(49.8), Some(33.0), Some(2.0)]),
        "a missing setpoint is a gap in its own trace and is not copied from the measurement"
    );
    assert_eq!(
        rows[4].plotted,
        row([None, Some(49.9), Some(34.0), Some(2.0)]),
        "a value the runtime reports as forced is a gap, whatever its flag says"
    );
}
