//! Tests of the scenario effect tables. The `TypeScript` suite asserts none of these rules (its editor test only keeps the
//! effect fields through edits), so the expectations are those of `engineerScenarioEffects.ts`: the field order of a
//! table, the numbers as `JSON.stringify` writes them, and each refusal with its own sentence.

use serde_json::json;

use super::*;
use crate::failure::{Cause, Failure, NextAction};
use crate::model::{decode_general_model, GeneralModel};
use crate::validation::texts;

const DURATION_MS: u64 = 30_000;

/// A single-input, single-output transfer function with a first-order denominator: one state, and channel 0 is the only
/// channel of each kind.
fn model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [1.5],
        "denominator": [4.0, 1.0],
        "delaySeconds": 0.5,
        "revision": "model-4",
        "source": "provided",
        "validation": "validated",
        "evidence": ["declared-fixture"],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }]
    }))
    .expect("the fixture model decodes")
}

/// A constant effect of value 1 that starts with the run, on one channel of one target.
fn constant(id: &str, target: EffectTarget, channel: usize) -> EngineerScenarioEffect {
    EngineerScenarioEffect {
        id: id.to_owned(),
        target,
        channel,
        mode: EffectMode::Add,
        start_seconds: 0.0,
        end_seconds: None,
        generator: EffectGenerator::Constant,
        value: Some(1.0),
        minimum: None,
        maximum: None,
        duration_seconds: None,
    }
}

/// The effects are refused with `text`: a Settings failure whose next action opens the requirements.
fn assert_refused(result: Result<String, Failure>, text: &str) {
    let failure = result.expect_err("the scenario effect is refused");
    assert_eq!(failure.message, text);
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn a_constant_effect_writes_one_table_in_the_native_field_order() {
    let effect = EngineerScenarioEffect {
        value: Some(1.962),
        ..constant("reviewed-load", EffectTarget::ProcessInput, 0)
    };
    let text =
        scenario_effect_tables(&[effect], &model(), DURATION_MS).expect("the effect is bounded");
    assert_eq!(
        text,
        "[[plants.uncertainties]]\nid = \"reviewed-load\"\ntarget = \"process_input\"\nchannel = 0\n\
         mode = \"add\"\ngenerator = \"constant\"\nstart_s = 0\nvalue = 1.962\n"
    );
}

#[test]
fn a_pulse_writes_its_end_then_its_value_then_its_duration() {
    let pulse = EngineerScenarioEffect {
        id: "pulse-1".to_owned(),
        target: EffectTarget::Measurement,
        channel: 0,
        mode: EffectMode::Replace,
        start_seconds: 1.5,
        end_seconds: Some(2.5),
        generator: EffectGenerator::Pulse,
        value: Some(3.0),
        minimum: None,
        maximum: None,
        duration_seconds: Some(0.5),
    };
    let text =
        scenario_effect_tables(&[pulse], &model(), DURATION_MS).expect("the pulse is bounded");
    assert_eq!(
        text,
        "[[plants.uncertainties]]\nid = \"pulse-1\"\ntarget = \"measurement\"\nchannel = 0\n\
         mode = \"replace\"\ngenerator = \"pulse\"\nstart_s = 1.5\nend_s = 2.5\nvalue = 3\nduration_s = 0.5\n"
    );
}

#[test]
fn uniform_noise_writes_its_bounds_and_tables_are_separated_by_a_blank_line() {
    let noise = EngineerScenarioEffect {
        id: "noise-1".to_owned(),
        target: EffectTarget::Measurement,
        channel: 0,
        mode: EffectMode::Add,
        start_seconds: 0.0,
        end_seconds: None,
        generator: EffectGenerator::UniformWhite,
        value: None,
        minimum: Some(-0.5),
        maximum: Some(0.5),
        duration_seconds: None,
    };
    let load = constant("load", EffectTarget::ProcessInput, 0);
    let text = scenario_effect_tables(&[noise, load], &model(), DURATION_MS)
        .expect("both effects are bounded");
    let noise_table = "[[plants.uncertainties]]\nid = \"noise-1\"\ntarget = \"measurement\"\nchannel = 0\n\
         mode = \"add\"\ngenerator = \"uniform_white\"\nstart_s = 0\nminimum = -0.5\nmaximum = 0.5\n";
    let load_table =
        "[[plants.uncertainties]]\nid = \"load\"\ntarget = \"process_input\"\nchannel = 0\n\
         mode = \"add\"\ngenerator = \"constant\"\nstart_s = 0\nvalue = 1\n";
    assert_eq!(text, format!("{noise_table}\n{load_table}"));
}

#[test]
fn numbers_print_as_ecmascript_prints_them() {
    // JSON.stringify(1e-7) is "1e-7": below 1e-6 the number takes exponential notation.
    let tiny = EngineerScenarioEffect {
        value: Some(1e-7),
        ..constant("tiny", EffectTarget::ProcessInput, 0)
    };
    let text =
        scenario_effect_tables(&[tiny], &model(), DURATION_MS).expect("the effect is bounded");
    assert!(text.ends_with("start_s = 0\nvalue = 1e-7\n"), "{text}");
}

#[test]
fn more_than_thirty_two_effects_or_a_repeated_identity_need_a_plant_and_distinct_identities() {
    let many: Vec<EngineerScenarioEffect> = (0..=MAXIMUM_EFFECTS)
        .map(|index| constant(&format!("effect-{index}"), EffectTarget::ProcessInput, 0))
        .collect();
    scenario_effect_tables(&many[..MAXIMUM_EFFECTS], &model(), DURATION_MS)
        .expect("32 effects are the most one scenario applies");
    assert_refused(
        scenario_effect_tables(&many, &model(), DURATION_MS),
        &texts::scenario_effects_need_plant(),
    );
    let repeated = [
        constant("same", EffectTarget::ProcessInput, 0),
        constant("same", EffectTarget::ProcessInput, 0),
    ];
    assert_refused(
        scenario_effect_tables(&repeated, &model(), DURATION_MS),
        &texts::scenario_effects_need_plant(),
    );
}

#[test]
fn a_channel_must_exist_for_its_target_and_a_state_counts_the_denominator_less_one() {
    // One input, one output, and one state: the denominator has two coefficients.
    let valid = [
        constant("command", EffectTarget::Command, 0),
        constant("output", EffectTarget::ProcessOutput, 0),
        constant("measured", EffectTarget::Measurement, 0),
        constant("state", EffectTarget::State, 0),
    ];
    scenario_effect_tables(&valid, &model(), DURATION_MS)
        .expect("channel 0 exists for every target");
    for (target, channel) in [
        (EffectTarget::ProcessInput, 1),
        (EffectTarget::State, 1),
        (EffectTarget::Measurement, 1),
    ] {
        assert_refused(
            scenario_effect_tables(&[constant("bad", target, channel)], &model(), DURATION_MS),
            &texts::scenario_effect_coordinates_invalid(),
        );
    }
}

#[test]
fn hold_is_refused_on_a_state_and_on_a_command_but_allowed_on_an_output() {
    let hold = |target| EngineerScenarioEffect {
        mode: EffectMode::Hold,
        ..constant("hold", target, 0)
    };
    assert_refused(
        scenario_effect_tables(&[hold(EffectTarget::State)], &model(), DURATION_MS),
        &texts::scenario_effect_coordinates_invalid(),
    );
    assert_refused(
        scenario_effect_tables(&[hold(EffectTarget::Command)], &model(), DURATION_MS),
        &texts::scenario_effect_coordinates_invalid(),
    );
    scenario_effect_tables(&[hold(EffectTarget::ProcessOutput)], &model(), DURATION_MS)
        .expect("a hold on an output is bounded");
}

#[test]
fn a_window_starts_inside_the_run_and_ends_after_its_start_inside_the_run() {
    let window = |start: f64, end: Option<f64>| EngineerScenarioEffect {
        start_seconds: start,
        end_seconds: end,
        ..constant("window", EffectTarget::ProcessInput, 0)
    };
    for effect in [
        window(30.0, None),
        window(-0.5, None),
        window(1.0, Some(1.0)),
        window(1.0, Some(31.0)),
    ] {
        assert_refused(
            scenario_effect_tables(&[effect], &model(), DURATION_MS),
            &texts::scenario_effect_coordinates_invalid(),
        );
    }
    scenario_effect_tables(&[window(1.0, Some(30.0))], &model(), DURATION_MS)
        .expect("a window ending at the run's end is bounded");
}

#[test]
fn an_identity_is_a_letter_then_letters_digits_underscores_or_dashes() {
    for id in ["ok_id-2", "Gain", "a-"] {
        scenario_effect_tables(
            &[constant(id, EffectTarget::ProcessInput, 0)],
            &model(),
            DURATION_MS,
        )
        .expect("the identity follows the native rule");
    }
    for id in ["1bad", "bad id", "", "bad.id"] {
        assert_refused(
            scenario_effect_tables(
                &[constant(id, EffectTarget::ProcessInput, 0)],
                &model(),
                DURATION_MS,
            ),
            &texts::scenario_effect_coordinates_invalid(),
        );
    }
}

#[test]
fn a_constant_needs_a_finite_value_a_pulse_a_positive_duration_and_noise_ordered_bounds() {
    let without_value = EngineerScenarioEffect {
        value: None,
        ..constant("a", EffectTarget::ProcessInput, 0)
    };
    assert_refused(
        scenario_effect_tables(&[without_value], &model(), DURATION_MS),
        &texts::constant_effect_needs_value(),
    );
    let infinite = EngineerScenarioEffect {
        value: Some(f64::INFINITY),
        ..constant("b", EffectTarget::ProcessInput, 0)
    };
    assert_refused(
        scenario_effect_tables(&[infinite], &model(), DURATION_MS),
        &texts::constant_effect_needs_value(),
    );
    let pulse_without_duration = EngineerScenarioEffect {
        generator: EffectGenerator::Pulse,
        duration_seconds: Some(0.0),
        ..constant("c", EffectTarget::ProcessInput, 0)
    };
    assert_refused(
        scenario_effect_tables(&[pulse_without_duration], &model(), DURATION_MS),
        &texts::pulse_needs_duration(),
    );
    let reversed = EngineerScenarioEffect {
        generator: EffectGenerator::UniformWhite,
        value: None,
        minimum: Some(1.0),
        maximum: Some(-1.0),
        ..constant("d", EffectTarget::Measurement, 0)
    };
    assert_refused(
        scenario_effect_tables(&[reversed], &model(), DURATION_MS),
        &texts::noise_bounds_invalid(),
    );
    let empty_band = EngineerScenarioEffect {
        generator: EffectGenerator::UniformWhite,
        value: None,
        minimum: Some(0.5),
        maximum: Some(0.5),
        ..constant("e", EffectTarget::Measurement, 0)
    };
    assert_refused(
        scenario_effect_tables(&[empty_band], &model(), DURATION_MS),
        &texts::noise_bounds_invalid(),
    );
}

#[test]
fn the_checks_alone_accept_what_the_tables_accept() {
    check_scenario_effects(
        &[constant("ok", EffectTarget::ProcessInput, 0)],
        &model(),
        DURATION_MS,
    )
    .expect("the effect is bounded");
    assert!(check_scenario_effects(
        &[constant("ok", EffectTarget::ProcessInput, 1)],
        &model(),
        DURATION_MS
    )
    .is_err());
}
