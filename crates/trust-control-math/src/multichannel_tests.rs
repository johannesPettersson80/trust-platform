//! Tests of the static gain matrix rules (specification 35, C7; numerical annex N4.4 and N4.5). Each value is a closed form or
//! a product that the test checks by hand.

use super::*;

/// The tolerance of the tests: the closed forms are exact to rounding.
const TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

fn assert_matrix_close(actual: &DenseMatrix, expected: &[&[f64]], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "row count");
    for (row, values) in expected.iter().enumerate() {
        assert_eq!(actual[row].len(), values.len(), "column count of row {row}");
        for (column, value) in values.iter().enumerate() {
            assert!(
                (actual[row][column] - value).abs() <= tolerance,
                "entry ({row}, {column}) is {}, expected {value}",
                actual[row][column]
            );
        }
    }
}

#[test]
fn the_relative_gain_array_of_a_two_by_two_matches_its_closed_form() {
    // K = [[2, 1], [1, 2]]: K⁻¹ = [[2/3, −1/3], [−1/3, 2/3]], so Λ = K ∘ (K⁻¹)ᵀ = [[4/3, −1/3], [−1/3, 4/3]].
    let gain = vec![vec![2.0, 1.0], vec![1.0, 2.0]];
    let relative = relative_gain_array(&gain, &TOLERANCE).expect("K is nonsingular");
    assert_matrix_close(
        &relative,
        &[&[4.0 / 3.0, -1.0 / 3.0], &[-1.0 / 3.0, 4.0 / 3.0]],
        1e-12,
    );
}

#[test]
fn the_relative_gains_of_any_nonsingular_gain_sum_to_one_along_each_row_and_column() {
    // Λ·1 = 1 and 1ᵀ·Λ = 1 hold for any nonsingular K, since Σⱼ kᵢⱼ(K⁻¹)ⱼᵢ = (K·K⁻¹)ᵢᵢ = 1.
    let gain = vec![
        vec![1.0, 2.0, 0.0],
        vec![0.0, 1.0, 3.0],
        vec![4.0, 0.0, 1.0],
    ];
    let relative = relative_gain_array(&gain, &TOLERANCE).expect("det K = 25");
    for row in &relative {
        assert!((row.iter().sum::<f64>() - 1.0).abs() < 1e-9, "row sum");
    }
    for column in 0..3 {
        let sum: f64 = relative.iter().map(|row| row[column]).sum();
        assert!((sum - 1.0).abs() < 1e-9, "column sum");
    }
}

#[test]
fn a_singular_gain_has_no_relative_gain_array_and_no_decoupler() {
    let singular = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
    assert!(relative_gain_array(&singular, &TOLERANCE).is_err());
    assert!(static_decoupler(&singular, &TOLERANCE).is_err());
}

#[test]
fn the_static_decoupler_diagonalizes_the_gain_it_is_built_from() {
    // K = [[2, 1], [0, 4]]: K⁻¹ = [[1/2, −1/8], [0, 1/4]], so D = K⁻¹·diag(K) = [[1, −1/2], [0, 1]].
    // Then K·D = [[2, 0], [0, 4]]: through D, each member's output moves only its own measurement.
    let gain = vec![vec![2.0, 1.0], vec![0.0, 4.0]];
    let decoupler = static_decoupler(&gain, &TOLERANCE).expect("K is nonsingular");
    assert_matrix_close(&decoupler, &[&[1.0, -0.5], &[0.0, 1.0]], 1e-12);
    let product = [
        [
            gain[0][0] * decoupler[0][0] + gain[0][1] * decoupler[1][0],
            gain[0][0] * decoupler[0][1] + gain[0][1] * decoupler[1][1],
        ],
        [
            gain[1][0] * decoupler[0][0] + gain[1][1] * decoupler[1][0],
            gain[1][0] * decoupler[0][1] + gain[1][1] * decoupler[1][1],
        ],
    ];
    assert!(product[0][1].abs() < 1e-12 && product[1][0].abs() < 1e-12);
    assert!((product[0][0] - 2.0).abs() < 1e-12 && (product[1][1] - 4.0).abs() < 1e-12);
}

#[test]
fn the_pairing_takes_the_diagonal_closest_to_one() {
    let direct = vec![vec![1.1, -0.1], vec![-0.1, 1.1]];
    assert_eq!(pairing_by_relative_gain(&direct), vec![0, 1]);
    // The crossed pairing has relative gains near 1.19, the direct one near −0.19: the crossed pairing is chosen.
    let crossed = vec![vec![-0.19, 1.19], vec![1.19, -0.19]];
    assert_eq!(pairing_by_relative_gain(&crossed), vec![1, 0]);
    assert_eq!(pairing_by_relative_gain(&Vec::new()), [] as [usize; 0]);
}

#[test]
fn a_continuous_steady_state_gain_is_minus_a_inverse_b() {
    // A = [[−0.13, 0.04], [0.04, −0.13]] has det A = 0.0153, so −A⁻¹ = (1/0.0153)·[[0.13, 0.04], [0.04, 0.13]].
    // With B = 0.01·I and C = I, K = 0.01/0.0153 · [[0.13, 0.04], [0.04, 0.13]] = [[0.085, 0.026], [0.026, 0.085]].
    let system = StateSpace {
        a: vec![vec![-0.13, 0.04], vec![0.04, -0.13]],
        b: vec![vec![0.01, 0.0], vec![0.0, 0.01]],
        c: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        d: vec![vec![0.0, 0.0], vec![0.0, 0.0]],
    };
    let gain = steady_state_gain(&system, false).expect("A is nonsingular");
    let k = 0.01 / 0.0153;
    assert_matrix_close(
        &gain,
        &[&[0.13 * k, 0.04 * k], &[0.04 * k, 0.13 * k]],
        1e-12,
    );
}

#[test]
fn a_discrete_steady_state_gain_is_c_times_i_minus_a_inverse_b() {
    // x(k+1) = 0.5·x(k) + u(k), y = x: the steady gain is 1/(1 − 0.5) = 2.
    let system = StateSpace {
        a: vec![vec![0.5]],
        b: vec![vec![1.0]],
        c: vec![vec![1.0]],
        d: vec![vec![0.0]],
    };
    assert_matrix_close(
        &steady_state_gain(&system, true).expect("1 − 0.5 ≠ 0"),
        &[&[2.0]],
        1e-12,
    );
}

#[test]
fn the_feedthrough_is_added_to_the_steady_state_gain() {
    // ẋ = −x + u, y = x + 0.5·u: the steady gain is −(1/−1)·1 + 0.5 = 1.5.
    let system = StateSpace {
        a: vec![vec![-1.0]],
        b: vec![vec![1.0]],
        c: vec![vec![1.0]],
        d: vec![vec![0.5]],
    };
    assert_matrix_close(
        &steady_state_gain(&system, false).expect("A = −1 is nonsingular"),
        &[&[1.5]],
        1e-12,
    );
}

#[test]
fn a_state_equation_without_a_solution_has_no_steady_state_gain() {
    // ẋ = 0·x + u has no steady state for a constant input: −A is zero, and its pivot is refused.
    let system = StateSpace {
        a: vec![vec![0.0]],
        b: vec![vec![1.0]],
        c: vec![vec![1.0]],
        d: vec![vec![0.0]],
    };
    assert!(steady_state_gain(&system, false).is_none());
}

#[test]
fn the_least_squares_gain_of_two_independent_trials_is_the_closed_form_gain() {
    // Each trial steps one input by 5, so the outputs move by 5 times a column of K = [9 4; 4 9] / 65 (lesson 69).
    let inputs: DenseMatrix = vec![vec![5.0, 0.0], vec![0.0, 5.0]];
    let outputs: DenseMatrix = vec![
        vec![45.0 / 65.0, 20.0 / 65.0],
        vec![20.0 / 65.0, 45.0 / 65.0],
    ];
    let fit = identify_static_gain(&inputs, &outputs, &TOLERANCE).expect("two independent trials");
    assert_matrix_close(
        &fit.gain,
        &[&[9.0 / 65.0, 4.0 / 65.0], &[4.0 / 65.0, 9.0 / 65.0]],
        1e-12,
    );
    assert_eq!(fit.input_rank, 2);
    assert!(fit.residual_norm < 1e-12);
}

#[test]
fn trials_that_do_not_pair_have_no_static_gain() {
    // One trial cannot fit two inputs, and a row of the outputs with another length does not pair with the inputs.
    let one_trial: DenseMatrix = vec![vec![5.0, 0.0]];
    let outputs: DenseMatrix = vec![vec![1.0, 2.0]];
    let rejection = identify_static_gain(&one_trial, &outputs, &TOLERANCE)
        .expect_err("one trial is not enough");
    assert_eq!(
        rejection.reason,
        NumericalRejectionReason::InconsistentDimensions
    );
    let inputs: DenseMatrix = vec![vec![5.0, 0.0], vec![0.0, 5.0]];
    let ragged: DenseMatrix = vec![vec![1.0, 2.0], vec![3.0]];
    let rejection =
        identify_static_gain(&inputs, &ragged, &TOLERANCE).expect_err("a ragged row does not pair");
    assert_eq!(
        rejection.reason,
        NumericalRejectionReason::InconsistentDimensions
    );
}

#[test]
fn input_steps_that_are_not_independent_have_no_static_gain() {
    // Both trials step the first input alone, so the second input is never excited: the input matrix has rank one.
    let inputs: DenseMatrix = vec![vec![5.0, 0.0], vec![5.0, 0.0]];
    let outputs: DenseMatrix = vec![vec![1.0, 0.0], vec![2.0, 0.0]];
    let rejection = identify_static_gain(&inputs, &outputs, &TOLERANCE)
        .expect_err("the steps are not independent");
    assert_eq!(rejection.reason, NumericalRejectionReason::Singular);
}

#[test]
fn the_named_pairing_takes_the_positive_diagonal_closest_to_one() {
    // The identity pairing has relative gains 1.2 and 1.2; the crossed pairing has −0.2 on its diagonal, so it is refused.
    let relative: DenseMatrix = vec![vec![1.2, -0.2], vec![-0.2, 1.2]];
    assert_eq!(named_pairing(&relative), Some(vec![0, 1]));
}

#[test]
fn between_two_positive_pairings_the_one_closer_to_one_is_named() {
    let identity_closer: DenseMatrix = vec![vec![0.9, 0.1], vec![0.1, 0.9]];
    assert_eq!(named_pairing(&identity_closer), Some(vec![0, 1]));
    let crossed_closer: DenseMatrix = vec![vec![0.1, 0.9], vec![0.9, 0.1]];
    assert_eq!(named_pairing(&crossed_closer), Some(vec![1, 0]));
}

#[test]
fn equally_close_pairings_name_the_earlier_one() {
    let relative: DenseMatrix = vec![vec![0.5, 0.5], vec![0.5, 0.5]];
    assert_eq!(named_pairing(&relative), Some(vec![0, 1]));
}

#[test]
fn no_pairing_is_named_when_no_pairing_has_every_diagonal_element_positive() {
    // The first two rows are positive only in the first column, so no pairing takes a positive element from each row (N4.5).
    let relative: DenseMatrix = vec![
        vec![1.2, -0.1, -0.1],
        vec![1.2, -0.1, -0.1],
        vec![-1.4, 1.2, 1.2],
    ];
    assert_eq!(named_pairing(&relative), None);
}

#[test]
fn the_condition_number_is_the_ratio_of_the_largest_to_the_smallest_singular_value() {
    let gain: DenseMatrix = vec![vec![4.0, 0.0], vec![0.0, 1.0]];
    let number = condition_number(&gain).expect("a diagonal gain has a ratio");
    assert!((number - 4.0).abs() < 1e-12);
    let rectangular: DenseMatrix = vec![vec![1.0, 2.0]];
    assert_eq!(condition_number(&rectangular), None);
}

#[test]
fn the_relative_gain_array_of_lesson_69s_gain_is_81_over_65_on_the_diagonal() {
    // K = [9 4; 4 9] / 65 °C per point (lesson 69): λ11 = (9/65)·9 and λ12 = (4/65)·(−4).
    let gain: DenseMatrix = vec![vec![9.0 / 65.0, 4.0 / 65.0], vec![4.0 / 65.0, 9.0 / 65.0]];
    let relative = relative_gain_array(&gain, &TOLERANCE).expect("a nonsingular gain");
    assert_matrix_close(
        &relative,
        &[&[81.0 / 65.0, -16.0 / 65.0], &[-16.0 / 65.0, 81.0 / 65.0]],
        1e-12,
    );
}
