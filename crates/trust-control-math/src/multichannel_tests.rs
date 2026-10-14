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
