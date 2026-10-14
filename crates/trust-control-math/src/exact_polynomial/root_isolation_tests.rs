use super::*;

fn poly(values: &[f64]) -> RationalPolynomial {
    RationalPolynomial::from_decimals(values).expect("finite decimal coefficients")
}

#[test]
fn isolates_exact_rational_polynomial_roots_including_repeated_signed_zero_and_close_roots() {
    // Ported from control-studio-numerics.test.ts: the first test of the exact numerical adapter.
    let repeated = isolate_real_roots(&poly(&[1.0, -2.0, 1.0])).expect("isolation");
    assert!(repeated
        .iter()
        .any(|interval| interval.lower <= 1.0 && interval.upper >= 1.0));
    assert_eq!(repeated.len(), 1, "a repeated root is isolated once");

    let signed = isolate_real_roots(&poly(&[1.0, 0.0, -1.0])).expect("isolation");
    assert!(signed.iter().any(|interval| interval.upper < 0.0));
    assert!(signed.iter().any(|interval| interval.lower > 0.0));

    let zero = isolate_real_roots(&poly(&[1.0, 0.0, 0.0])).expect("isolation");
    assert!(zero
        .iter()
        .any(|interval| interval.lower == 0.0 && interval.upper == 0.0));

    // The roots 1 and 1.000001 are nearby rationals; they must not be merged.
    let close = isolate_real_roots(&poly(&[1.0, -2.000_001, 1.000_001])).expect("isolation");
    assert!(close.len() >= 2, "nearby rational roots must not be merged");

    // A root at a bisection point (0 of the interval [-2, 2]) must not hide its neighbours.
    let split = isolate_real_roots(&poly(&[1.0, 0.0, -1.0, 0.0])).expect("isolation");
    assert_eq!(
        split.len(),
        3,
        "a root at a bisection point must not hide either adjacent root"
    );
}

#[test]
fn the_zero_polynomial_has_no_isolated_roots_and_a_constant_has_none() {
    assert_eq!(
        isolate_real_roots(&poly(&[0.0])),
        Err(ExactError::ZeroPolynomial)
    );
    assert_eq!(
        isolate_real_roots(&poly(&[3.0])).expect("isolation"),
        Vec::new()
    );
}

#[test]
fn the_sturm_count_changes_sign_where_a_root_lies() {
    // For x² − 1 the sequence is x² − 1, 2x, 1. At −2 the signs are +, −, +: two changes; at +2, none.
    let sequence = sturm_sequence(&poly(&[1.0, 0.0, -1.0])).expect("sequence");
    let minus_two = Rational::integer(-2);
    let plus_two = Rational::integer(2);
    assert_eq!(sturm_variations(&sequence, &minus_two).expect("count"), 2);
    assert_eq!(sturm_variations(&sequence, &plus_two).expect("count"), 0);
}
