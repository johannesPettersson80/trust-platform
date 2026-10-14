#![allow(clippy::float_cmp)] // The structural coefficients are exact products of the inputs, compared exactly on purpose.

use std::f64::consts::PI;

use trust_control_math::margins::analyze_loop;
use trust_control_math::model::StateSpace;

use super::*;
use crate::identification::plant::IdentifiedModel;
use crate::tuning::robustness::candidate_robustness;

fn model(
    kind: LowOrderModelKind,
    gain: f64,
    time: Option<f64>,
    dead_time: Option<f64>,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: time,
        second_time_constant_seconds: None,
        dead_time_seconds: dead_time,
        normalized_error: Some(0.0),
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

fn finite(value: MarginValue) -> f64 {
    match value {
        MarginValue::Finite(number) => number,
        other => panic!("the margin is finite, not {other:?}"),
    }
}

#[test]
fn an_integrator_under_proportional_control_has_the_closed_form_margins() {
    // L = 2·0.5/s = 1/s: the gain crossover is at 1 rad/s, the phase is −90° everywhere, so the phase margin is 90°,
    // the delay margin is (π/2)/1 s, and the phase never reaches −180°, so the gain margin is unbounded (N7.7).
    let integrator = model(LowOrderModelKind::Integrating, 2.0, None, None);
    let margins = pid_margins(&integrator, &PredictedGains::parallel(0.5, 0.0, 0.0))
        .expect("the integrator and the gains are valid");
    assert_eq!(margins.gain_margin_db, MarginValue::Unbounded);
    assert!(
        (finite(margins.phase_margin_degrees) - 90.0).abs() <= 1e-6,
        "{:?}",
        margins.phase_margin_degrees
    );
    let crossover = margins
        .gain_crossover_rad_per_second
        .expect("the loop crosses 0 dB");
    assert!((crossover - 1.0).abs() <= 1e-6, "{crossover}");
    assert!(margins.unavailable.is_none());
    let robustness = candidate_robustness(&margins);
    assert!((finite(robustness.delay_margin_seconds) - PI / 2.0).abs() <= 1e-6);
}

#[test]
fn a_first_order_loop_below_unit_gain_has_no_crossover_so_its_margins_are_unbounded() {
    // L = 1/(s + 1) never reaches unit magnitude above zero frequency, and its phase is above −90°.
    let first_order = model(LowOrderModelKind::FirstOrder, 1.0, Some(1.0), None);
    let margins = pid_margins(&first_order, &PredictedGains::parallel(1.0, 0.0, 0.0))
        .expect("the model and the gains are valid");
    assert_eq!(margins.phase_margin_degrees, MarginValue::Unbounded);
    assert_eq!(margins.gain_margin_db, MarginValue::Unbounded);
    assert!(margins.gain_crossover_rad_per_second.is_none());
    let robustness = candidate_robustness(&margins);
    assert_eq!(robustness.delay_margin_seconds, MarginValue::Unbounded);
}

#[test]
fn a_candidates_margins_are_the_libraries_margins_of_the_same_loop() {
    // identify-tune: a stable loop with no delay and a PI whose phase never reaches −180° has an unbounded gain margin.
    let plant = model(LowOrderModelKind::FirstOrder, 1.45928, Some(4.0), None);
    let gains = PredictedGains::parallel(0.57285, 0.38139, 0.0);
    let margins = pid_margins(&plant, &gains).expect("the model and the gains are valid");
    assert_eq!(margins.gain_margin_db, MarginValue::Unbounded);
    assert!(finite(margins.phase_margin_degrees).is_finite());
    let loop_ = pid_loop(&plant, &gains).expect("the loop is valid");
    let library = analyze_loop(&loop_.open_loop, &loop_.frequencies)
        .expect("the library reads the same loop");
    assert_eq!(library.gain_margin_db, margins.gain_margin_db);
    assert!(
        (finite(library.phase_margin_degrees) - finite(margins.phase_margin_degrees)).abs()
            <= 1e-12
    );
}

#[test]
fn a_delayed_plant_with_too_much_gain_has_a_negative_gain_margin_of_the_closed_form_size() {
    // identify-tune: L = 100·e^(−5s)/(s + 1) under proportional control with Kp = 1. The first phase crossover is
    // at atan(ω) + 5ω = π, ω ≈ 0.53 rad/s, where |L| ≈ 88.5, so the gain margin is −38.92 dB. The grid is refined for
    // the delay (N7.8); the margin stays visible and negative rather than being coerced positive.
    let delayed = model(
        LowOrderModelKind::FirstOrderDeadTime,
        100.0,
        Some(1.0),
        Some(5.0),
    );
    let margins = pid_margins(&delayed, &PredictedGains::parallel(1.0, 0.0, 0.0))
        .expect("the model and the gains are valid");
    let gain_margin = finite(margins.gain_margin_db);
    assert!(gain_margin < 0.0, "{gain_margin}");
    assert!((gain_margin + 38.922_214).abs() <= 0.01, "{gain_margin}");
}

#[test]
fn a_sampled_integrator_under_state_feedback_has_the_closed_form_phase_margin() {
    // The integrator x' = u sampled at T = 0.1 s with K = 1: L(z) = 0.1/(z − 1), so |L| = 1 at 2·asin(0.05)/T rad/s,
    // where the phase margin is 90° − asin(0.05) = 87.134016° (N7.9). Its only phase crossover is the Nyquist
    // frequency, which the grid does not reach, so the gain margin is not observed in range, never infinite (N7.8).
    let plant = StateSpace {
        a: vec![vec![0.0]],
        b: vec![vec![1.0]],
        c: vec![vec![1.0]],
        d: vec![vec![0.0]],
    };
    let margins =
        state_feedback_margins(&plant, 0.1, 0.0, &[1.0]).expect("the sampled loop is valid");
    assert!(
        (finite(margins.phase_margin_degrees) - 87.134_016).abs() <= 1e-4,
        "{:?}",
        margins.phase_margin_degrees
    );
    let crossover = margins
        .gain_crossover_rad_per_second
        .expect("the loop crosses 0 dB");
    assert!((crossover - 1.000_417_136).abs() <= 1e-6, "{crossover}");
    assert_eq!(margins.gain_margin_db, MarginValue::NotObservedInRange);
}

#[test]
fn a_unit_static_loop_has_no_defined_crossover_but_its_peak_sensitivity_is_one_half() {
    // identify-tune (the flat case): L ≡ K·Kp = 1, so |S| = 1/(1 + 1) = 1/2 at every frequency, and the crossovers are
    // undefined (magnitude one everywhere). The margins say so, and the peak sensitivity is still reported.
    let static_gain = model(LowOrderModelKind::StaticGain, 1.0, None, None);
    let margins = pid_margins(&static_gain, &PredictedGains::parallel(1.0, 0.0, 0.0))
        .expect("the model and the gains are valid");
    assert_eq!(
        margins.phase_margin_degrees,
        MarginValue::NotObservedInRange
    );
    assert!(margins.unavailable.is_some());
    assert!(
        (margins.peak_sensitivity - 0.5).abs() <= 1e-9,
        "{}",
        margins.peak_sensitivity
    );
}

#[test]
fn the_peak_sensitivity_of_a_loop_is_at_least_one_because_the_far_sensitivity_tends_to_one() {
    // identify-tune (Ms): a PI on 1/(8s + 1) with a 90° phase margin reaches |S| of one only above the plotted range,
    // and the peak includes that frequency, so Ms is at least one (N7.7).
    let lag = model(LowOrderModelKind::FirstOrder, 1.0, Some(8.0), None);
    let margins = pid_margins(&lag, &PredictedGains::parallel(3.75, 0.469, 0.0))
        .expect("the model and the gains are valid");
    assert!(
        margins.peak_sensitivity >= 1.0 - 1e-9,
        "{}",
        margins.peak_sensitivity
    );
    assert!(margins.peak_sensitivity.is_finite());
}

#[test]
fn the_delay_margin_is_the_phase_margin_over_the_gain_crossover_on_a_delayed_plant() {
    // identify-tune: a PI with a delay has a finite gain and phase margin, and its delay margin is the phase margin in
    // radians over the gain-crossover frequency (N7.7, the relation the candidate-robustness test checks).
    let delayed = model(
        LowOrderModelKind::FirstOrderDeadTime,
        1.0,
        Some(1.0),
        Some(0.2),
    );
    let margins = pid_margins(&delayed, &PredictedGains::parallel(1.0, 0.5, 0.0))
        .expect("the model and the gains are valid");
    let phase = finite(margins.phase_margin_degrees);
    let gain = finite(margins.gain_margin_db);
    assert!(gain.is_finite());
    let crossover = margins
        .gain_crossover_rad_per_second
        .expect("the loop crosses 0 dB");
    let delay = finite(candidate_robustness(&margins).delay_margin_seconds);
    assert!(
        (delay - phase.to_radians() / crossover).abs() <= 1e-9,
        "{delay}"
    );
}

#[test]
fn a_zero_plant_gain_is_refused_with_the_plot_sentence() {
    let zero = model(LowOrderModelKind::FirstOrder, 0.0, Some(1.0), None);
    let refusal = pid_margins(&zero, &PredictedGains::parallel(1.0, 0.0, 0.0))
        .expect_err("zero gain is refused");
    assert_eq!(
        refusal.0,
        "Engineering plots require a valid stable plant model."
    );
}

#[test]
fn the_measurement_filter_is_part_of_the_controller_and_the_positional_law_has_no_integrator() {
    // C0: the filter multiplies the controller by 1/(Tf·s + 1); a positional law has no integrator in its denominator.
    let filtered = PredictedGains {
        measurement_filter_seconds: 0.5,
        ..PredictedGains::parallel(2.0, 0.0, 0.0)
    };
    let controller = controller_transfer(&filtered);
    assert_eq!(controller.denominator, vec![0.5, 1.0]);
    assert_eq!(controller.numerator, vec![2.0]);
    let incremental = controller_transfer(&PredictedGains::parallel(2.0, 1.0, 0.0));
    assert_eq!(incremental.denominator, vec![1.0, 0.0]);
    assert_eq!(incremental.numerator, vec![2.0, 1.0]);
}

#[test]
fn an_integrating_plant_with_a_lag_has_the_integrator_and_the_lag_in_its_denominator() {
    // N0.4 and C0: K/(s(τs + 1)), so the denominator is τs² + s.
    let lagged = model(LowOrderModelKind::Integrating, 3.0, Some(2.0), None);
    let plant = plant_transfer(&lagged);
    assert_eq!(plant.numerator, vec![3.0]);
    assert_eq!(plant.denominator, vec![2.0, 1.0, 0.0]);
}

#[test]
fn a_nonlinear_law_has_no_margins_and_says_why() {
    // N7.7: a switching law is not a linear transfer function, so it shows no margins.
    assert_eq!(
        nonlinear_law_refusal().0,
        "A switching or other nonlinear law shows no margins (N7.7)."
    );
}
