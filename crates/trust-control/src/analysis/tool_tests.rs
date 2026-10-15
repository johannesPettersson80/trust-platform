//! The catalogue of the analysis tools (B9.9, draft §2.2): which tools apply, the page's words, the defaults and the inputs read.

use std::f64::consts::PI;

use serde_json::{json, Value};

use super::{
    applies, channel_point_budget, default_grid, default_lead_lag_value, defaults_for, no_model,
    not_applicable, number, offers, presentation, read_inputs, Facts,
};
use crate::analysis::lead_lag::LeadLagMode;
use crate::failure::{Cause, NextAction};
use crate::messages::{AnalysisInputs, AnalysisTool, PoleZeroView};
use crate::model::{decode_general_model, GeneralModel, ResponseConnection};

/// A channel with the unit `u` and a wide operating range.
fn channel(id: &str) -> Value {
    json!({ "id": id, "unit": "u", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// The fields every model carries, with the given channels.
fn identity(kind: &str, inputs: &[&str], outputs: &[&str]) -> Value {
    json!({
        "kind": kind, "revision": "r1", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": inputs.iter().map(|id| channel(id)).collect::<Vec<_>>(),
        "outputs": outputs.iter().map(|id| channel(id)).collect::<Vec<_>>(),
    })
}

/// A decoded model from its identity and its dynamics, which are added to the identity.
fn model(mut value: Value, dynamics: Value) -> GeneralModel {
    let object = value.as_object_mut().expect("an object");
    let Value::Object(entries) = dynamics else {
        panic!("the dynamics are an object")
    };
    object.extend(entries);
    decode_general_model(&value).expect("a valid model")
}

/// A continuous single-input, single-output state-space model with two states.
fn siso_state_space() -> GeneralModel {
    model(
        identity("continuous-state-space", &["u"], &["y"]),
        json!({
            "states": [channel("x1"), channel("x2")],
            "a": [[0.0, 1.0], [-2.0, -3.0]], "b": [[0.0], [1.0]], "c": [[1.0, 0.0]], "d": [[0.0]],
        }),
    )
}

/// A continuous two-input, two-output state-space model with two states.
fn rectangular_state_space() -> GeneralModel {
    model(
        identity("continuous-state-space", &["u1", "u2"], &["y1", "y2"]),
        json!({
            "states": [channel("x1"), channel("x2")],
            "a": [[-1.0, 0.0], [0.0, -2.0]], "b": [[1.0, 0.0], [0.0, 1.0]],
            "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]],
        }),
    )
}

/// A continuous single-input, single-output transfer function `1 / (s + 2)` with a delay of one second.
fn delayed_transfer() -> GeneralModel {
    let mut value = identity("transfer-function", &["u"], &["y"]);
    value["delaySeconds"] = json!(1.0);
    model(
        value,
        json!({ "numerator": [1.0], "denominator": [1.0, 2.0] }),
    )
}

/// The facts of a model with no controller, no declared analysis and no recording.
fn facts(model: Option<&GeneralModel>) -> Facts<'_> {
    Facts {
        model,
        controller: None,
        declared: None,
        recordings: &[],
        multichannel: false,
    }
}

fn tools_of(facts: &Facts<'_>) -> Vec<AnalysisTool> {
    offers(facts).into_iter().map(|offer| offer.tool).collect()
}

#[test]
fn the_tools_that_apply_to_a_siso_state_space_model_are_offered_in_the_page_order() {
    let model = siso_state_space();
    // Without a recording, a declared analysis or a controller, the measured response and the feedback do not apply, and the
    // root locus and Routh-Hurwitz need the declared analysis of the loop file.
    assert_eq!(
        tools_of(&facts(Some(&model))),
        vec![
            AnalysisTool::PoleZero,
            AnalysisTool::Frequency,
            AnalysisTool::LeadLag,
            AnalysisTool::InitialState,
            AnalysisTool::StateStructure,
        ]
    );
    let recordings = ["rec-1".to_owned()];
    let with_recording = Facts {
        recordings: &recordings,
        ..facts(Some(&model))
    };
    assert!(applies(AnalysisTool::MeasuredFrequency, &with_recording));
    assert!(!applies(AnalysisTool::RelativeGain, &with_recording));
}

#[test]
fn measured_paths_apply_to_a_square_multichannel_loop_with_a_recording() {
    let model = siso_state_space();
    let recordings = ["rec-1".to_owned()];
    let multichannel = Facts {
        recordings: &recordings,
        multichannel: true,
        ..facts(Some(&model))
    };
    assert!(applies(AnalysisTool::MeasuredPaths, &multichannel));
    let single_loop = Facts {
        multichannel: false,
        ..multichannel
    };
    assert!(!applies(AnalysisTool::MeasuredPaths, &single_loop));
    assert!(!applies(AnalysisTool::MeasuredPaths, &facts(Some(&model))));
    assert_eq!(
        presentation(AnalysisTool::MeasuredPaths).action,
        "Calculate measured paths"
    );
    assert_eq!(
        defaults_for(AnalysisTool::MeasuredPaths, &multichannel),
        AnalysisInputs::default()
    );
}

#[test]
fn a_rectangular_state_space_model_offers_the_relative_gain_and_no_lead_lag() {
    let model = rectangular_state_space();
    let tools = tools_of(&facts(Some(&model)));
    assert!(tools.contains(&AnalysisTool::RelativeGain));
    assert!(tools.contains(&AnalysisTool::StateStructure));
    assert!(tools.contains(&AnalysisTool::Frequency));
    assert!(
        !tools.contains(&AnalysisTool::LeadLag),
        "a lead/lag design needs one input and one output"
    );
}

#[test]
fn a_transfer_function_has_no_structure_and_no_relative_gain_but_takes_a_lead_lag() {
    let model = delayed_transfer();
    let tools = tools_of(&facts(Some(&model)));
    assert!(tools.contains(&AnalysisTool::LeadLag));
    assert!(!tools.contains(&AnalysisTool::StateStructure));
    assert!(!tools.contains(&AnalysisTool::RelativeGain));
}

#[test]
fn without_a_model_a_declared_analysis_or_a_recording_no_tool_applies() {
    assert_eq!(tools_of(&facts(None)).len(), 0);
    assert!(!applies(AnalysisTool::Feedback, &facts(None)));
    assert!(!applies(AnalysisTool::RootLocus, &facts(None)));
}

#[test]
fn the_words_of_each_tool_are_the_words_the_page_showed() {
    // The labels, the action texts and the abort text of `threePageWorkflow.ts` and `threePageAnalysis.ts`.
    let frequency = presentation(AnalysisTool::Frequency);
    assert_eq!(frequency.label, "Bode (frequency response)");
    assert_eq!(frequency.action, "Calculate frequency response");
    assert_eq!(frequency.abort_action, Some("Abort analysis"));
    assert!(frequency.operation);
    let lead_lag = presentation(AnalysisTool::LeadLag);
    assert_eq!(lead_lag.label, "Lead/lag design");
    assert_eq!(lead_lag.action, "Calculate lead/lag design");
    assert_eq!(
        lead_lag.abort_action, None,
        "a quick answer has no abort text"
    );
    assert!(!lead_lag.operation);
    assert_eq!(
        presentation(AnalysisTool::PoleZero).action,
        "Calculate poles and zeros"
    );
    assert_eq!(
        presentation(AnalysisTool::StateStructure).label,
        "Controllability and observability"
    );
    assert_eq!(
        presentation(AnalysisTool::RelativeGain).action,
        "Calculate relative gain array"
    );
    assert_eq!(
        presentation(AnalysisTool::Routh).action,
        "Calculate Routh–Hurwitz"
    );
}

#[test]
fn an_offer_carries_its_words_its_operation_flag_and_its_defaults() {
    let model = siso_state_space();
    let offer = offers(&facts(Some(&model)))
        .into_iter()
        .find(|offer| offer.tool == AnalysisTool::Frequency)
        .expect("the frequency tool is offered");
    assert_eq!(offer.label, "Bode (frequency response)");
    assert!(offer.operation);
    assert_eq!(offer.abort_action.as_deref(), Some("Abort analysis"));
    let grid = offer.defaults.grid.expect("a default grid");
    assert!((grid.minimum_frequency - 0.1).abs() < 1e-12);
    assert!((grid.maximum_frequency - 100.0).abs() < 1e-12);
    assert_eq!(grid.points, 128);
}

#[test]
fn the_default_grid_is_the_bode_default_and_a_discrete_model_stops_below_its_nyquist_frequency() {
    let continuous = siso_state_space();
    let grid = default_grid(&continuous);
    assert_eq!(grid.points, 128);
    assert!((grid.maximum_frequency - 100.0).abs() < 1e-12);

    // A discrete model with a sample time of 0.1 s: the Nyquist frequency is π/Ts = 31.4 rad/s, and the default maximum is
    // 0.5·π/Ts (at most 100 rad/s), which lies below it.
    let discrete = model(
        identity("discrete-state-space", &["u"], &["y"]),
        json!({
            "states": [channel("x1")], "a": [[0.5]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
            "sampleTimeSeconds": 0.1,
        }),
    );
    let grid = default_grid(&discrete);
    assert!((grid.maximum_frequency - 0.5 * PI / 0.1).abs() < 1e-9);
    assert!(grid.maximum_frequency < PI / 0.1);
}

#[test]
fn the_default_points_are_lowered_when_the_channel_limit_needs_it() {
    // 16,384 channel-frequency points over 256 channels leave 64 points, fewer than the 128 of the Bode default (A7).
    assert_eq!(channel_point_budget(256), 64);
    assert_eq!(channel_point_budget(1), 16_384);
    assert_eq!(
        channel_point_budget(0),
        16_384,
        "an empty count is one channel"
    );
}

#[test]
fn a_lead_lag_takes_thirty_degrees_for_a_lead_and_a_ratio_of_five_for_a_lag() {
    assert!((default_lead_lag_value(LeadLagMode::Lead) - 30.0).abs() < 1e-12);
    assert!((default_lead_lag_value(LeadLagMode::Lag) - 5.0).abs() < 1e-12);
    let model = delayed_transfer();
    let facts = facts(Some(&model));
    let lag = read_inputs(
        AnalysisTool::LeadLag,
        &AnalysisInputs {
            mode: Some(LeadLagMode::Lag),
            ..AnalysisInputs::default()
        },
        &facts,
    );
    assert_eq!(lag.mode, Some(LeadLagMode::Lag));
    assert_eq!(lag.value, Some(5.0), "the lag's default is the ratio 5");
    let given = read_inputs(
        AnalysisTool::LeadLag,
        &AnalysisInputs {
            value: Some(12.0),
            ..AnalysisInputs::default()
        },
        &facts,
    );
    assert_eq!(given.mode, Some(LeadLagMode::Lead));
    assert_eq!(given.value, Some(12.0), "a given value is kept");
    assert_eq!(given.crossover_rad_per_second, Some(1.0));
}

#[test]
fn the_inputs_read_keep_only_the_fields_of_their_tool_and_fill_the_defaults() {
    let model = siso_state_space();
    let facts = facts(Some(&model));
    let given = AnalysisInputs {
        view: Some(PoleZeroView::Minimal),
        selected_value: Some(3.0),
        ..AnalysisInputs::default()
    };
    let read = read_inputs(AnalysisTool::PoleZero, &given, &facts);
    assert_eq!(read.view, Some(PoleZeroView::Minimal));
    assert_eq!(
        read.selected_value, None,
        "a field of another tool is not read"
    );
    let initial = read_inputs(
        AnalysisTool::InitialState,
        &AnalysisInputs::default(),
        &facts,
    );
    assert_eq!(initial.step_seconds, Some(0.1));
    assert_eq!(initial.duration_seconds, Some(10.0));
    assert_eq!(initial.initial_state, Some(vec![0.0, 0.0]));
    assert_eq!(initial.input, Some(vec![0.0]));
    assert_eq!(initial.connection, Some(ResponseConnection::OpenLoop));
    let defaults = defaults_for(AnalysisTool::MeasuredFrequency, &facts);
    assert_eq!(defaults.frequencies, Some(vec![1.0, 2.0, 4.0]));
    assert_eq!(defaults.segment_count, Some(4));
    assert_eq!(defaults.startup_samples, Some(16));
    assert_eq!(defaults.recording_id, None, "a recording is always named");
}

#[test]
fn a_number_in_a_sentence_has_four_decimals_at_most_and_a_power_of_ten_for_small_or_large_values() {
    assert_eq!(number(1.5), "1.5");
    assert_eq!(number(2.0), "2");
    assert_eq!(number(0.0), "0");
    assert_eq!(number(-0.0), "0");
    assert_eq!(number(-0.25), "-0.25");
    assert_eq!(number(1.234_567_8), "1.2346");
    assert_eq!(number(0.0001), "1.000e-4");
    assert_eq!(number(1_234_567.0), "1.235e6");
    assert_eq!(number(f64::INFINITY), "inf");
}

#[test]
fn a_missing_model_and_a_tool_that_does_not_apply_are_named_failures_with_their_next_action() {
    let missing = no_model();
    assert_eq!(missing.next_action, NextAction::Identify);
    assert_eq!(missing.cause, Cause::ModelOrData);
    let refused = not_applicable(AnalysisTool::Frequency);
    assert_eq!(refused.next_action, NextAction::ChooseMethod);
    assert_eq!(
        refused.message,
        "Bode (frequency response) does not apply to this model or loop."
    );
}
