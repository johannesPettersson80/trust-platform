use super::*;

fn tolerance(absolute: f64, relative: f64, condition_limit: f64) -> NumericalTolerance {
    NumericalTolerance {
        absolute,
        relative,
        condition_limit,
    }
}

fn refusal(result: Result<DenseMatrix, NumericalRejection>) -> NumericalRejectionReason {
    result.expect_err("the matrix is refused").reason
}

#[test]
fn the_inverse_of_a_diagonal_matrix_holds_the_reciprocals() {
    // diag(4, 1) has the inverse diag(1/4, 1); its condition number is 4, within the limit 1e8.
    let inverse = invert(
        &vec![vec![4.0, 0.0], vec![0.0, 1.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    )
    .expect("the matrix is invertible");
    assert!((inverse[0][0] - 0.25).abs() <= 1e-12);
    assert!(inverse[0][1].abs() <= 1e-12);
    assert!(inverse[1][0].abs() <= 1e-12);
    assert!((inverse[1][1] - 1.0).abs() <= 1e-12);
}

#[test]
fn a_rank_deficient_matrix_is_refused_as_singular() {
    // The second row is twice the first, so the rank is one.
    let result = invert(
        &vec![vec![1.0, 2.0], vec![2.0, 4.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    );
    assert_eq!(refusal(result), NumericalRejectionReason::Singular);
}

#[test]
fn a_condition_number_above_the_limit_is_refused() {
    // diag(1e9, 1) has rank two at this tolerance, but its condition number 1e9 exceeds the limit 1e8.
    let result = invert(
        &vec![vec![1e9, 0.0], vec![0.0, 1.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    );
    assert_eq!(refusal(result), NumericalRejectionReason::IllConditioned);
}

#[test]
fn a_rectangular_matrix_is_not_square() {
    let result = invert(
        &vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    );
    assert_eq!(refusal(result), NumericalRejectionReason::MatrixNotSquare);
}

#[test]
fn an_invalid_tolerance_or_a_nonfinite_entry_is_refused() {
    let identity = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let zero_tolerance = invert(&identity, &tolerance(0.0, 1e-10, 1e8));
    assert_eq!(
        refusal(zero_tolerance),
        NumericalRejectionReason::NonfiniteInput
    );
    let nonfinite = invert(
        &vec![vec![f64::NAN, 0.0], vec![0.0, 1.0]],
        &tolerance(1e-12, 1e-10, 1e8),
    );
    assert_eq!(refusal(nonfinite), NumericalRejectionReason::NonfiniteInput);
}
