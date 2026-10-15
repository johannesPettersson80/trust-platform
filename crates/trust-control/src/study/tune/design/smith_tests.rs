//! Tune of a Smith predictor (specification 35, C9) on the heater of the predictor's tests: the model's gain, time constant and delay from
//! the two-point reduction (`K = 0.2`, `T = 5`, `θ = 1`), the PI as the candidate's gains with the model values beside them, and the refusals
//! of a settling time no longer than the delay and of a named method. The model values are closed forms of the model, so they are checked
//! whatever the PI is; the PI's signs follow the `TypeScript` test's (`control-studio-smith-predictor.test.ts`).

#![allow(clippy::float_cmp)] // The disturbance-free request names exact values; the candidate's structure is compared exactly.

use super::*;
use crate::commissioning::fixture_loops::smith_predictor_loop;
use crate::loop_definition::LoopDefinition;
use crate::messages::TuneMethod;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::tune_held;
use crate::tuning::objective::TuningObjective;
use crate::tuning::structures::fixtures::{requirements, smith_heater_model};

/// The sentence a refusal shows in full: its detail when it says more than its first sentence.
fn text_of(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The loop of the predictor's tests: the fixture's loop with the heater's measurement unit, `degC` (the model's output unit).
fn heater_loop() -> LoopDefinition {
    let mut loop_definition = smith_predictor_loop();
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition
}

/// The study's model: the heater, supplied rather than fitted, so it is current for every loop.
fn current() -> CurrentModel {
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model: smith_heater_model(),
        identify: None,
    }
}

/// The request of the Smith predictor's Tune, with the method given.
fn request(method: Option<TuneMethod>) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/smith".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "loop:tank".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

/// The test's task: a 0.5 °C change, a 0.1 °C allowance, a 0.05 °C band, a 10 s settling time and a 30 s test; balanced, as declared.
fn task(settling_time_seconds: f64, duration_seconds: f64) -> TuningRequirements {
    let mut task = requirements(0.5, 0.1, 0.05, settling_time_seconds, duration_seconds);
    task.tuning_objective = Some(TuningObjective::Balanced);
    task
}

#[test]
fn tune_gives_the_model_values_and_the_pi_as_the_candidate() {
    let loop_definition = heater_loop();
    let model = current();
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
    };
    let outcome =
        tune_held(&inputs, Some(&task(10.0, 30.0)), None).expect("the Smith predictor is designed");
    let candidate = outcome
        .section
        .project_candidate
        .as_ref()
        .expect("the predictor's candidate is a program's own law");
    assert_eq!(candidate.structure, Structure::PI);
    let values = &candidate.values;
    assert!(
        (values["Main.Program.ModelGain"] - 0.2).abs() < 0.002,
        "K {values:?}"
    );
    assert!(
        (values["Main.Program.ModelTau"] - 5.0).abs() < 0.05,
        "T {values:?}"
    );
    assert!(
        (values["Main.Program.ModelDelay"] - 1.0).abs() < 0.02,
        "θ {values:?}"
    );
    assert!(
        candidate.gains.proportional_gain > 0.0 && candidate.gains.integral_gain > 0.0,
        "{:?}",
        candidate.gains
    );
    assert_eq!(candidate.gains.derivative_gain, 0.0);
    assert_eq!(values["Main.Program.Kp"], candidate.gains.proportional_gain);
    assert_eq!(outcome.section.method, None);
}

#[test]
fn a_settling_time_no_longer_than_the_models_delay_is_refused() {
    let loop_definition = heater_loop();
    let model = current();
    let request = request(None);
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
    };
    let refusal = tune_held(&inputs, Some(&task(0.8, 5.0)), None)
        .expect_err("no controller acts sooner than the delay");
    assert!(
        text_of(&refusal)
            .starts_with("The settling time must be longer than the model's delay of "),
        "{}",
        text_of(&refusal)
    );
}

#[test]
fn a_named_method_is_refused_because_only_automatic_designs_the_predictor() {
    let loop_definition = heater_loop();
    let model = current();
    let request = request(Some(TuneMethod::Simc));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
    };
    let refusal = tune_held(&inputs, Some(&task(10.0, 30.0)), None)
        .expect_err("a named method does not design the predictor");
    assert_eq!(refusal.next_action, NextAction::ChooseMethod);
    assert_eq!(text_of(&refusal), SMITH_AUTOMATIC);
}
