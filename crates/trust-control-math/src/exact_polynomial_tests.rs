use super::*;

fn rat(numerator: i64, denominator: i64) -> Rational {
    Rational::new(big(numerator), big(denominator)).expect("a rational with a nonzero denominator")
}

fn poly(values: &[f64]) -> RationalPolynomial {
    RationalPolynomial::from_decimals(values).expect("finite decimal coefficients")
}

#[test]
fn a_decimal_coefficient_is_the_rational_its_text_denotes() {
    // The TypeScript converts `String(value)` exactly: 0.1 is one tenth, not the nearest double.
    assert_eq!(Rational::from_decimal(0.1).expect("exact"), rat(1, 10));
    assert_eq!(Rational::from_decimal(-2.5).expect("exact"), rat(-5, 2));
    assert_eq!(
        Rational::from_decimal(-2.000_001).expect("exact"),
        rat(-2_000_001, 1_000_000)
    );
    assert_eq!(
        Rational::from_decimal(1e-7).expect("exact"),
        rat(1, 10_000_000)
    );
    assert_eq!(Rational::from_decimal(0.0).expect("exact"), rat(0, 1));
    assert_eq!(Rational::from_decimal(f64::NAN), Err(ExactError::Nonfinite));
}

#[test]
fn rational_arithmetic_is_exact_and_refuses_a_zero_divisor() {
    assert_eq!(rat(1, 3).add(&rat(1, 6)).expect("sum"), rat(1, 2));
    assert_eq!(rat(1, 2).mul(&rat(2, 3)).expect("product"), rat(1, 3));
    assert_eq!(rat(1, 2).div(&rat(1, 4)).expect("quotient"), rat(2, 1));
    assert_eq!(rat(1, 2).div(&rat(0, 1)), Err(ExactError::DivisionByZero));
    assert_eq!(rat(2, 4).to_string(), "1/2");
    assert_eq!(rat(4, 2).to_string(), "2");
    assert_eq!(rat(-3, 6).to_string(), "-1/2");
}

#[test]
fn long_division_gives_the_quotient_and_the_remainder() {
    // x² + 1 = (x − 1)(x + 1) + 2
    let (quotient, remainder) = poly(&[1.0, 0.0, 1.0])
        .divide(&poly(&[1.0, -1.0]))
        .expect("division");
    assert_eq!(quotient, poly(&[1.0, 1.0]));
    assert_eq!(remainder, poly(&[2.0]));
}

#[test]
fn square_free_part_removes_the_repeated_root() {
    // (x − 1)²(x + 2) = x³ − 3x + 2, whose square-free part is (x − 1)(x + 2) = x² + x − 2.
    let square_free = poly(&[1.0, 0.0, -3.0, 2.0])
        .square_free()
        .expect("square-free");
    assert_eq!(square_free, poly(&[1.0, 1.0, -2.0]));
}

#[test]
fn primitive_form_is_positive_scaling_without_common_factors() {
    // 0.5x + 1 has the primitive form x + 2 (a positive scaling by 2).
    assert_eq!(
        poly(&[0.5, 1.0]).primitive().expect("primitive"),
        poly(&[1.0, 2.0])
    );
}

#[test]
fn determinant_of_a_two_by_two_matrix_of_polynomials() {
    // [[x, 1], [1, x]] has determinant x² − 1.
    let x = poly(&[1.0, 0.0]);
    let one = RationalPolynomial::constant(Rational::one());
    let matrix = vec![vec![x.clone(), one.clone()], vec![one, x]];
    assert_eq!(
        determinant(&matrix).expect("determinant"),
        poly(&[1.0, 0.0, -1.0])
    );
}

#[test]
fn a_matrix_that_is_not_square_of_order_one_to_sixteen_is_refused() {
    assert_eq!(determinant(&[]), Err(ExactError::MatrixOrder));
    let one = RationalPolynomial::constant(Rational::one());
    assert_eq!(
        determinant(&[vec![one.clone(), one.clone()]]),
        Err(ExactError::MatrixOrder)
    );
}

#[test]
fn a_polynomial_of_more_than_256_degrees_is_refused() {
    let coefficients = vec![Rational::one(); MAX_COEFFICIENTS + 1];
    assert_eq!(
        RationalPolynomial::new(coefficients),
        Err(ExactError::Degree)
    );
}

#[test]
fn leading_zeros_are_removed_and_the_zero_polynomial_keeps_one_coefficient() {
    assert_eq!(
        poly(&[0.0, 0.0, 3.0]),
        RationalPolynomial::constant(rat(3, 1))
    );
    assert!(poly(&[0.0, 0.0]).is_zero());
    assert_eq!(poly(&[0.0, 0.0]).coefficients().len(), 1);
}
