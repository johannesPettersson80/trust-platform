use super::*;
use crate::linalg::ComplexValue;
use crate::model::TransferFunction;
use crate::polynomial::{add_aligned, convolve, roots};

/// The tolerance of the gain and polynomial checks.
const TOLERANCE: f64 = 1e-12;
/// The tolerance of the roots, which the eigenvalue solver delivers to about this accuracy on these polynomials.
const ROOT_TOLERANCE: f64 = 1e-7;

/// The request of the reference placements: damping 0.8, a 10 ms task and the 0.2 sample-rate limit (N5.12(c)).
fn request(decay_rate: f64) -> PlacementRequest {
    PlacementRequest {
        decay_rate,
        damping: 0.8,
        minimum_stiffness: 0.0,
        sample_time_seconds: 0.01,
        maximum_omega_sample_product: 0.2,
    }
}

fn transfer(numerator: &[f64], denominator: &[f64]) -> TransferFunction {
    TransferFunction::new(numerator.to_vec(), denominator.to_vec())
        .expect("a valid transfer function")
}

fn plant(numerator: &[f64], denominator: &[f64]) -> PlacementPlant {
    PlacementPlant::new(&transfer(numerator, denominator), 0.0).expect("a placement plant")
}

fn place(
    numerator: &[f64],
    denominator: &[f64],
    family: Family,
    decay_rate: f64,
) -> Result<ParallelGains, PlacementRefusal> {
    place_poles(plant(numerator, denominator), family, request(decay_rate))
}

/// The characteristic polynomial of the loop, built here from the loop definition: `s·D(s) + b·(Kd·s² + Kp·s + Ki)` with the
/// integral term, and `D(s) + b·(Kd·s + Kp)` without it.
fn loop_polynomial(
    denominator: &[f64],
    gain: f64,
    parallel: ParallelGains,
    with_integral: bool,
) -> Vec<f64> {
    let proportional = gain * parallel.proportional_gain;
    let integral_term = gain * parallel.integral_gain;
    let derivative = gain * parallel.derivative_gain;
    if with_integral {
        add_aligned(
            &convolve(&[1.0, 0.0], denominator),
            &[derivative, proportional, integral_term],
        )
    } else {
        add_aligned(denominator, &[derivative, proportional])
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "expected {expected}, got {actual}"
    );
}

/// Asserts that the polynomial has exactly the expected roots `(real, imaginary)`, in any order.
fn assert_roots(polynomial: &[f64], expected: &[(f64, f64)]) {
    let found = roots(polynomial).expect("the roots of the closed loop");
    assert_eq!(found.len(), expected.len(), "the number of roots");
    for &(real, imaginary) in expected {
        assert!(
            found.iter().any(|root: &ComplexValue| {
                (root.real - real).abs() <= ROOT_TOLERANCE
                    && (root.imaginary - imaginary).abs() <= ROOT_TOLERANCE
            }),
            "no root at {real} + {imaginary}j among {found:?}"
        );
    }
}

#[test]
fn a_first_order_proportional_loop_places_its_pole_at_the_decay_rate() {
    // G = 2/(s + 1): Kp = (σ·a − a0)/b = (3 − 1)/2 = 1 puts the single pole at −3.
    let gains = place(&[2.0], &[1.0, 1.0], Family::P, 3.0).expect("a P placement");
    assert_close(gains.proportional_gain, 1.0);
    assert_close(gains.integral_gain, 0.0);
    assert_close(gains.derivative_gain, 0.0);
    assert_roots(
        &loop_polynomial(&[1.0, 1.0], 2.0, gains, false),
        &[(-3.0, 0.0)],
    );
}

#[test]
fn a_first_order_pi_loop_places_the_damped_pair_at_the_decay_rate() {
    // G = 2/(s + 1), σ = 1, ζ = 0.8, ω = 1.25: Kp = (2ζω·a − a0)/b = 0.5 and Ki = a·ω²/b = 0.78125.
    // The pair is −ζω ± jω√(1 − ζ²) = −1 ± 0.75j, whose real part is the decay rate σ.
    let gains = place(&[2.0], &[1.0, 1.0], Family::PI, 1.0).expect("a PI placement");
    assert_close(gains.proportional_gain, 0.5);
    assert_close(gains.integral_gain, 0.78125);
    assert_close(gains.derivative_gain, 0.0);
    assert_roots(
        &loop_polynomial(&[1.0, 1.0], 2.0, gains, true),
        &[(-1.0, 0.75), (-1.0, -0.75)],
    );
}

#[test]
fn an_unstable_first_order_plant_is_stabilised_by_pi() {
    // G = 2/(s − 1): a0 = −1, so Kp = (2ζω + 1)/2 = 1.5 and Ki = 0.78125. The loop is s² + 2s + 1.5625.
    let gains = place(&[2.0], &[1.0, -1.0], Family::PI, 1.0).expect("a PI placement");
    assert_close(gains.proportional_gain, 1.5);
    assert_close(gains.integral_gain, 0.78125);
    assert_roots(
        &loop_polynomial(&[1.0, -1.0], 2.0, gains, true),
        &[(-1.0, 0.75), (-1.0, -0.75)],
    );
}

#[test]
fn a_reverse_plant_gives_gains_of_the_plant_sign() {
    // G = −2/(s + 1): b < 0, so Kp = (2ζω − a0)/b = −0.5 and Ki = −0.78125, both negative in the magnitude frame.
    let gains = place(&[-2.0], &[1.0, 1.0], Family::PI, 1.0).expect("a PI placement");
    assert_close(gains.proportional_gain, -0.5);
    assert_close(gains.integral_gain, -0.78125);
    assert_roots(
        &loop_polynomial(&[1.0, 1.0], -2.0, gains, true),
        &[(-1.0, 0.75), (-1.0, -0.75)],
    );
}

#[test]
fn the_minimum_stiffness_raises_the_proportional_pole_for_the_steady_error() {
    let mut placement = request(1.0);
    placement.minimum_stiffness = 5.0;
    let gains =
        place_poles(plant(&[2.0], &[1.0, 1.0]), Family::P, placement).expect("a P placement");
    // S = max(a·σ, 5) = 5, so Kp = (5 − 1)/2 = 2 and the pole is at −5.
    assert_close(gains.proportional_gain, 2.0);
    assert_roots(
        &loop_polynomial(&[1.0, 1.0], 2.0, gains, false),
        &[(-5.0, 0.0)],
    );
}

#[test]
fn a_gain_on_the_wrong_side_of_the_magnitude_frame_refuses_the_placement() {
    // The plant is already faster than σ = 0.5: S = 0.5 < a0 = 1, so Kp = −0.25 while b = 2.
    assert_eq!(
        place(&[2.0], &[1.0, 1.0], Family::P, 0.5),
        Err(PlacementRefusal::NegativeGain)
    );
}

#[test]
fn a_derivative_term_is_refused_on_a_first_order_plant() {
    assert_eq!(
        place(&[2.0], &[1.0, 1.0], Family::PD, 1.0),
        Err(PlacementRefusal::DerivativeOnFirstOrder)
    );
    assert_eq!(
        place(&[2.0], &[1.0, 1.0], Family::PID, 1.0),
        Err(PlacementRefusal::DerivativeOnFirstOrder)
    );
}

#[test]
fn a_second_order_proportional_loop_places_the_natural_frequency_and_keeps_the_plant_decay() {
    // G = 1/(s² + 3s + 1), σ = 1, ω = 1.25: S = a·ω² = 1.5625, so Kp = 0.5625. The plant's decay 3/2 is at least σ.
    // The loop is s² + 3s + 1.5625, with real roots whose product is ω² and whose sum is −a1/a.
    let gains = place(&[1.0], &[1.0, 3.0, 1.0], Family::P, 1.0).expect("a P placement");
    assert_close(gains.proportional_gain, 0.5625);
    let root = 2.75_f64.sqrt();
    assert_roots(
        &loop_polynomial(&[1.0, 3.0, 1.0], 1.0, gains, false),
        &[
            (f64::midpoint(-3.0, root), 0.0),
            (f64::midpoint(-3.0, -root), 0.0),
        ],
    );
}

#[test]
fn a_second_order_proportional_loop_refuses_a_decay_the_plant_cannot_reach() {
    // G = 1/(s² + 1.5s + 1): its decay 0.75 is below σ = 1, and proportional action cannot change it.
    assert_eq!(
        place(&[1.0], &[1.0, 1.5, 1.0], Family::P, 1.0),
        Err(PlacementRefusal::ProportionalDecayTooSlow)
    );
}

#[test]
fn a_second_order_pd_loop_places_the_damped_pair() {
    // G = 1/(s² + 1.5s + 1), σ = 1, ζ = 0.8: S = 1.5625 gives Kp = 0.5625 and ω_r = 1.25, so Kd = (2ζω_r·a − a1)/b = 0.5.
    let gains = place(&[1.0], &[1.0, 1.5, 1.0], Family::PD, 1.0).expect("a PD placement");
    assert_close(gains.proportional_gain, 0.5625);
    assert_close(gains.integral_gain, 0.0);
    assert_close(gains.derivative_gain, 0.5);
    assert_roots(
        &loop_polynomial(&[1.0, 1.5, 1.0], 1.0, gains, false),
        &[(-1.0, 0.75), (-1.0, -0.75)],
    );
}

#[test]
fn a_derivative_gain_on_the_wrong_side_of_the_magnitude_frame_refuses_the_placement() {
    // G = 1/(s² + 3s + 1): Kd = 2ζω·a − a1 = 2 − 3 = −1 for b = 1, so the placement is refused.
    assert_eq!(
        place(&[1.0], &[1.0, 3.0, 1.0], Family::PD, 1.0),
        Err(PlacementRefusal::NegativeGain)
    );
}

#[test]
fn a_second_order_pid_loop_places_the_pair_and_its_third_pole() {
    // G = 1/(s² + 3s + 2), σ = 1, ω = 1.25: the third pole is 3ω = 3.75.
    // Kd = 2.75, Kp = 7.0625 and Ki = 5.859375, so the loop is (s² + 2s + 1.5625)(s + 3.75).
    let gains = place(&[1.0], &[1.0, 3.0, 2.0], Family::PID, 1.0).expect("a PID placement");
    assert_close(gains.proportional_gain, 7.0625);
    assert_close(gains.integral_gain, 5.859_375);
    assert_close(gains.derivative_gain, 2.75);
    assert_roots(
        &loop_polynomial(&[1.0, 3.0, 2.0], 1.0, gains, true),
        &[(-1.0, 0.75), (-1.0, -0.75), (-3.75, 0.0)],
    );
}

#[test]
fn a_second_order_pi_loop_places_the_fastest_pair_its_damping_allows() {
    // G = 1/(s² + 9.2s + 1): w = a1/(a(2ζ + 3)) = 2, which is faster than ω = 1.25.
    // Kp = a·w²(1 + 6ζ) − a0 = 22.2 and Ki = 3·a·w³ = 24, so the loop is (s² + 3.2s + 4)(s + 6).
    let gains = place(&[1.0], &[1.0, 9.2, 1.0], Family::PI, 1.0).expect("a PI placement");
    assert_close(gains.proportional_gain, 22.2);
    assert_close(gains.integral_gain, 24.0);
    assert_roots(
        &loop_polynomial(&[1.0, 9.2, 1.0], 1.0, gains, true),
        &[(-1.6, 1.2), (-1.6, -1.2), (-6.0, 0.0)],
    );
}

#[test]
fn a_second_order_pi_loop_refuses_a_pair_slower_than_requested_or_without_damping() {
    // G = 1/(s² + 1.5s + 1): w = 1.5/4.6 ≈ 0.33, which is below ω = 1.25.
    assert_eq!(
        place(&[1.0], &[1.0, 1.5, 1.0], Family::PI, 1.0),
        Err(PlacementRefusal::IntegralTooSlow)
    );
    // G = 1/(s² − s + 1): the linear coefficient is not positive, so integral action has no damping to use.
    assert_eq!(
        place(&[1.0], &[1.0, -1.0, 1.0], Family::PI, 1.0),
        Err(PlacementRefusal::IntegralWithoutDamping)
    );
}

#[test]
fn the_sample_rate_limit_is_strict() {
    // ω = σ/ζ = 1.25: at h = 0.1 s, ω·h = 0.125 is below 0.2; at h = 0.2 s, ω·h = 0.25 is not.
    let mut placement = request(1.0);
    placement.sample_time_seconds = 0.1;
    assert!(place_poles(plant(&[2.0], &[1.0, 1.0]), Family::PI, placement).is_ok());
    placement.sample_time_seconds = 0.2;
    assert_eq!(
        place_poles(plant(&[2.0], &[1.0, 1.0]), Family::PI, placement),
        Err(PlacementRefusal::SampleRate)
    );
}

#[test]
fn a_delay_zeros_order_or_zero_gain_is_not_a_placement_plant() {
    assert_eq!(
        PlacementPlant::new(&transfer(&[2.0], &[1.0, 1.0]), 0.5),
        Err(PlacementRefusal::Delay)
    );
    assert_eq!(
        PlacementPlant::new(&transfer(&[1.0, 1.0], &[1.0, 2.0, 3.0]), 0.0),
        Err(PlacementRefusal::Zeros)
    );
    assert_eq!(
        PlacementPlant::new(&transfer(&[1.0], &[1.0, 2.0, 3.0, 4.0]), 0.0),
        Err(PlacementRefusal::Order)
    );
    assert_eq!(
        PlacementPlant::new(&transfer(&[0.0], &[1.0, 1.0]), 0.0),
        Err(PlacementRefusal::ZeroGain)
    );
    assert_eq!(
        PlacementPlant::new(&transfer(&[1.0], &[-1.0, 1.0]), 0.0),
        Err(PlacementRefusal::LeadingCoefficient)
    );
}

#[test]
fn a_nonpositive_or_nonfinite_request_is_refused() {
    let mut placement = request(1.0);
    placement.decay_rate = 0.0;
    assert_eq!(
        place_poles(plant(&[2.0], &[1.0, 1.0]), Family::P, placement),
        Err(PlacementRefusal::InvalidInput)
    );
    placement.decay_rate = 1.0;
    placement.damping = f64::NAN;
    assert_eq!(
        place_poles(plant(&[2.0], &[1.0, 1.0]), Family::P, placement),
        Err(PlacementRefusal::InvalidInput)
    );
}
