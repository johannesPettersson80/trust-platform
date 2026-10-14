use super::*;

#[test]
fn a_zero_pivot_is_replaced_by_the_formal_epsilon_and_its_sign_is_the_limit() {
    // s⁴ + s² + 2s + 3 (the TypeScript test "counts ... a zero pivot ... with the formal ε rule").
    // Hand calculation: the pivots are 1, ε, (ε − 2)/ε, F = 2 − 3ε²/(ε − 2) = (−3ε² + 2ε − 4)/(ε − 2) and 3, so their
    // limit signs are +, +, −, + (F → 2), +: two sign changes, the count the TypeScript test asserts.
    let table = exact_routh_signs(&[1.0, 0.0, 1.0, 2.0, 3.0]).expect("a table");
    assert!(table.zero_pivot);
    assert!(!table.uncertain);
    assert!(!table.boundary);
    assert_eq!(table.signs, vec![1, 1, -1, 1, 1]);
    assert_eq!(table.rows[0].entries, vec!["1", "1", "3"]);
    assert_eq!(table.rows[1].entries, vec!["1ε^1", "2", "0"]);
}

#[test]
fn a_polynomial_the_table_does_not_take_has_no_table() {
    assert!(
        exact_routh_signs(&[0.0, 1.0]).is_none(),
        "a zero leading coefficient"
    );
    assert!(exact_routh_signs(&[1.0]).is_none(), "a single coefficient");
    assert!(
        exact_routh_signs(&[1.0, f64::NAN]).is_none(),
        "a coefficient that is not finite"
    );
}

#[test]
fn an_origin_root_is_a_boundary() {
    // s² + s has a root at the origin: its constant coefficient is zero.
    let table = exact_routh_signs(&[1.0, 1.0, 0.0]).expect("a table");
    assert!(table.boundary);
    assert!(!table.uncertain);
}

#[test]
fn a_common_root_of_the_even_and_odd_parts_is_an_imaginary_axis_boundary() {
    // s³ + 2s² + s + 2 = (s + 2)(s² + 1): the even part 2s² + 2 and the odd part s² + 1 (after s) share
    // the root s² = −1, so the polynomial has roots ±j on the imaginary axis.
    let table = exact_routh_signs(&[1.0, 2.0, 1.0, 2.0]).expect("a table");
    assert!(!table.uncertain);
    assert!(table.boundary);
}
