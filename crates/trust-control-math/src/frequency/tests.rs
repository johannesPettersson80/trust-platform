use std::f64::consts::PI;

use super::*;

fn transfer(numerator: &[f64], denominator: &[f64]) -> TransferFunction {
    TransferFunction::new(numerator.to_vec(), denominator.to_vec()).expect("valid test polynomial")
}

fn continuous(numerator: &[f64], denominator: &[f64], delay_seconds: f64) -> FrequencyModel {
    FrequencyModel::Transfer {
        transfer: transfer(numerator, denominator),
        timing: Timing::Continuous,
        delay_seconds,
    }
}

fn sampled(numerator: &[f64], denominator: &[f64], sample_time_seconds: f64) -> FrequencyModel {
    FrequencyModel::Transfer {
        transfer: transfer(numerator, denominator),
        timing: Timing::Discrete {
            sample_time_seconds,
        },
        delay_seconds: 0.0,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn modulus(value: ComplexValue) -> f64 {
    value.real.hypot(value.imaginary)
}

fn first_value(model: &FrequencyModel, omega: f64) -> ComplexValue {
    response_at(model, omega).expect("response")[0][0]
}

#[test]
fn logarithmic_grid_has_exact_end_points_and_equal_decade_steps() {
    // Three points from 1 to 100 rad/s: 1, 10, 100 (the middle point is the geometric mean).
    let grid = logarithmic_grid(1.0, 100.0, 3).expect("grid");
    assert_eq!(grid.len(), 3);
    assert_close(grid[0], 1.0, 0.0);
    assert_close(grid[1], 10.0, 1e-12);
    assert_close(grid[2], 100.0, 0.0);

    let dense = logarithmic_grid(1e-3, 1e3, 2001).expect("grid");
    assert_eq!(dense.len(), 2001);
    assert!(dense.windows(2).all(|pair| pair[1] > pair[0]));
    // The ratio between neighbours is constant: (1e6)^(1/2000).
    assert_close(dense[1] / dense[0], 1e6_f64.powf(1.0 / 2000.0), 1e-12);
}

#[test]
fn logarithmic_grid_refuses_short_counts_and_bad_bounds() {
    assert_eq!(
        logarithmic_grid(1.0, 10.0, 1),
        Err(FrequencyError::GridTooShort)
    );
    assert_eq!(
        logarithmic_grid(0.0, 10.0, 5),
        Err(FrequencyError::InvalidInput)
    );
    assert_eq!(
        logarithmic_grid(10.0, 1.0, 5),
        Err(FrequencyError::InvalidInput)
    );
    assert_eq!(
        logarithmic_grid(1.0, f64::INFINITY, 5),
        Err(FrequencyError::InvalidInput)
    );
}

#[test]
fn transfer_response_matches_the_second_order_check_at_one_rad_per_second() {
    // G(s) = 1/(s² + 3s + 2) at s = j: 1/(1 + 3j) = 0.1 − 0.3j, |G| = 1/√10 (−10 dB), phase −atan 3.
    let value = first_value(&continuous(&[1.0], &[1.0, 3.0, 2.0], 0.0), 1.0);
    assert_close(value.real, 0.1, 1e-12);
    assert_close(value.imaginary, -0.3, 1e-12);
    assert_close(magnitude_db(value), -10.0, 1e-9);
    assert_close(phase_degrees(value), -(3.0_f64).atan().to_degrees(), 1e-9);
}

#[test]
fn integrator_response_is_k_over_j_omega() {
    // G(s) = 2/s at ω = 0.5: 2/(0.5j) = −4j. Magnitude K/ω = 4, phase −90°.
    let value = first_value(&continuous(&[2.0], &[1.0, 0.0], 0.0), 0.5);
    assert_close(value.real, 0.0, 1e-12);
    assert_close(value.imaginary, -4.0, 1e-12);
    assert_close(phase_degrees(value), -90.0, 1e-9);
}

#[test]
fn pure_delay_multiplies_by_exp_minus_j_omega_l() {
    // A unit gain with delay π/2 at ω = 1 rad/s: exp(−jπ/2) = −j.
    let value = first_value(&continuous(&[1.0], &[1.0], PI / 2.0), 1.0);
    assert_close(value.real, 0.0, 1e-12);
    assert_close(value.imaginary, -1.0, 1e-12);
}

#[test]
fn discrete_response_is_evaluated_at_exp_j_omega_ts() {
    // G(z) = 1/(z − 0.5), Ts = 0.1 s. At ωTs = π/2, z = j: G = 1/(j − 0.5) = −0.4 − 0.8j.
    let model = sampled(&[1.0], &[1.0, -0.5], 0.1);
    let value = first_value(&model, 5.0 * PI);
    assert_close(value.real, -0.4, 1e-9);
    assert_close(value.imaginary, -0.8, 1e-9);
    // The Nyquist frequency π / Ts is rejected, not evaluated (N4.3).
    assert_eq!(
        response_at(&model, PI / 0.1),
        Err(FrequencyError::AboveNyquist)
    );
}

#[test]
fn discrete_delay_must_be_whole_samples_and_is_a_power_of_z() {
    let fractional = FrequencyModel::Transfer {
        transfer: transfer(&[1.0], &[1.0, -0.5]),
        timing: Timing::Discrete {
            sample_time_seconds: 0.1,
        },
        delay_seconds: 0.25,
    };
    assert_eq!(
        response_at(&fractional, 5.0 * PI),
        Err(FrequencyError::NonIntegerDiscreteDelay)
    );
    // Three samples of delay: z⁻³ = j⁻³ = j at z = j, so (−0.4 − 0.8j)·j = 0.8 − 0.4j.
    let whole = FrequencyModel::Transfer {
        transfer: transfer(&[1.0], &[1.0, -0.5]),
        timing: Timing::Discrete {
            sample_time_seconds: 0.1,
        },
        delay_seconds: 0.3,
    };
    let value = first_value(&whole, 5.0 * PI);
    assert_close(value.real, 0.8, 1e-9);
    assert_close(value.imaginary, -0.4, 1e-9);
}

#[test]
fn a_pole_on_the_evaluation_point_is_singular() {
    // s² + 1 vanishes at s = j: the request is refused, not answered with an infinite point.
    let model = continuous(&[1.0], &[1.0, 0.0, 1.0], 0.0);
    assert_eq!(response_at(&model, 1.0), Err(FrequencyError::Singular));
}

#[test]
fn state_space_response_matches_its_transfer_function_and_keeps_output_order() {
    let single = FrequencyModel::StateSpace {
        system: StateSpace::new(
            vec![vec![-1.0]],
            vec![vec![1.0]],
            vec![vec![1.0]],
            vec![vec![0.0]],
        )
        .expect("valid system"),
        timing: Timing::Continuous,
        delay_seconds: 0.0,
    };
    // 1/(s + 1) at s = j: 1/(1 + j) = 0.5 − 0.5j, the same as the transfer function.
    let value = first_value(&single, 1.0);
    let expected = first_value(&continuous(&[1.0], &[1.0, 1.0], 0.0), 1.0);
    assert_close(value.real, 0.5, 1e-12);
    assert_close(value.imaginary, -0.5, 1e-12);
    assert_close(value.real, expected.real, 1e-12);
    assert_close(value.imaginary, expected.imaginary, 1e-12);

    // Two outputs from one input: 1/(s + 1) and 1/(s + 2). At s = j the second is 1/(2 + j) = 0.4 − 0.2j.
    let two_outputs = FrequencyModel::StateSpace {
        system: StateSpace::new(
            vec![vec![-1.0, 0.0], vec![0.0, -2.0]],
            vec![vec![1.0], vec![1.0]],
            vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            vec![vec![0.0], vec![0.0]],
        )
        .expect("valid system"),
        timing: Timing::Continuous,
        delay_seconds: 0.0,
    };
    let rows = response_at(&two_outputs, 1.0).expect("response");
    assert_eq!(rows.len(), 2);
    assert_close(rows[0][0].real, 0.5, 1e-12);
    assert_close(rows[1][0].real, 0.4, 1e-12);
    assert_close(rows[1][0].imaginary, -0.2, 1e-12);
}

#[test]
fn discrete_state_space_response_agrees_with_the_discrete_transfer_function() {
    // x(k+1) = 0.5x(k) + u(k): G(z) = 1/(z − 0.5). At z = j the value is −0.4 − 0.8j.
    let model = FrequencyModel::StateSpace {
        system: StateSpace::new(
            vec![vec![0.5]],
            vec![vec![1.0]],
            vec![vec![1.0]],
            vec![vec![0.0]],
        )
        .expect("valid system"),
        timing: Timing::Discrete {
            sample_time_seconds: 0.1,
        },
        delay_seconds: 0.0,
    };
    let value = first_value(&model, 5.0 * PI);
    assert_close(value.real, -0.4, 1e-9);
    assert_close(value.imaginary, -0.8, 1e-9);
}

#[test]
fn selected_loop_is_the_product_of_plant_and_controller_and_needs_one_timing() {
    // Plant 1/(s + 1) and controller 2 at ω = 1: L = 2/(1 + j) = 1 − j.
    let plant = continuous(&[1.0], &[1.0, 1.0], 0.0);
    let controller = continuous(&[2.0], &[1.0], 0.0);
    let loop_value = selected_loop_response(&plant, &controller, 1.0).expect("loop");
    assert_close(loop_value.real, 1.0, 1e-12);
    assert_close(loop_value.imaginary, -1.0, 1e-12);

    let sampled_controller = sampled(&[2.0], &[1.0], 0.1);
    assert_eq!(
        selected_loop_response(&plant, &sampled_controller, 1.0),
        Err(FrequencyError::TimingMismatch)
    );
}

#[test]
fn sensitivities_of_a_constant_loop_are_closed_form() {
    // L = 4: S = 1/(1 + 4) = 0.2 and T = 4/(1 + 4) = 0.8.
    let loop_value = ComplexValue {
        real: 4.0,
        imaginary: 0.0,
    };
    let sensitivity_value = sensitivity(loop_value).expect("sensitivity");
    let complementary = complementary_sensitivity(loop_value).expect("complementary");
    assert_close(sensitivity_value.real, 0.2, 1e-12);
    assert_close(complementary.real, 0.8, 1e-12);
    // L = −1 makes 1 + L zero: refused, never infinite.
    let minus_one = ComplexValue {
        real: -1.0,
        imaginary: 0.0,
    };
    assert_eq!(sensitivity(minus_one), Err(FrequencyError::Singular));
}

#[test]
fn sensitivity_of_the_precision_drive_loop_matches_the_test_suite_values() {
    // Plant 10/(s² + 0.6s + 1) with a static controller 0.4: L = 4/(s² + 0.6s + 1).
    // At ω = 0.01 the test suite expects |S| = 0.2000 (±0.001); at ω = 10, |T| = 0.04202 (±0.0001).
    let plant = continuous(&[10.0], &[1.0, 0.6, 1.0], 0.0);
    let controller = continuous(&[0.4], &[1.0], 0.0);
    let low = selected_loop_response(&plant, &controller, 0.01).expect("loop");
    let low_sensitivity = sensitivity(low).expect("sensitivity");
    assert_close(modulus(low_sensitivity), 0.2000, 1e-3);
    let high = selected_loop_response(&plant, &controller, 10.0).expect("loop");
    let high_complementary = complementary_sensitivity(high).expect("complementary");
    assert_close(modulus(high_complementary), 0.04202, 1e-4);
}

/// The unit response at the angle `degrees`.
fn at_angle(degrees: f64) -> ComplexValue {
    ComplexValue {
        real: degrees.to_radians().cos(),
        imaginary: degrees.to_radians().sin(),
    }
}

#[test]
fn a_plotted_phase_starts_at_the_principal_phase_and_moves_each_point_by_whole_turns() {
    // N4.3: the principal phases 170°, −170°, −10° and 100° at increasing frequencies. The first point keeps 170°; −170° is
    // moved up one turn to 190° (a step of +20°, not −340°), −10° to 350° (+160°) and 100° to 460° (+110°). Catches a wrapped
    // series, which jumps from 170° to −170°.
    let values = [170.0, -170.0, -10.0, 100.0].map(at_angle);
    let phases = plotted_phase_degrees(&values, &[1.0, 2.0, 3.0, 4.0], 0.0);
    assert_eq!(phases.len(), 4);
    for (phase, expected) in phases.iter().zip([170.0, 190.0, 350.0, 460.0]) {
        assert_close(*phase, expected, 1e-9);
    }
}

#[test]
fn a_half_turn_step_is_taken_forward_and_a_negative_zero_imaginary_part_reads_180_degrees() {
    // N4.3: a step lies in (−180°, 180°], so 90° (the value j) followed by −90° (−j), a step of exactly −180°, becomes 90° and
    // 270°. Catches an unwrap that keeps the half turn backward (90°, −90°). The first value −2 − 0j, which atan2 reads as
    // −180°, takes the principal phase 180°: catches a first point outside (−180°, 180°].
    let quarter_turns = [
        ComplexValue {
            real: 0.0,
            imaginary: 1.0,
        },
        ComplexValue {
            real: 0.0,
            imaginary: -1.0,
        },
    ];
    let half_turn = plotted_phase_degrees(&quarter_turns, &[1.0, 2.0], 0.0);
    assert_close(half_turn[0], 90.0, 1e-12);
    assert_close(half_turn[1], 270.0, 1e-12);
    let negative_zero = ComplexValue {
        real: -2.0,
        imaginary: -0.0,
    };
    assert_close(
        plotted_phase_degrees(&[negative_zero], &[1.0], 0.0)[0],
        180.0,
        1e-12,
    );
}

#[test]
fn the_delay_phase_is_added_in_closed_form_however_coarse_the_grid() {
    // N4.3: a delay-free phase of 0° at 0.1, 10 and 1000 rad/s with a delay of L = 1 s: the plotted phase is −ω·L·180°/π,
    // −5.729578°, −572.957795° and −57,295.779513°. The full response e^(−jωL) has the principal phases −5.73°, 147.04° and
    // −55.78° there, which a plain unwrap reads as −5.73°, 147.04° and 304.22°. Catches an unwrap of the full response, which
    // loses the turns of the delay between sparse points.
    let one = ComplexValue {
        real: 1.0,
        imaginary: 0.0,
    };
    let phases = plotted_phase_degrees(&[one, one, one], &[0.1, 10.0, 1000.0], 1.0);
    assert_close(phases[0], -5.729_577_951_308_23, 1e-9);
    assert_close(phases[1], -572.957_795_130_823, 1e-9);
    assert_close(phases[2], -57_295.779_513_082_3, 1e-8);
}
