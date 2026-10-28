#![allow(clippy::float_cmp)] // The tests compare exact values on purpose.

use super::*;

fn transfer(numerator: &[f64], denominator: &[f64], delay_seconds: f64) -> Transfer {
    Transfer {
        numerator: numerator.to_vec(),
        denominator: denominator.to_vec(),
        delay_seconds,
    }
}

fn sweep(minimum: f64, maximum: f64, points_per_decade: u32) -> Sweep {
    Sweep {
        minimum_rad_per_second: minimum,
        maximum_rad_per_second: maximum,
        points_per_decade,
    }
}

fn sorted_real_parts(values: &[ComplexValue]) -> Vec<f64> {
    let mut parts: Vec<f64> = values.iter().map(|value| value.real).collect();
    parts.sort_by(f64::total_cmp);
    parts
}

#[test]
fn a_logarithmic_sweep_has_the_ends_and_the_points_per_decade() {
    // From 1 to 100 rad/s at ten points per decade: 20 intervals and 21 points, the last one at 100.
    let frequencies = logarithmic_frequencies(sweep(1.0, 100.0, 10)).expect("a sweep");
    assert_eq!(frequencies.len(), 21);
    assert_eq!(frequencies[0], 1.0);
    assert!((frequencies[20] - 100.0).abs() < 1e-9);
    assert!(
        (frequencies[10] - 10.0).abs() < 1e-9,
        "the middle point is the decade's end"
    );
}

#[test]
fn an_invalid_sweep_is_refused() {
    assert_eq!(
        logarithmic_frequencies(sweep(0.0, 1.0, 10)),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(
        logarithmic_frequencies(sweep(1.0, 1.0, 10)),
        Err(ClassicalError::InvalidInput)
    );
    assert_eq!(
        logarithmic_frequencies(sweep(1.0, 10.0, 9)),
        Err(ClassicalError::InvalidInput)
    );
}

#[test]
fn a_second_order_transfer_function_has_the_expected_poles_and_bode_point() {
    // Ported from control-studio-numerics.test.ts: 1/(s² + 3s + 2) at ω = 1.
    let analysis =
        analyze_transfer(&transfer(&[1.0], &[1.0, 3.0, 2.0], 0.0), &[1.0]).expect("analysis");
    assert_eq!(sorted_real_parts(&analysis.poles), vec![-2.0, -1.0]);
    assert_eq!(analysis.zeros, [] as [ComplexValue; 0]);
    assert!((analysis.bode[0].magnitude_db + 10.0).abs() <= 1e-10);
    assert!((analysis.bode[0].phase_degrees + 71.565_051_177_1).abs() <= 1e-8);
    assert!(analysis.bode[0].sensitivity_complementarity_residual <= 1e-12);
}

#[test]
fn the_classical_margins_of_a_type_one_loop_match_the_reference_values() {
    // Ported from control-studio-numerics.test.ts: L = 10/(s² + s) on 1e-3 to 1e3 rad/s, 120 points per decade.
    let margins = classical_stability_margins(
        &transfer(&[10.0], &[1.0, 1.0, 0.0], 0.0),
        sweep(1e-3, 1e3, 120),
    )
    .expect("margins");
    assert!((margins.gain_crossover_rad_per_second - 3.084_232_837_7).abs() <= 0.01);
    assert!((margins.phase_margin_degrees - 17.964_235_92).abs() <= 0.08);
    assert_eq!(
        margins.gain_margin,
        GainMargin::Unbounded,
        "no phase crossover is found, so it is not infinite"
    );
    assert!((margins.delay_margin_seconds - 0.10165).abs() <= 0.002);
}

#[test]
fn a_loop_without_a_gain_crossover_on_the_grid_is_refused() {
    // L = 0.001/(s + 1) never reaches 0 dB on 1 to 10 rad/s.
    let result =
        classical_stability_margins(&transfer(&[0.001], &[1.0, 1.0], 0.0), sweep(1.0, 10.0, 10));
    assert_eq!(result, Err(ClassicalError::SolverFailed));
}

#[test]
fn a_zero_numerator_and_a_negative_delay_are_refused_for_the_bode_data() {
    assert_eq!(
        analyze_transfer(&transfer(&[0.0], &[1.0, 1.0], 0.0), &[1.0]),
        Err(ClassicalError::InvalidPolynomial)
    );
    assert_eq!(
        analyze_transfer(&transfer(&[1.0], &[1.0, 1.0], -0.5), &[1.0]),
        Err(ClassicalError::InvalidPolynomial)
    );
    assert_eq!(
        analyze_transfer(&transfer(&[1.0], &[1.0, 1.0], 0.0), &[]),
        Err(ClassicalError::InvalidInput)
    );
}

#[test]
fn a_frequency_at_a_pole_on_the_imaginary_axis_is_refused_as_singular() {
    // 1/(s² + 1) at ω = 1 is 1/0: the pole is exactly on the grid, so the response is not finite (N4.3).
    let result = analyze_transfer(&transfer(&[1.0], &[1.0, 0.0, 1.0], 0.0), &[1.0]);
    assert_eq!(result, Err(ClassicalError::NonfiniteResponse));
}

#[test]
fn a_delay_multiplies_the_response_by_a_unit_phase_factor() {
    // A delay of 0.5 s at ω = 2 rad/s turns the phase by −1 rad, and leaves the magnitude alone.
    let plain = transfer_value(&transfer(&[1.0], &[1.0, 1.0], 0.0), 2.0);
    let delayed = transfer_value(&transfer(&[1.0], &[1.0, 1.0], 0.5), 2.0);
    assert!((magnitude(plain) - magnitude(delayed)).abs() < 1e-12);
    let rotation = delayed.imaginary.atan2(delayed.real) - plain.imaginary.atan2(plain.real);
    assert!((rotation + 1.0).abs() < 1e-12);
}

#[test]
fn roots_of_a_real_polynomial_are_its_eigenvalues() {
    // s² − 3s + 2 = (s − 1)(s − 2).
    let roots = roots_of_real_polynomial(&[1.0, -3.0, 2.0]).expect("roots");
    assert_eq!(sorted_real_parts(&roots), vec![1.0, 2.0]);
    assert_eq!(
        roots_of_real_polynomial(&[0.0, 0.0]),
        None,
        "a polynomial with no coefficient above 1e-15"
    );
}
