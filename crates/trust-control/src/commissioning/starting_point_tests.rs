// The bounds and operating values compared here are the exact numbers the fixtures set.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::entry_allowance::auto_entry_tolerance;
use crate::commissioning::fixture_loops::{
    first_order_model, integrating_model, parallel_pid_loop, requirements,
};
use crate::commissioning::transfer::pid_transfer_binding;

fn symbols(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn reading(pairs: &[(&str, f64)]) -> BTreeMap<String, TransferValue> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), TransferValue::Number(*value)))
        .collect()
}

#[test]
fn a_manual_apply_accepts_a_plant_input_within_the_auto_entry_allowance_and_refuses_one_beyond_it()
{
    // V4.6: the allowance of the fixture PID with Kp 1 at a noise of 0.05 is 2·Kp·n = 0.1 (A6.4).
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let inputs = symbols(&["ValveMV"]);
    let outputs = symbols(&["LevelPV"]);
    let gains = BTreeMap::from([
        ("Main.Parameters.Kp".to_owned(), 1.0),
        ("Main.Parameters.Ki".to_owned(), 0.0),
        ("Main.Parameters.Kd".to_owned(), 0.0),
    ]);
    let allowance = auto_entry_tolerance(&loop_definition, &task, &gains, Some(0.05), Some(0.0))
        .expect("the allowance");
    assert!((allowance - 0.1).abs() < 1e-12);

    let compared = compared_plant_inputs(&inputs, Some(FoundMode::Manual));
    let manual = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: compared,
        output_symbols: &outputs,
        input_tolerance: allowance,
    };
    assert_eq!(
        manual.assert_settled(&reading(&[("ValveMV", 0.05), ("LevelPV", 2.0)])),
        Ok(()),
        "0.05 lies within ±0.1 of the validated input 0"
    );
    let failure = manual
        .assert_settled(&reading(&[("ValveMV", 0.2), ("LevelPV", 2.0)]))
        .expect_err("0.2 lies beyond ±0.1");
    assert_eq!(
        failure.message,
        "ValveMV is 0.2 %, but the settings were validated from 0 % ± 0.1 %."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::None),
        "a plant input away from its point cannot settle by itself: no next action"
    );
}

#[test]
fn an_auto_apply_does_not_compare_the_plant_input_because_the_running_controller_moves_it() {
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let inputs = symbols(&["ValveMV"]);
    let outputs = symbols(&["LevelPV"]);
    let compared = compared_plant_inputs(&inputs, Some(FoundMode::Auto));
    assert!(
        compared.is_empty(),
        "a loop found in Auto compares no plant input"
    );
    let auto = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: compared,
        output_symbols: &outputs,
        input_tolerance: 0.1,
    };
    assert_eq!(
        auto.assert_settled(&reading(&[("ValveMV", 40.0), ("LevelPV", 2.0)])),
        Ok(()),
        "the transfer starts from wherever the plant input is"
    );
}

#[test]
fn a_settled_process_within_the_settling_band_reaches_the_apply() {
    // The `TypeScript` test: a settled 2.004 m against the validated 2 m within ±0.02 m reaches the review.
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let outputs = symbols(&["LevelPV"]);
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &[],
        output_symbols: &outputs,
        input_tolerance: 0.0,
    };
    assert_eq!(
        start.assert_settled(&reading(&[("LevelPV", 2.004)])),
        Ok(())
    );
    // The dwell is 0.2 s: a tenth of the 2 s settling time, and twenty 10 ms executions (B6.2). The first reading inside the band
    // starts it, and the wait ends once the output has stayed inside for that dwell.
    let entered = start
        .wait_step(&reading(&[("LevelPV", 2.004)]), 100.0, 60_000.0, None)
        .expect("the reading waits for the dwell");
    assert_eq!(inside_since(&entered), Some(100.0));
    assert_eq!(
        start.wait_step(
            &reading(&[("LevelPV", 2.004)]),
            400.0,
            60_000.0,
            Some(100.0)
        ),
        Ok(WaitStep::Settled)
    );
}

/// The start of the settle dwell that a waiting step hands on. A settled step has no start to hand on.
fn inside_since(step: &WaitStep) -> Option<f64> {
    match step {
        WaitStep::Waiting {
            inside_since_ms, ..
        } => *inside_since_ms,
        WaitStep::Settled => panic!("the wait has settled: {step:?}"),
    }
}

#[test]
fn an_output_outside_its_band_is_waited_for_with_its_validated_and_current_value() {
    // The `TypeScript` progress text: "Waiting for … to settle at 2 … ± 0.02 … (now 2.5 …)…".
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let outputs = symbols(&["LevelPV"]);
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &[],
        output_symbols: &outputs,
        input_tolerance: 0.0,
    };
    let failure = start
        .assert_settled(&reading(&[("LevelPV", 2.5)]))
        .expect_err("2.5 lies outside the band of 2 ± 0.02");
    assert_eq!(
        failure.message,
        "LevelPV is 2.5 %, but the settings were validated from 2 % ± 0.02 %."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Wait)
    );

    let step = start
        .wait_step(&reading(&[("LevelPV", 2.5)]), 100.0, 60_000.0, None)
        .expect("the wait continues");
    assert_eq!(
        step,
        WaitStep::Waiting {
            progress: "Waiting for LevelPV to settle at 2 % ± 0.02 % (now 2.5 %)…".into(),
            inside_since_ms: None,
        }
    );
}

#[test]
fn a_wait_ends_with_a_validation_once_the_process_time_bound_has_passed() {
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let outputs = symbols(&["LevelPV"]);
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &[],
        output_symbols: &outputs,
        input_tolerance: 0.0,
    };
    let bound = wait_bound_ms(&task);
    assert_eq!(
        bound, 60_000.0,
        "the longer of 60 s and three times the 4 s duration"
    );
    let failure = start
        .wait_step(&reading(&[("LevelPV", 2.5)]), bound + 1.0, bound, None)
        .expect_err("the bound has passed");
    assert_eq!(
        failure.message,
        "LevelPV did not settle at 2 % ± 0.02 % within 60 s of process time; it is 2.5 %."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );

    let long_task = crate::validation::engineer_verification_plan::engineer_requirements(
        &serde_json::json!({ "referenceChange": 1, "maximumError": 0.1, "settlingBand": 0.02,
            "settlingTimeSeconds": 2, "durationSeconds": 30 }),
    )
    .expect("the 30 s requirements are valid");
    assert_eq!(
        wait_bound_ms(&long_task),
        90_000.0,
        "three times a 30 s duration"
    );
}

#[test]
fn a_plant_input_away_from_its_point_is_refused_at_once_while_outputs_are_waited_for() {
    let loop_definition = parallel_pid_loop();
    let model = first_order_model();
    let task = requirements();
    let inputs = symbols(&["ValveMV"]);
    let outputs = symbols(&["LevelPV"]);
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 0.1,
    };
    let failure = start
        .wait_step(
            &reading(&[("ValveMV", 0.5), ("LevelPV", 2.5)]),
            100.0,
            60_000.0,
            None,
        )
        .expect_err("the input is away");
    assert!(
        failure.message.starts_with("ValveMV is 0.5 %"),
        "{}",
        failure.message
    );
    assert_eq!(
        failure.next_action,
        NextAction::None,
        "the wait does not help a plant input"
    );
}

#[test]
fn an_integrating_output_rests_wherever_it_is_and_takes_its_present_value_as_its_point() {
    let loop_definition = parallel_pid_loop();
    let model = integrating_model();
    let task = requirements();
    let outputs = symbols(&["LevelPV"]);
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &[],
        output_symbols: &outputs,
        input_tolerance: 0.0,
    };
    assert_eq!(
        start.assert_settled(&reading(&[("LevelPV", 2.5)])),
        Ok(()),
        "the level does not matter"
    );

    let anchored = anchor_integrating_outputs(&model, &reading(&[("LevelPV", 2.5)]), &outputs);
    assert_eq!(
        anchored
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![2.5])
    );
    let stable = first_order_model();
    assert_eq!(
        anchor_integrating_outputs(&stable, &reading(&[("LevelPV", 2.5)]), &outputs),
        stable,
        "a stable output keeps its operating point"
    );
}

#[test]
fn the_compared_plant_inputs_follow_the_found_mode() {
    let inputs = symbols(&["ValveMV"]);
    assert_eq!(
        compared_plant_inputs(&inputs, Some(FoundMode::Manual)),
        inputs.as_slice()
    );
    assert_eq!(
        compared_plant_inputs(&inputs, None),
        inputs.as_slice(),
        "an unknown mode compares"
    );
    assert_eq!(
        compared_plant_inputs(&inputs, Some(FoundMode::Auto)),
        [] as [String; 0]
    );
}

#[test]
fn a_cancelled_wait_writes_nothing_and_offers_the_validation() {
    let failure = wait_cancelled();
    assert_eq!(
        failure.message,
        "Apply cancelled while waiting for the process to settle; nothing was written."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );
    assert_eq!(failure.next_action.label(), "Validate controller");
}

#[test]
fn an_inner_cascade_loop_refuses_its_apply_while_the_outer_controller_is_in_auto() {
    let hold = ReferenceHold {
        mode_symbol: "Main.OuterAuto".into(),
        manual_value: false,
    };
    let failure = assert_reference_held(
        Some(&hold),
        &BTreeMap::from([("Main.OuterAuto".to_owned(), TransferValue::Boolean(true))]),
    )
    .expect_err("the outer controller is in Auto");
    assert_eq!(
        failure.message,
        "Put the outer controller in Manual before applying (Main.OuterAuto): its output is this loop's setpoint."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Validate)
    );

    let outer_in_manual =
        BTreeMap::from([("Main.OuterAuto".to_owned(), TransferValue::Number(0.0))]);
    assert_eq!(
        assert_reference_held(Some(&hold), &outer_in_manual),
        Ok(()),
        "in Manual the reference is held"
    );
    assert!(
        assert_reference_held(Some(&hold), &BTreeMap::new()).is_err(),
        "no reading holds nothing"
    );
    assert_eq!(
        assert_reference_held(None, &BTreeMap::new()),
        Ok(()),
        "a loop without a cascade is not held"
    );
}

#[test]
fn the_reference_hold_guards_the_outer_mode_once_and_without_regard_to_case() {
    let loop_definition = parallel_pid_loop();
    let binding = pid_transfer_binding(&loop_definition).expect("the fixture binds");
    let hold = ReferenceHold {
        mode_symbol: "Main.OuterAuto".into(),
        manual_value: false,
    };
    let guarded = with_reference_hold_guard(binding.clone(), Some(&hold));
    let outer = guarded
        .guards
        .iter()
        .find(|guard| guard.symbol == "Main.OuterAuto")
        .expect("the outer mode is a guard");
    assert_eq!(outer.iec_type, TransferValueType::Bool);
    assert_eq!((outer.minimum, outer.maximum), (0.0, 1.0));
    assert_eq!(
        with_reference_hold_guard(guarded.clone(), Some(&hold)),
        guarded,
        "not added twice"
    );

    let lower = ReferenceHold {
        mode_symbol: "main.outerauto".into(),
        manual_value: false,
    };
    assert_eq!(
        with_reference_hold_guard(guarded.clone(), Some(&lower)),
        guarded,
        "the same mode in another case"
    );
    assert_eq!(
        with_reference_hold_guard(binding.clone(), None),
        binding,
        "no hold, no guard"
    );
}
