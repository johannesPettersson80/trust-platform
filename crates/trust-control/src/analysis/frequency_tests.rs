//! The Bode frequency response (`frequency`, B9.9; N4.2 and N4.3). Ported assertions name the TypeScript test they come from.

use std::f64::consts::PI;
use std::time::Duration;

use serde_json::{json, Value};

use super::{checked_grid, frequency_answer, is_stable, model_poles_and_zeros, POLE_SCOPE};
use crate::analysis::budget::{Budget, ANALYSIS_BUDGET};
use crate::messages::{AnalysisInputs, FrequencyAnswer, FrequencyGrid};
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

/// A transfer function from `u` in % to `T` in °C with the given polynomials and delay.
fn transfer_model(numerator: &[f64], denominator: &[f64], delay_seconds: f64) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "transfer", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": delay_seconds,
        "inputs": [channel("u", "%")], "outputs": [channel("T", "degC")],
        "numerator": numerator, "denominator": denominator,
    }))
    .expect("the transfer function is valid")
}

/// `G(s) = K·e^(−Ls)/(Ts + 1)` with `K = 1`, `T = 1 s` and `L = 1 s`.
fn delayed_first_order() -> GeneralModel {
    transfer_model(&[1.0], &[1.0, 1.0], 1.0)
}

/// The plotted phases in degrees of the first channel of an answer.
fn phases_of(answer: &FrequencyAnswer) -> Vec<f64> {
    answer.channels[0]
        .points
        .iter()
        .map(|point| point.phase_degrees)
        .collect()
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
fn the_plotted_phase_keeps_falling_with_the_delay_past_minus_180_degrees() {
    // Ported from `keeps the Bode phase continuous while a delay keeps lowering it` (TypeScript), which asserted the unwrapped
    // phase that N4.3 requires: the heater 0.2·e^(−s)/(5s + 1) has the plotted phase −ω − atan(5ω) rad at each grid frequency.
    // At 3.2 rad/s that is −3.2 − atan(16) = −4.708378 rad (−269.77°, the −269.8° of Lesson 54); its principal value would be
    // +90.23°. Catches a wrapped plotted phase.
    let model = transfer_model(&[0.2], &[5.0, 1.0], 1.0);
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(0.0125, 3.2, 17)), budget))
        .expect("the response qualifies");
    let points = &answer.channels[0].points;
    assert_eq!(points.len(), 17);
    for point in points {
        let omega = point.frequency_rad_per_second;
        let expected = -omega - (5.0 * omega).atan();
        assert!(
            (point.phase_radians - expected).abs() < 1e-9,
            "φ at {omega}: {} against {expected}",
            point.phase_radians
        );
        assert!(
            (point.phase_degrees - expected.to_degrees()).abs() < 1e-7,
            "the degrees are the same angle as the radians at {omega}"
        );
    }
    let last = points[16];
    assert!(close(last.frequency_rad_per_second, 3.2));
    assert!(
        (last.phase_radians + 4.708_377_516_798_94).abs() < 1e-9,
        "past −180° at 3.2 rad/s: {}",
        last.phase_radians
    );
}

#[test]
fn a_delayed_first_order_phase_is_continuous_far_past_minus_180_degrees() {
    // N4.3: e^(−s)/(s + 1) has the plotted phase −atan(ω) − ω rad. At 10 rad/s, the last of 64 grid points from 0.01 rad/s,
    // that is −atan(10) − 10 rad = −84.289407° − 572.957795° = −657.247202°; the principal phase there is 62.752798°. The
    // widest grid step, 10·(1 − 1000^(−1/63)) = 1.04 rad/s, turns the phase by about 60°, so no step reaches 180°. Catches a
    // wrapped plotted phase, which jumps by about 360° each time it passes −180°.
    let answer = calm(|budget| {
        frequency_answer(
            &delayed_first_order(),
            &inputs(grid(0.01, 10.0, 64)),
            budget,
        )
    })
    .expect("the response qualifies");
    let points = &answer.channels[0].points;
    assert_eq!(points.len(), 64);
    let last = points[63];
    assert!(close(last.frequency_rad_per_second, 10.0));
    assert!(
        (last.phase_degrees + 657.247_201_993_3).abs() < 1e-9,
        "{}",
        last.phase_degrees
    );
    for pair in points.windows(2) {
        let step = pair[1].phase_degrees - pair[0].phase_degrees;
        assert!(
            step.abs() < 180.0,
            "a step of {step}° at {} rad/s",
            pair[1].frequency_rad_per_second
        );
    }
    for point in points {
        let omega = point.frequency_rad_per_second;
        let expected = (-omega.atan() - omega).to_degrees();
        assert!(
            (point.phase_degrees - expected).abs() < 1e-9,
            "{omega}: {}",
            point.phase_degrees
        );
        assert!((point.phase_radians - point.phase_degrees.to_radians()).abs() < 1e-12);
    }
}

#[test]
fn on_a_coarse_grid_the_delay_phase_is_added_in_closed_form() {
    // N4.3: the same model on three points, 0.1, √(0.1·100) = 3.162278 and 100 rad/s. Each point is exactly −atan(ω) − ω rad:
    // −11.440171°, −72.451599° − 181.185164° = −253.636763°, and at 100 rad/s −atan(100) − 100 rad = −89.427061° − 5729.577951°
    // = −5819.005013°. Catches a plain unwrap of the full response: its principal phases are −11.44°, 106.36° and −59.01°, which
    // that unwrap keeps as they are, 16 turns above the plotted phase at 100 rad/s.
    let answer = calm(|budget| {
        frequency_answer(&delayed_first_order(), &inputs(grid(0.1, 100.0, 3)), budget)
    })
    .expect("the response qualifies");
    let phases = phases_of(&answer);
    let expected = [
        -11.440_171_088_808,
        -253.636_762_962_36,
        -5_819.005_012_610_5,
    ];
    assert_eq!(phases.len(), expected.len());
    for (phase, expected) in phases.iter().zip(expected) {
        assert!((phase - expected).abs() < 1e-8, "{phases:?}");
    }
}

#[test]
fn a_negative_static_gain_plots_180_degrees_at_every_frequency() {
    // N4.3: G(s) = −2 is −2 + 0j at every frequency: magnitude 2, and the first point takes the principal phase 180°. The phase
    // never changes, so every point stays at 180°. Catches a first point read as −180° and a series that jumps between ±180°.
    let model = transfer_model(&[-2.0], &[1.0], 0.0);
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(0.1, 100.0, 16)), budget))
        .expect("the response qualifies");
    for point in &answer.channels[0].points {
        assert!(
            (point.phase_degrees - 180.0).abs() < 1e-9,
            "{}",
            point.phase_degrees
        );
        assert!(close(point.magnitude, 2.0));
    }
}

#[test]
fn a_negative_rate_gain_integrator_plots_plus_90_degrees_throughout() {
    // N4.3: Kprime/(jω) with Kprime = −0.5 is 0.5j/ω: magnitude 0.5/ω and phase +90° at every frequency, from the principal
    // phase of the first point. Catches the phase of a positive rate gain (−90°) and a first point on another branch (−270°).
    let model = transfer_model(&[-0.5], &[1.0, 0.0], 0.0);
    let answer = calm(|budget| frequency_answer(&model, &inputs(grid(0.1, 10.0, 9)), budget))
        .expect("the response qualifies");
    for point in &answer.channels[0].points {
        let omega = point.frequency_rad_per_second;
        assert!(
            (point.phase_degrees - 90.0).abs() < 1e-9,
            "{omega}: {}",
            point.phase_degrees
        );
        assert!(close(point.magnitude, 0.5 / omega));
    }
}

#[test]
fn a_discrete_delay_of_whole_samples_is_added_in_closed_form() {
    // N4.3: x⁺ = 0.5x + u, y = x with Ts = 0.1 s and a delay of three samples, 0.3 s, on the grid 1 rad/s and π/(2·Ts). Without
    // the delay the response is 1/(z − 0.5): at 1 rad/s, z = e^(0.1j), its phase is −11.402566°; at π/(2·Ts), z = j, it is
    // 1/(j − 0.5) = −0.4 − 0.8j with the phase −116.565051°, a step of −105.16° and no turn. The delay adds −ω·0.3 s: −17.188734°
    // and −(π/2)·3 rad = −270°, so the points are −28.591300° and −386.565051°. The response itself there, 0.8 − 0.4j, has the
    // principal phase −26.565051°, which a plain unwrap from −28.59° keeps. Catches a delay of whole samples unwrapped with the
    // response instead of added in closed form.
    let model = decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "discrete-delay", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.3,
        "inputs": [channel("heater", "W")], "outputs": [channel("temperature", "degC")],
        "states": [channel("thermal", "degC")],
        "a": [[0.5]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]], "sampleTimeSeconds": 0.1,
    }))
    .expect("the discrete model is valid");
    let quarter_sampling = PI / 2.0 / 0.1;
    let answer =
        calm(|budget| frequency_answer(&model, &inputs(grid(1.0, quarter_sampling, 2)), budget))
            .expect("the response qualifies below the Nyquist frequency");
    let points = &answer.channels[0].points;
    assert!(
        close(points[1].real, 0.8) && close(points[1].imaginary, -0.4),
        "the response itself keeps its delay"
    );
    assert!(
        (points[0].phase_degrees + 28.591_299_696_288).abs() < 1e-9,
        "{}",
        points[0].phase_degrees
    );
    assert!(
        (points[1].phase_degrees + 386.565_051_177_078).abs() < 1e-9,
        "{}",
        points[1].phase_degrees
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
