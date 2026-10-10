use super::*;

fn tolerance(absolute: f64, relative: f64, condition_limit: f64) -> NumericalTolerance {
    NumericalTolerance {
        absolute,
        relative,
        condition_limit,
    }
}

fn refusal_reason<T: std::fmt::Debug>(
    result: Result<T, NumericalRejection>,
) -> NumericalRejectionReason {
    result.expect_err("the computation is refused").reason
}

#[test]
fn least_squares_recovers_the_exact_line() {
    // y = 1 + 2x sampled at x = 0, 1, 2: the exact fit is (1, 2) with zero residual.
    let coefficients = vec![vec![1.0, 0.0], vec![1.0, 1.0], vec![1.0, 2.0]];
    let fit = solve_least_squares(
        &coefficients,
        &[1.0, 3.0, 5.0],
        &tolerance(1e-12, 1e-10, 1e8),
    )
    .expect("the exact line is identified");
    assert!((fit.solution[0] - 1.0).abs() <= 1e-10);
    assert!((fit.solution[1] - 2.0).abs() <= 1e-10);
    assert!(fit.residual_norm <= 1e-10);
    assert_eq!(fit.rank, 2);
}

#[test]
fn least_squares_rejects_bad_inputs_with_their_reasons() {
    let loose = tolerance(1e-12, 1e-10, 1e6);
    assert_eq!(
        refusal_reason(solve_least_squares(
            &vec![vec![1.0, f64::NAN]],
            &[1.0],
            &loose
        )),
        NumericalRejectionReason::NonfiniteInput
    );
    assert_eq!(
        refusal_reason(solve_least_squares(
            &vec![vec![1.0, 0.0]],
            &[1.0, 2.0],
            &loose
        )),
        NumericalRejectionReason::InconsistentDimensions
    );
    assert_eq!(
        refusal_reason(solve_least_squares(
            &vec![vec![1.0, 1.0], vec![2.0, 2.0]],
            &[1.0, 2.0],
            &loose
        )),
        NumericalRejectionReason::Singular
    );
    // Singular values are about 2 and 5e-10; the threshold is 2e-10, so the matrix has rank 2
    // and the ratio about 4e9 exceeds the limit 1e6.
    assert_eq!(
        refusal_reason(solve_least_squares(
            &vec![vec![1.0, 1.0], vec![1.0, 1.0 + 1e-9]],
            &[2.0, 2.0 + 1e-9],
            &loose
        )),
        NumericalRejectionReason::IllConditioned
    );
    assert_eq!(
        refusal_reason(solve_least_squares(
            &vec![vec![1.0, 2.0]],
            &[1.0],
            &tolerance(0.0, 1e-10, 1e6)
        )),
        NumericalRejectionReason::NonfiniteInput,
        "a zero absolute tolerance is not valid"
    );
}

#[test]
fn eigenvalues_of_a_real_matrix_are_its_roots() {
    // det(λI − A) = (λ + 1)(λ + 2) for A = [[0, 1], [−2, −3]].
    let solution = eigenvalues(
        &vec![vec![0.0, 1.0], vec![-2.0, -3.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    )
    .expect("the eigenvalues are qualified");
    let mut reals: Vec<f64> = solution.values.iter().map(|value| value.real).collect();
    reals.sort_by(f64::total_cmp);
    assert!((reals[0] + 2.0).abs() <= 1e-10);
    assert!((reals[1] + 1.0).abs() <= 1e-10);
    assert!(solution
        .values
        .iter()
        .all(|value| value.imaginary.abs() <= 1e-12));
}

#[test]
fn eigenvalues_of_a_rotation_are_a_conjugate_pair() {
    // A = [[0, −2], [2, 0]] has eigenvalues ±2i.
    let solution = eigenvalues(
        &vec![vec![0.0, -2.0], vec![2.0, 0.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    )
    .expect("the eigenvalues are qualified");
    assert!(solution
        .values
        .iter()
        .all(|value| value.real.abs() <= 1e-10));
    let mut imaginary: Vec<f64> = solution
        .values
        .iter()
        .map(|value| value.imaginary)
        .collect();
    imaginary.sort_by(f64::total_cmp);
    assert!((imaginary[0] + 2.0).abs() <= 1e-10);
    assert!((imaginary[1] - 2.0).abs() <= 1e-10);
}

#[test]
fn eigenvalues_reject_a_matrix_that_is_not_square() {
    assert_eq!(
        refusal_reason(eigenvalues(
            &vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
            &tolerance(1e-12, 1e-10, 1e6)
        )),
        NumericalRejectionReason::MatrixNotSquare
    );
}

#[test]
fn numerical_rank_counts_singular_values_above_the_threshold() {
    let loose = tolerance(1e-12, 1e-10, 1e8);
    assert_eq!(
        numerical_rank(&vec![vec![4.0, 0.0], vec![0.0, 1.0]], &loose).expect("ranked"),
        2
    );
    // The second row is twice the first: one singular value is zero to rounding.
    assert_eq!(
        numerical_rank(&vec![vec![1.0, 2.0], vec![2.0, 4.0]], &loose).expect("ranked"),
        1
    );
}
