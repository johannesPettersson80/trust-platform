//! Tune of a measured-disturbance feedforward (specification 35, C8) on lesson 84 and the heater's model: the feedforward gain, lead and lag
//! from both reduced paths, the PI on the output path for the requirements, the answer's two paths and deviations, and the request that
//! only Automatic designs. The feedforward values depend only on the paths, so their closed forms are checked whatever the PI is.

#![allow(clippy::float_cmp)] // The disturbance step is the value the request names, compared exactly.

use super::*;
use crate::messages::TuneMethod;
use crate::model::GeneralModel;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::tune_held;
use crate::tuning::objective::TuningObjective;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structures::fixtures::{heater_model, requirements};
use crate::tuning::structures::texts;

/// Lesson 84 as the shipped file declares it: the heater with its draw-off fed forward through a lead and a lag.
const LESSON_84: &str = include_str!(
    "../../../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
);

/// The sentence a refusal shows in full: its detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

fn lesson_84() -> LoopDefinition {
    LoopDefinition::decode(LESSON_84).expect("lesson 84 decodes")
}

/// The study's model: the heater, supplied rather than fitted, so it is current for every loop with its signals.
fn current(model: GeneralModel) -> CurrentModel {
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model,
        identify: None,
    }
}

/// The request of lesson 84's Tune, with the method given.
fn request(method: Option<TuneMethod>) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-capstone-independent".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-capstone-independent".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

/// The test's task: a 1 °C change, a 0.3 °C allowance at 40 s, a 0.1 °C band and an 80 s test; balanced, as the feedforward tests declare.
fn task() -> TuningRequirements {
    let mut task = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    task
}

#[test]
fn tune_gives_the_feedforward_from_both_paths_and_the_pi_on_the_output_path() {
    let loop_definition = lesson_84();
    let model = current(heater_model());
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let outcome = tune_held(&inputs, Some(&task()), None).expect("the feedforward is designed");
    let candidate = outcome
        .section
        .project_candidate
        .as_ref()
        .expect("the feedforward candidate is a program's own law");
    assert_eq!(candidate.structure, Structure::PI);
    let values = &candidate.values;
    assert!(
        (values["P1.FeedforwardGain"] - 0.5).abs() < 0.005,
        "K_ff {values:?}"
    );
    assert!((values["P1.LeadS"] - 8.0).abs() < 0.1, "T_lead {values:?}");
    assert!((values["P1.LagS"] - 3.0).abs() < 0.05, "T_lag {values:?}");
    assert!(values.contains_key("P1.Parameters.ProportionalGain"));
    assert!(
        !values.contains_key("P1.Parameters.DerivativeGain"),
        "a PI writes its derivative as zero at Apply, not as a candidate value"
    );
    assert_eq!(
        outcome.section.model_revision,
        model.model.identity().revision
    );
    assert_eq!(outcome.section.method, None);
}

#[test]
fn the_answer_shows_both_paths_the_three_values_and_the_deviations_with_the_feedforward_smaller() {
    let loop_definition = lesson_84();
    let model = current(heater_model());
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let outcome = tune_held(&inputs, Some(&task()), None).expect("the feedforward is designed");
    let answer = outcome
        .result
        .feedforward
        .as_ref()
        .expect("a feedforward answer");
    assert!(
        (answer.output_path.gain - 0.2).abs() < 0.002,
        "K_u {}",
        answer.output_path.gain
    );
    assert!(
        (answer.disturbance_path.gain + 0.1).abs() < 0.001,
        "K_d {}",
        answer.disturbance_path.gain
    );
    assert_eq!(answer.disturbance_step, 10.0);
    assert!(
        answer.feedforward_deviation < answer.feedback_deviation,
        "{answer:?}"
    );
}

#[test]
fn a_model_answer_states_each_gain_unit() {
    // C8: the page showed the gains with units it composed from the loop's signals (and with none when two units were the same). The
    // answer states them: the heater's output path gives degC per % (the water's temperature per the heater's output), its disturbance
    // path degC per kW, and the feedforward gain K_ff is % per kW, the manipulated variable's unit per the disturbance's. The disturbance
    // and the measurement are in kW and degC.
    let loop_definition = lesson_84();
    let model = current(heater_model());
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let outcome = tune_held(&inputs, Some(&task()), None).expect("the feedforward is designed");
    let answer = outcome
        .result
        .feedforward
        .as_ref()
        .expect("a feedforward answer");
    assert_eq!(answer.output_path.gain_unit.as_deref(), Some("degC/%"));
    assert_eq!(
        answer.disturbance_path.gain_unit.as_deref(),
        Some("degC/kW")
    );
    assert_eq!(answer.gain_unit.as_deref(), Some("%/kW"));
    assert_eq!(answer.disturbance_unit.as_deref(), Some("kW"));
    assert_eq!(answer.measurement_unit.as_deref(), Some("degC"));
}

#[test]
fn a_named_method_is_refused_because_only_automatic_designs_a_feedforward() {
    let loop_definition = lesson_84();
    let model = current(heater_model());
    let request = request(Some(TuneMethod::Simc));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let failure = tune_held(&inputs, Some(&task()), None)
        .expect_err("a named method does not design a feedforward");
    assert_eq!(text_of(&failure), texts::FEEDFORWARD_AUTOMATIC);
}

#[test]
fn the_candidate_carries_the_margins_of_the_pi_on_the_output_path() {
    // control-studio-feedforward: "a feedforward candidate carries the PI's margins on the output's path" (specification 35, C0 "Margins
    // of linear candidates" and C8: the feedforward acts outside the loop, so the margins are those of the PI on the output path). A PI
    // on 0.2/(8·s + 1) has no phase crossover: its gain margin is unbounded, its phase margin finite and its peak sensitivity at least one.
    use trust_control_math::margins::MarginValue;
    let loop_definition = lesson_84();
    let model = current(heater_model());
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let outcome = tune_held(&inputs, Some(&task()), None).expect("the feedforward is designed");
    let margins = outcome
        .result
        .margins
        .as_ref()
        .expect("the candidate has margins on the output path");
    assert_eq!(margins.gain_margin_db, MarginValue::Unbounded);
    assert!(
        matches!(margins.phase_margin_degrees, MarginValue::Finite(degrees) if degrees > 0.0),
        "{:?}",
        margins.phase_margin_degrees
    );
    assert!(
        margins.peak_sensitivity >= 1.0 - 1e-9,
        "{}",
        margins.peak_sensitivity
    );
    assert_eq!(outcome.result.margins_unavailable, None);
}
