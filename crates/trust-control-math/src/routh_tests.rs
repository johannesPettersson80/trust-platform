use super::*;

const TOLERANCE: f64 = 1e-12;

fn entries(table: &Table) -> Vec<(usize, Vec<f64>)> {
    table
        .rows
        .iter()
        .map(|row| (row.power, row.entries.clone()))
        .collect()
}

#[test]
fn a_stable_quadratic_has_the_expected_table() {
    // Ported from control-studio-routh-hurwitz.test.ts: s² + 5s + 6.
    let table = build(&[1.0, 5.0, 6.0], TOLERANCE);
    assert_eq!(
        entries(&table),
        vec![
            (2, vec![1.0, 6.0]),
            (1, vec![5.0, 0.0]),
            (0, vec![6.0, 0.0])
        ]
    );
    assert_eq!(table.stop, None);
    assert_eq!(table.zero_rows, [] as [ZeroRow; 0]);
}

#[test]
fn an_unstable_quartic_has_the_expected_first_column() {
    // s³ + 2s² + s + 4: first column 1, 2, −1, 4, with two sign changes.
    let table = build(&[1.0, 2.0, 1.0, 4.0], TOLERANCE);
    let first: Vec<f64> = table.rows.iter().map(|row| row.entries[0]).collect();
    assert_eq!(first, vec![1.0, 2.0, -1.0, 4.0]);
    assert_eq!(table.rows[2].calculations[0].result, -1.0);
}

#[test]
fn a_zero_pivot_stops_the_float_table_without_an_epsilon() {
    // s⁴ + s² + 2s + 3: the pivot of the s³ row is exactly zero.
    let table = build(&[1.0, 0.0, 1.0, 2.0, 3.0], TOLERANCE);
    assert_eq!(table.stop, Some(Stop::ZeroPivot { power: 3 }));
    assert_eq!(table.rows.len(), 2);
}

#[test]
fn a_zero_constant_gives_an_all_zero_computed_row() {
    // s² + s: the s⁰ row is computed and is all zero, so the construction ends there.
    let table = build(&[1.0, 1.0, 0.0], TOLERANCE);
    assert_eq!(table.zero_rows, vec![ZeroRow::Computed { power: 0 }]);
    assert_eq!(table.rows.len(), 3);
}

#[test]
fn an_auxiliary_row_replaces_an_all_zero_row_with_the_derivative_of_its_polynomial() {
    // s⁴ + 2s² + 1: the s³ row is zero, so the derivative of the auxiliary polynomial 4s⁴ + 4s² + 0 replaces it.
    let table = build(&[1.0, 0.0, 2.0, 0.0, 1.0], TOLERANCE);
    assert_eq!(table.zero_rows[0], ZeroRow::Replaced { power: 3 });
    assert_eq!(table.rows[1].entries, vec![4.0, 4.0, 0.0]);
    assert_eq!(
        table.zero_rows.last(),
        Some(&ZeroRow::Computed { power: 1 })
    );
}

#[test]
fn the_auxiliary_derivative_row_weights_each_entry_by_its_power() {
    // A row (1, 2, 0) below the power 4: the weights are 4, 2 and 0, and a weight of zero is dropped.
    assert_eq!(
        auxiliary_derivative_row(&[1.0, 2.0, 0.0], 4, 3),
        vec![4.0, 4.0, 0.0]
    );
}

#[test]
fn a_near_zero_pivot_is_reported_against_the_polynomial_scale() {
    // The first pivot of the s row is 1e-13 against a scale of 1: below the 1e-12 relative tolerance.
    let table = build(&[1.0, 1e-13, 1.0], TOLERANCE);
    assert_eq!(table.stop, Some(Stop::NearZeroPivot { power: 1 }));
}
