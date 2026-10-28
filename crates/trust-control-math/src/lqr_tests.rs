use super::*;

const LIMITS: HorizonLimits = HorizonLimits {
    maximum_steps: 512,
    maximum_states: 32,
    maximum_inputs: 16,
};

fn tolerance() -> NumericalTolerance {
    NumericalTolerance {
        absolute: 1e-12,
        relative: 1e-10,
        condition_limit: 1e12,
    }
}

fn scalar(value: f64) -> DenseMatrix {
    vec![vec![value]]
}

fn assert_close(actual: &DenseMatrix, expected: &DenseMatrix, tolerance: f64) {
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

#[test]
fn the_scalar_riccati_equation_has_the_golden_ratio_solution() {
    // a = b = q = r = 1: P = 1 + P/(1 + P), so P² − P − 1 = 0 and P = (1 + √5)/2. The gain is P/(1 + P) = (√5 − 1)/2 and the
    // closed-loop pole is 1 − K = (3 − √5)/2.
    let solution = solve_discrete_lqr(
        &scalar(1.0),
        &scalar(1.0),
        &scalar(1.0),
        &scalar(1.0),
        &tolerance(),
        None,
        LIMITS,
    )
    .expect("the scalar problem is stabilisable");
    let root_five = 5.0_f64.sqrt();
    assert!((solution.riccati[0][0] - f64::midpoint(1.0, root_five)).abs() <= 1e-9);
    assert!((solution.gain[0][0] - (root_five - 1.0) / 2.0).abs() <= 1e-9);
    assert!((solution.closed_loop_poles[0].real - (3.0 - root_five) / 2.0).abs() <= 1e-9);
    assert!(solution
        .riccati_residual
        .is_some_and(|residual| residual <= 1e-10));
    assert!(solution.finite.is_none());
}

#[test]
fn a_two_step_horizon_keeps_both_gains_and_all_three_riccati_stages() {
    // From P₂ = 1: K₁ = 1/(1 + 1) = 0.5 and P₁ = 0.5 + 1 = 1.5; then K₀ = 1.5/2.5 = 0.6 and P₀ = 0.6 + 1 = 1.6.
    let horizon = FiniteHorizon {
        steps: 2,
        terminal_weights: scalar(1.0),
    };
    let solution = solve_discrete_lqr(
        &scalar(1.0),
        &scalar(1.0),
        &scalar(1.0),
        &scalar(1.0),
        &tolerance(),
        Some(&horizon),
        LIMITS,
    )
    .expect("the finite horizon is qualified");
    let stages = solution.finite.expect("a finite horizon keeps its stages");
    assert_eq!(stages.gain_sequence.len(), 2);
    assert_close(&stages.gain_sequence[0], &scalar(0.6), 1e-12);
    assert_close(&stages.gain_sequence[1], &scalar(0.5), 1e-12);
    assert_eq!(stages.riccati_sequence.len(), 3);
    assert_close(&stages.riccati_sequence[0], &scalar(1.6), 1e-12);
    assert_close(&stages.riccati_sequence[1], &scalar(1.5), 1e-12);
    assert_close(&stages.riccati_sequence[2], &scalar(1.0), 1e-12);
    assert_close(&solution.gain, &scalar(0.6), 1e-12);
    assert_close(&solution.riccati, &scalar(1.6), 1e-12);
    assert!(solution.riccati_residual.is_none());
    assert!(stages.repeated_first_gain_stable);
}

#[test]
fn a_horizon_outside_its_limits_or_with_a_refused_weight_is_invalid() {
    let horizon = |steps: usize| FiniteHorizon {
        steps,
        terminal_weights: scalar(1.0),
    };
    let run = |steps: usize, r: DenseMatrix, limits: HorizonLimits| {
        solve_discrete_lqr(
            &scalar(1.0),
            &scalar(1.0),
            &scalar(1.0),
            &r,
            &tolerance(),
            Some(&horizon(steps)),
            limits,
        )
    };
    assert_eq!(
        run(0, scalar(1.0), LIMITS),
        Err(LqrRefusal::InvalidFiniteHorizon)
    );
    assert_eq!(
        run(513, scalar(1.0), LIMITS),
        Err(LqrRefusal::InvalidFiniteHorizon)
    );
    let no_states_allowed = HorizonLimits {
        maximum_states: 0,
        ..LIMITS
    };
    assert_eq!(
        run(2, scalar(1.0), no_states_allowed),
        Err(LqrRefusal::InvalidFiniteHorizon)
    );
    // R must be strictly positive: a zero control weight is refused.
    assert_eq!(
        run(2, scalar(0.0), LIMITS),
        Err(LqrRefusal::InvalidFiniteHorizon)
    );
    // A terminal weight that is not symmetric is refused, here in a two-state problem.
    let asymmetric = solve_discrete_lqr(
        &vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        &vec![vec![1.0], vec![0.0]],
        &vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        &scalar(1.0),
        &tolerance(),
        Some(&FiniteHorizon {
            steps: 2,
            terminal_weights: vec![vec![1.0, 0.5], vec![0.0, 1.0]],
        }),
        LIMITS,
    );
    assert_eq!(asymmetric, Err(LqrRefusal::InvalidFiniteHorizon));
}

#[test]
fn a_steady_state_weight_that_is_not_admissible_is_malformed() {
    let run = |q: DenseMatrix, r: DenseMatrix| {
        solve_discrete_lqr(
            &scalar(1.0),
            &scalar(1.0),
            &q,
            &r,
            &tolerance(),
            None,
            LIMITS,
        )
    };
    // A negative state weight and a zero input weight are refused, as the finite horizon refuses them.
    assert_eq!(
        run(scalar(-1.0), scalar(1.0)),
        Err(LqrRefusal::MalformedWeight)
    );
    assert_eq!(
        run(scalar(1.0), scalar(0.0)),
        Err(LqrRefusal::MalformedWeight)
    );
    // A state weight that is not symmetric is refused, here in a two-state problem.
    let asymmetric = solve_discrete_lqr(
        &vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        &vec![vec![1.0], vec![0.0]],
        &vec![vec![1.0, 0.5], vec![0.0, 1.0]],
        &scalar(1.0),
        &tolerance(),
        None,
        LIMITS,
    );
    assert_eq!(asymmetric, Err(LqrRefusal::MalformedWeight));
    // Admissible weights still solve.
    assert!(run(scalar(1.0), scalar(1.0)).is_ok());
}

#[test]
fn a_shape_that_does_not_fit_the_steady_state_problem_is_refused() {
    // The input matrix has two columns, but R is 1×1 and must be 2×2 for two inputs.
    let result = solve_discrete_lqr(
        &scalar(1.0),
        &vec![vec![1.0, 2.0]],
        &scalar(1.0),
        &scalar(1.0),
        &tolerance(),
        None,
        LIMITS,
    );
    assert_eq!(
        result,
        Err(LqrRefusal::Numerical(
            NumericalRejectionReason::InconsistentDimensions
        ))
    );
}
