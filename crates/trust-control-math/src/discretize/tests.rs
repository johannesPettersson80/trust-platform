#![allow(clippy::float_cmp)] // The sample period and the copied C and D are compared exactly.

use super::*;

fn assert_matrix_close(actual: &[Vec<f64>], expected: &[Vec<f64>], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "row count");
    for (row, (actual_row, expected_row)) in actual.iter().zip(expected).enumerate() {
        for (column, (value, reference)) in actual_row.iter().zip(expected_row).enumerate() {
            assert!(
                (value - reference).abs() <= tolerance,
                "entry ({row}, {column}): {value} differs from {reference}"
            );
        }
    }
}

fn single_input_plant(a: Vec<Vec<f64>>, b: Vec<Vec<f64>>) -> StateSpace {
    let states = a.len();
    let mut c = vec![0.0; states];
    if let Some(first) = c.first_mut() {
        *first = 1.0;
    }
    StateSpace::new(a, b, vec![c], vec![vec![0.0]]).expect("consistent shapes")
}

#[test]
fn the_four_methods_match_their_formulas_on_a_first_order_plant() {
    // dx/dt = −2x + 3u sampled at T = 0.1 s: the reference values are the closed forms of each
    // method, with C = 1 and D = 0 copied through unchanged.
    let model = single_input_plant(vec![vec![-2.0]], vec![vec![3.0]]);
    let exact = (-0.2_f64).exp();
    let cases = [
        (DiscretizationMethod::ForwardEuler, 0.8, 0.3),
        (DiscretizationMethod::BackwardEuler, 1.0 / 1.2, 0.25),
        (DiscretizationMethod::Tustin, 0.9 / 1.1, 0.3 / 1.1),
        (DiscretizationMethod::ExactZoh, exact, 1.5 * (1.0 - exact)),
    ];
    for (method, expected_a, expected_b) in cases {
        let discrete = discretize_state_space(&model, 0.1, method).expect("the plant qualifies");
        assert!(
            (discrete.system.a[0][0] - expected_a).abs() <= 1e-10,
            "{method:?}: A"
        );
        assert!(
            (discrete.system.b[0][0] - expected_b).abs() <= 1e-10,
            "{method:?}: B"
        );
        assert_eq!(discrete.system.c, vec![vec![1.0]]);
        assert_eq!(discrete.system.d, vec![vec![0.0]]);
        assert_eq!(discrete.method, method);
        assert_eq!(discrete.sample_time_seconds, 0.1);
    }
}

#[test]
fn the_exact_zero_order_hold_matches_closed_forms_for_three_plants() {
    const TOLERANCE: f64 = 1e-12;

    // 1. First order: dx/dt = −x/τ + (K/τ)u with τ = 2 s, K = 3 and T = 0.5 s.
    //    Ad = e^{−T/τ} and Bd = K(1 − e^{−T/τ}).
    let (tau, gain, period) = (2.0_f64, 3.0_f64, 0.5_f64);
    let first_order = single_input_plant(vec![vec![-1.0 / tau]], vec![vec![gain / tau]]);
    let discrete = discretize_state_space(&first_order, period, DiscretizationMethod::ExactZoh)
        .expect("the first-order plant qualifies");
    let decay = (-period / tau).exp();
    assert!((discrete.system.a[0][0] - decay).abs() <= TOLERANCE);
    assert!((discrete.system.b[0][0] - gain * (1.0 - decay)).abs() <= TOLERANCE);

    // 2. Double integrator: A = [[0, 1], [0, 0]], B = [[0], [1]], T = 0.1 s.
    //    Ad = [[1, T], [0, 1]] and Bd = [[T²/2], [T]].
    let period = 0.1_f64;
    let integrator = StateSpace::new(
        vec![vec![0.0, 1.0], vec![0.0, 0.0]],
        vec![vec![0.0], vec![1.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let discrete = discretize_state_space(&integrator, period, DiscretizationMethod::ExactZoh)
        .expect("the double integrator qualifies");
    assert_matrix_close(
        &discrete.system.a,
        &[vec![1.0, period], vec![0.0, 1.0]],
        TOLERANCE,
    );
    assert_matrix_close(
        &discrete.system.b,
        &[vec![period * period / 2.0], vec![period]],
        TOLERANCE,
    );

    // 3. Lightly damped oscillator: A = [[0, 1], [−ω², 0]], B = [[0], [1]], ω = 2 rad/s, T = 0.25 s.
    //    With θ = ωT: Ad = [[cos θ, sin θ/ω], [−ω sin θ, cos θ]] and Bd = [[(1 − cos θ)/ω²], [sin θ/ω]].
    let (omega, period) = (2.0_f64, 0.25_f64);
    let theta = omega * period;
    let oscillator = StateSpace::new(
        vec![vec![0.0, 1.0], vec![-omega * omega, 0.0]],
        vec![vec![0.0], vec![1.0]],
        vec![vec![1.0, 0.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let discrete = discretize_state_space(&oscillator, period, DiscretizationMethod::ExactZoh)
        .expect("the oscillator qualifies");
    assert_matrix_close(
        &discrete.system.a,
        &[
            vec![theta.cos(), theta.sin() / omega],
            vec![-omega * theta.sin(), theta.cos()],
        ],
        TOLERANCE,
    );
    assert_matrix_close(
        &discrete.system.b,
        &[
            vec![(1.0 - theta.cos()) / (omega * omega)],
            vec![theta.sin() / omega],
        ],
        TOLERANCE,
    );
}

#[test]
fn backward_euler_refuses_a_singular_resolvent() {
    // I − T·A = 1 − 0.1·10 = 0 for A = 10 at T = 0.1 s.
    let model = single_input_plant(vec![vec![10.0]], vec![vec![1.0]]);
    let refused = discretize_state_space(&model, 0.1, DiscretizationMethod::BackwardEuler)
        .expect_err("the resolvent has no inverse");
    assert_eq!(refused.reason, NumericalRejectionReason::Singular);
}

#[test]
fn discretization_refuses_a_nonpositive_or_nonfinite_sample_period() {
    let model = single_input_plant(vec![vec![-1.0]], vec![vec![1.0]]);
    for period in [0.0, -0.1, f64::NAN, f64::INFINITY] {
        let refused = discretize_state_space(&model, period, DiscretizationMethod::ForwardEuler)
            .expect_err("the sample period is not valid");
        assert_eq!(refused.reason, NumericalRejectionReason::NonfiniteInput);
    }
}
