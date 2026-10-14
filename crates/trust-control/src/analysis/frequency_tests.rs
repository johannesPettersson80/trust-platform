//! The Bode frequency response (`frequency`, B9.9; N4.2 and N4.3). Ported assertions name the TypeScript test they come from.

use std::f64::consts::PI;
use std::time::Duration;

use serde_json::{json, Value};

use super::{checked_grid, frequency_answer, is_stable, model_poles_and_zeros, POLE_SCOPE};
use crate::analysis::budget::{Budget, ANALYSIS_BUDGET};
use crate::messages::{AnalysisInputs, FrequencyGrid};
use crate::model::{decode_general_model, GeneralModel};

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// The supplied model of `general-model.test.ts`: a continuous state-space heater with two inputs (`heater`, `fan`) and one output
/// (`temperature`), one state, `a = -2`, `b = [1, 3]`, `c = 2`, `d = [0, 1]`.
fn heater() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "engineer-model-1", "source": "provided",
        "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W"), channel("fan", "V")],
        "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "a": [[-2.0]], "b": [[1.0, 3.0]], "c": [[2.0]], "d": [[0.0, 1.0]],
    }))
    .expect("the heater model is valid")
}

/// The same heater with `b` and `d` set to zero, so that its response is exactly zero.
fn silent_heater() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "silent", "source": "provided",
        "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W")], "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "a": [[-2.0]], "b": [[0.0]], "c": [[2.0]], "d": [[0.0]],
    }))
    .expect("the silent model is valid")
}

fn grid(minimum_frequency: f64, maximum_frequency: f64, points: u32) -> FrequencyGrid {
    FrequencyGrid {
        minimum_frequency,
        maximum_frequency,
        points,
    }
}

fn inputs(grid: FrequencyGrid) -> AnalysisInputs {
    AnalysisInputs {
        grid: Some(grid),
        ..AnalysisInputs::default()
    }
}

/// Runs `run` with a budget that has spent no time and was not cancelled.
fn calm<T>(run: impl FnOnce(&Budget<'_>) -> T) -> T {
    let elapsed = || Duration::ZERO;
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    run(&budget)
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
}

/// The principal value of an angle, in `[−π, π)`.
fn wrapped(angle: f64) -> f64 {
    (angle + PI).rem_euclid(2.0 * PI) - PI
}

#[test]
fn an_unvalidated_rectangular_supplied_model_is_analysed_without_a_runtime_controller() {
    // Ported from `analyzes an unvalidated rectangular supplied model without a runtime controller` (control-studio-general-model.test.ts):
    // at 2 rad/s the heater channel is 0.5 − 0.5j and the fan channel 2.5 − 1.5j; the channels run output by output, inputs fastest.
    let model = heater();
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(2.0, 8.0, 2)), budget))
        .expect("the frequency response qualifies");
    let pairs: Vec<(&str, &str)> = answer
        .channels
        .iter()
        .map(|channel| (channel.output.id.as_str(), channel.input.id.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![("temperature", "heater"), ("temperature", "fan")]
    );
    let heater_point = answer.channels[0].points[0];
    assert!(close(heater_point.frequency_rad_per_second, 2.0));
    assert!(close(heater_point.real, 0.5) && close(heater_point.imaginary, -0.5));
    let fan_point = answer.channels[1].points[0];
    assert!(close(fan_point.real, 2.5) && close(fan_point.imaginary, -1.5));
    // Ported: `assert.deepStrictEqual(result.poles, [{ real: -2, imaginary: 0 }])`.
    assert_eq!(answer.poles.len(), 1);
    assert!(close(answer.poles[0].real, -2.0) && close(answer.poles[0].imaginary, 0.0));
    assert!(
        answer.zeros.is_empty(),
        "a state-space model has no zeros here"
    );
    assert!(answer.stable, "the pole at −2 is stable (N4.2)");
}

#[test]
fn a_transfer_function_is_analysed_with_its_exact_frequency_delay() {
    // Ported from `analyzes supplied transfer functions with exact frequency delay` (control-studio-general-model.test.ts): the
    // transfer function 2/(s + 2) with a delay of π/4 s, at 2 rad/s, is −0.5 − 0.5j, and its pole scope names the delay.
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "delayed", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": PI / 4.0,
        "inputs": [channel("u", "%")], "outputs": [channel("T", "degC")],
        "numerator": [2.0], "denominator": [1.0, 2.0],
    }))
    .expect("the transfer function is valid");
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(2.0, 8.0, 2)), budget))
        .expect("the response qualifies");
    let point = answer.channels[0].points[0];
    assert!(close(point.real, -0.5) && close(point.imaginary, -0.5));
    assert!(answer.pole_scope.contains("delay"));
    assert!(close(answer.poles[0].real, -2.0));
    assert!(answer.zeros.is_empty(), "a constant numerator has no zeros");
}

#[test]
fn a_discrete_model_is_analysed_on_the_unit_circle_and_an_alias_is_refused() {
    // Ported from `distinguishes discrete dynamics and rejects aliasing` (control-studio-general-model.test.ts): a = 0.5, b = [1, 0],
    // c = 1, Ts = 0.1 s; at π/(2·Ts) rad/s, z = j and G = 1/(j − 0.5) = −0.4 − 0.8j for the first input.
    let model = decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "discrete", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W"), channel("fan", "V")], "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "a": [[0.5]], "b": [[1.0, 0.0]], "c": [[1.0]], "d": [[0.0, 0.0]], "sampleTimeSeconds": 0.1,
    }))
    .expect("the discrete model is valid");
    let frequency = PI / 2.0 / 0.1;
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(frequency, 20.0, 2)), budget))
        .expect("the response qualifies below the Nyquist frequency");
    let point = answer.channels[0].points[0];
    assert!(close(point.real, -0.4) && close(point.imaginary, -0.8));
    assert!(
        answer.stable,
        "|0.5| < 1 is stable for a discrete pole (N4.2)"
    );
    // Ported: `alias.reason === "alias-risk"`: a grid at or above π/Ts is refused, and the refusal names the grid (A7, N4.3).
    let alias = checked_grid(&model, Some(grid(1.0, PI / 0.1, 2)))
        .expect_err("a grid at the Nyquist frequency is refused");
    assert!(alias.message.contains("Nyquist"), "{}", alias.message);
    assert_eq!(alias.next_action, crate::failure::NextAction::None);
}

#[test]
fn a_zero_response_is_finite_and_its_decibels_are_floored_at_the_smallest_positive_number() {
    // Ported from `keeps supplied evidence and channel units in a separate validated snapshot` (control-studio-general-model.test.ts): a
    // channel whose response is zero is the point (0, 0) with magnitude 0, which the page floors at Number.MIN_VALUE.
    let answer =
        calm(|budget| frequency_answer(&silent_heater(), &inputs(grid(2.0, 8.0, 2)), budget))
            .expect("a zero response qualifies");
    let point = answer.channels[0].points[0];
    assert!(point.real.abs() < 1e-12 && point.imaginary.abs() < 1e-12);
    assert!(point.magnitude.abs() < 1e-12);
    assert!(point.magnitude_db.is_finite());
    assert!(close(point.magnitude_db, 20.0 * f64::from_bits(1).log10()));
    assert!(close(point.phase_degrees, 0.0));
}

#[test]
fn the_phase_is_the_principal_value_as_n4_3_requires_for_a_plotted_phase() {
    // The TypeScript test `keeps the Bode phase continuous while a delay keeps lowering it` asserted the unwrapped phase. N4.3 says
    // "plotted phase is wrapped", so this assertion is the principal value of −ω − arctan(5ω) at each grid frequency; past −180°
    // at 3.2 rad/s it reads +90° (wrapped), as N4.3 requires.
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "heater", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 1.0,
        "inputs": [channel("u", "%")], "outputs": [channel("T", "degC")],
        "numerator": [0.2], "denominator": [5.0, 1.0],
    }))
    .expect("the heater transfer function is valid");
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(0.0125, 3.2, 17)), budget))
        .expect("the response qualifies");
    let points = &answer.channels[0].points;
    assert_eq!(points.len(), 17);
    for point in points {
        let omega = point.frequency_rad_per_second;
        let expected = wrapped(-omega - (5.0 * omega).atan());
        assert!(
            (point.phase_radians - expected).abs() < 1e-9,
            "φ at {omega}: {} against {expected}",
            point.phase_radians
        );
    }
    let last = points[16];
    assert!(close(last.frequency_rad_per_second, 3.2));
    assert!(
        last.phase_radians > 0.0 && last.phase_radians < PI,
        "wrapped past −180° at 3.2 rad/s: {}",
        last.phase_radians
    );
}

#[test]
fn a_frequency_at_which_the_response_is_singular_is_refused_as_a_failure_of_the_calculation() {
    // Ported from `qualifies square DC relative gain but rejects a singular frequency` (control-studio-general-model.test.ts): the
    // model with a = [[0, −1], [1, 0]] has the poles ±j, so its response is singular at 1 rad/s.
    let singular = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "singular", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W"), channel("fan", "V")], "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC"), channel("second", "degC")],
        "a": [[0.0, -1.0], [1.0, 0.0]], "b": [[1.0, 0.0], [0.0, 1.0]],
        "c": [[1.0, 0.0]], "d": [[0.0, 0.0]],
    }))
    .expect("the singular model is valid");
    let refused = calm(|budget| frequency_answer(&singular, &inputs(grid(1.0, 2.0, 2)), budget));
    assert!(
        refused.is_err(),
        "a singular frequency is refused, never answered with a nonfinite point"
    );
}

#[test]
fn a_marginal_pole_at_the_origin_qualifies_and_is_not_stable() {
    // Ported from `rejects malformed, unsupported and falsely validated model contracts` (control-studio-general-model.test.ts):
    // `a = 0` qualifies, and the pole on the boundary is marginal, so it is not stable (N4.2).
    let marginal = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "marginal", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("heater", "W")], "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "a": [[0.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
    }))
    .expect("the marginal model is valid");
    let answer = calm(|budget| frequency_answer(&marginal, &inputs(grid(1.0, 2.0, 2)), budget))
        .expect("a marginal model qualifies");
    assert!(!answer.stable);
    assert!(!is_stable(&marginal, &answer.poles));
}

#[test]
fn the_grid_rules_refuse_too_few_or_too_many_points_and_a_non_positive_frequency() {
    let model = heater();
    // Ported from `rejects oversized grids and never accepts another editor scope` (control-studio-general-model-session.test.ts).
    let oversized =
        checked_grid(&model, Some(grid(1.0, 10.0, 100_000))).expect_err("too many points");
    assert!(
        oversized.message.contains("points"),
        "{}",
        oversized.message
    );
    assert!(
        checked_grid(&model, Some(grid(1.0, 10.0, 1))).is_err(),
        "one point is refused"
    );
    assert!(
        checked_grid(&model, Some(grid(1.0, 10.0, 513))).is_err(),
        "513 points are refused"
    );
    // Ported from `rejects malformed ... model contracts`: the grid `[0]` is an invalid frequency grid.
    assert!(checked_grid(&model, Some(grid(0.0, 10.0, 2))).is_err());
    assert!(
        checked_grid(&model, Some(grid(10.0, 1.0, 2))).is_err(),
        "the maximum must be above the minimum"
    );
}

#[test]
fn without_a_grid_the_default_grid_is_used_and_the_answer_names_it() {
    // C23: 0.1 to 100 rad/s with 128 points for a one-channel continuous model.
    let grid = checked_grid(&heater(), None).expect("the default grid is valid");
    assert!(close(grid.minimum_frequency, 0.1));
    assert!(close(grid.maximum_frequency, 100.0));
    assert_eq!(grid.points, 128);
}

#[test]
fn the_summary_and_the_details_are_the_sentences_the_page_wrote() {
    let model = heater();
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(2.0, 8.0, 2)), budget))
        .expect("the response qualifies");
    assert_eq!(answer.summary[0], "Channel heater → temperature.");
    assert_eq!(answer.summary.len(), 2, "one sentence per channel");
    assert_eq!(answer.details, vec![format!("Pole scope: {POLE_SCOPE}")]);
}

#[test]
fn the_poles_and_zeros_of_a_transfer_function_are_its_polynomial_roots() {
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "zeros", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "%")], "outputs": [channel("T", "degC")],
        "numerator": [1.0, -1.0], "denominator": [1.0, 0.0, -1.0],
    }))
    .expect("the transfer function is valid");
    let (poles, zeros) = model_poles_and_zeros(&model).expect("the roots are found");
    let mut real_poles: Vec<f64> = poles.iter().map(|pole| pole.real).collect();
    real_poles.sort_by(f64::total_cmp);
    assert!(close(real_poles[0], -1.0) && close(real_poles[1], 1.0));
    assert_eq!(zeros.len(), 1);
    assert!(close(zeros[0].real, 1.0));
}
