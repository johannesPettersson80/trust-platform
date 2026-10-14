//! The exact Routh table of a real polynomial, with a formal positive infinitesimal `ε` for a zero
//! pivot and the auxiliary polynomial for an all-zero row (N7.3).
//!
//! Every entry is a quotient of two polynomials in `ε` with exact rational coefficients. A zero pivot
//! is replaced by `ε`, so the sign of a pivot is the sign of its limit as `ε` tends to zero from above.
//! The construction is exact: an intermediate that exceeds the bit budget ends the table as uncertain,
//! and no rounding is made.

use crate::exact_polynomial::{big, isolate_real_roots, ExactError, Rational, RationalPolynomial};

/// The exact table of a polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Each flag is one independent finding of the construction (N7.5), shown as such.
pub struct ExactTable {
    /// The sign of each pivot's limit as `ε → 0⁺`, `-1`, `0` or `1`, from the first row down. A
    /// shorter list means that the construction stopped early.
    pub signs: Vec<i8>,
    /// The rows of the table, from the first, with their formal entries.
    pub rows: Vec<ExactRow>,
    /// Whether a pivot was zero and was replaced by `ε`.
    pub zero_pivot: bool,
    /// Whether a row was all zero and was replaced by the auxiliary polynomial's derivative.
    pub all_zero_row: bool,
    /// Whether the polynomial has a root at the origin, or roots on the imaginary axis, which the
    /// auxiliary polynomial shows.
    pub boundary: bool,
    /// Whether the construction failed, so that its signs are not a proof.
    pub uncertain: bool,
}

/// One row of an exact table: its power of `s` and its entries written as formal expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactRow {
    /// The power of `s` of the row.
    pub power: usize,
    /// The entries as text, for example `1`, `ε` or `(ε − 2) / ε`.
    pub entries: Vec<String>,
}

/// A quotient of two polynomials in `ε`. Its denominator is never zero.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fraction {
    numerator: RationalPolynomial,
    denominator: RationalPolynomial,
}

/// What the construction has produced so far, so that a failure keeps the rows already built.
#[derive(Debug, Default)]
struct State {
    signs: Vec<i8>,
    rows: Vec<ExactRow>,
    zero_pivot: bool,
    all_zero_row: bool,
}

/// The exact table of `coefficients`, real and descending. It is `None` for a polynomial that the table
/// does not take: fewer than two or more than seventeen coefficients, a coefficient that is not finite, or
/// a zero leading coefficient.
#[must_use]
pub fn exact_routh_signs(coefficients: &[f64]) -> Option<ExactTable> {
    if coefficients.len() < 2
        || coefficients.len() > 17
        || coefficients.iter().any(|value| !value.is_finite())
        || is_exact_zero(coefficients[0])
    {
        return None;
    }
    let mut state = State::default();
    Some(match build(coefficients, &mut state) {
        Ok(boundary) => ExactTable {
            signs: state.signs,
            rows: state.rows,
            zero_pivot: state.zero_pivot,
            all_zero_row: state.all_zero_row,
            boundary,
            uncertain: false,
        },
        Err(_) => ExactTable {
            signs: state.signs,
            rows: state.rows,
            zero_pivot: state.zero_pivot,
            all_zero_row: state.all_zero_row,
            boundary: false,
            uncertain: true,
        },
    })
}

/// Builds the table into `state`, and returns whether the polynomial has a boundary root.
fn build(coefficients: &[f64], state: &mut State) -> Result<bool, ExactError> {
    let degree = coefficients.len() - 1;
    let width = degree / 2 + 1;
    let mut table: Vec<Vec<Fraction>> = Vec::with_capacity(degree + 1);
    for offset in 0..2 {
        let mut row = Vec::with_capacity(width);
        for column in 0..width {
            let value = coefficients
                .get(2 * column + offset)
                .copied()
                .unwrap_or(0.0);
            row.push(Fraction::of(RationalPolynomial::from_decimals(&[value])?));
        }
        table.push(row);
    }
    for row in 0..=degree {
        if row > 0 && table[row].iter().all(|entry| entry.numerator.is_zero()) {
            state.all_zero_row = true;
            let previous = table[row - 1].clone();
            let mut replacement = Vec::with_capacity(width);
            for (column, value) in previous.iter().enumerate() {
                let weight = to_i64(degree + 1) - to_i64(row) - 2 * to_i64(column);
                replacement.push(if weight > 0 {
                    multiply(value, &integer_fraction(weight))?
                } else {
                    Fraction::zero()
                });
            }
            table[row] = replacement;
        }
        if table[row][0].numerator.is_zero() {
            state.zero_pivot = true;
            table[row][0] = epsilon();
        }
        state.signs.push(limit_sign(&table[row][0]));
        state.rows.push(ExactRow {
            power: degree - row,
            entries: table[row].iter().map(format_fraction).collect(),
        });
        if row > 0 && row < degree {
            let prior = table[row - 1].clone();
            let pivot = table[row][0].clone();
            let mut next = Vec::with_capacity(width);
            for column in 0..width {
                if column + 1 == width {
                    next.push(Fraction::zero());
                } else {
                    let left = multiply(&pivot, &prior[column + 1])?;
                    let right = multiply(&prior[0], &table[row][column + 1])?;
                    next.push(divide(&subtract(&left, &right)?, &pivot)?);
                }
            }
            table.push(next);
        }
    }
    imaginary_axis_boundary(coefficients)
}

impl Fraction {
    /// The fraction with numerator `numerator` and denominator one.
    fn of(numerator: RationalPolynomial) -> Self {
        Self {
            numerator,
            denominator: RationalPolynomial::constant(Rational::one()),
        }
    }

    /// The fraction zero.
    fn zero() -> Self {
        Self::of(RationalPolynomial::zero())
    }
}

/// The integer `value` as a fraction.
fn integer_fraction(value: i64) -> Fraction {
    Fraction::of(RationalPolynomial::constant(Rational::integer(value)))
}

/// The formal positive infinitesimal `ε`.
fn epsilon() -> Fraction {
    Fraction::of(
        RationalPolynomial::new(vec![Rational::one(), Rational::zero()])
            .unwrap_or_else(|_| RationalPolynomial::zero()),
    )
}

/// The product of two fractions, reduced.
fn multiply(a: &Fraction, b: &Fraction) -> Result<Fraction, ExactError> {
    reduce(
        &a.numerator.mul(&b.numerator)?,
        &a.denominator.mul(&b.denominator)?,
    )
}

/// The difference of two fractions, reduced.
fn subtract(a: &Fraction, b: &Fraction) -> Result<Fraction, ExactError> {
    reduce(
        &a.numerator
            .mul(&b.denominator)?
            .sub(&b.numerator.mul(&a.denominator)?)?,
        &a.denominator.mul(&b.denominator)?,
    )
}

/// The quotient of two fractions, reduced.
fn divide(a: &Fraction, b: &Fraction) -> Result<Fraction, ExactError> {
    reduce(
        &a.numerator.mul(&b.denominator)?,
        &a.denominator.mul(&b.numerator)?,
    )
}

/// The fraction `numerator / denominator` with the common polynomial factor removed and the denominator
/// made monic, so that equivalent fractions do not accumulate large constants.
fn reduce(
    numerator: &RationalPolynomial,
    denominator: &RationalPolynomial,
) -> Result<Fraction, ExactError> {
    if denominator.is_zero() {
        return Err(ExactError::ZeroFormalDenominator);
    }
    if numerator.is_zero() {
        return Ok(Fraction::zero());
    }
    let common = RationalPolynomial::gcd(numerator, denominator)?;
    let reduced_numerator = numerator
        .exact_divide(&common)?
        .ok_or(ExactError::Divisibility)?;
    let reduced_denominator = denominator
        .exact_divide(&common)?
        .ok_or(ExactError::Divisibility)?;
    let scale = Rational::one().div(&reduced_denominator.coefficients()[0])?;
    Ok(Fraction {
        numerator: scale_polynomial(&reduced_numerator, &scale)?,
        denominator: scale_polynomial(&reduced_denominator, &scale)?,
    })
}

/// The polynomial with each coefficient multiplied by `scale`.
fn scale_polynomial(
    polynomial: &RationalPolynomial,
    scale: &Rational,
) -> Result<RationalPolynomial, ExactError> {
    let coefficients = polynomial
        .coefficients()
        .iter()
        .map(|coefficient| coefficient.mul(scale))
        .collect::<Result<Vec<_>, _>>()?;
    RationalPolynomial::new(coefficients)
}

/// The sign of the limit of a fraction as `ε → 0⁺`: the sign of its lowest-order nonzero terms.
fn limit_sign(value: &Fraction) -> i8 {
    lowest_term_sign(&value.numerator) * lowest_term_sign(&value.denominator)
}

/// The sign of the lowest-order nonzero coefficient of a polynomial, or zero for the zero polynomial.
fn lowest_term_sign(polynomial: &RationalPolynomial) -> i8 {
    polynomial
        .coefficients()
        .iter()
        .rev()
        .find(|coefficient| !coefficient.is_zero())
        .map_or(0, Rational::sign)
}

/// The text of a fraction: a polynomial in `ε`, or a quotient of two of them.
fn format_fraction(value: &Fraction) -> String {
    let numerator = format_polynomial(&value.numerator);
    let denominator_is_one = value.denominator.coefficients().len() == 1
        && value.denominator.coefficients()[0].numerator()
            == value.denominator.coefficients()[0].denominator();
    if denominator_is_one {
        numerator
    } else {
        format!(
            "({numerator}) / ({})",
            format_polynomial(&value.denominator)
        )
    }
}

/// The text of a polynomial in `ε`, with its nonzero terms joined by `+`, or `0`.
fn format_polynomial(polynomial: &RationalPolynomial) -> String {
    let length = polynomial.coefficients().len();
    let terms: Vec<String> = polynomial
        .coefficients()
        .iter()
        .enumerate()
        .filter(|(_, coefficient)| !coefficient.is_zero())
        .map(|(index, coefficient)| {
            let power = length - 1 - index;
            if power > 0 {
                format!("{coefficient}ε^{power}")
            } else {
                coefficient.to_string()
            }
        })
        .collect();
    if terms.is_empty() {
        "0".to_string()
    } else {
        terms.join(" + ")
    }
}

/// Whether the polynomial has a root at the origin, or a root on the imaginary axis. Its even and odd
/// parts `P(s) = E(s²) + s·O(s²)` share a real root `r` exactly when `r` is a root of both; a negative
/// common root `r` gives imaginary roots `±j√(−r)` (N7.3).
fn imaginary_axis_boundary(coefficients: &[f64]) -> Result<bool, ExactError> {
    if coefficients
        .last()
        .is_some_and(|value| is_exact_zero(*value))
    {
        return Ok(true);
    }
    let degree = coefficients.len() - 1;
    let even: Vec<f64> = coefficients
        .iter()
        .enumerate()
        .filter(|(index, _)| (degree - index).is_multiple_of(2))
        .map(|(_, value)| *value)
        .collect();
    let odd: Vec<f64> = coefficients
        .iter()
        .enumerate()
        .filter(|(index, _)| (degree - index) % 2 == 1)
        .map(|(_, value)| *value)
        .collect();
    let common = RationalPolynomial::gcd(
        &RationalPolynomial::from_decimals(&even)?,
        &RationalPolynomial::from_decimals(&odd)?,
    )?;
    if common.coefficients().len() == 1 {
        return Ok(false);
    }
    let roots = isolate_real_roots(&common).map_err(|_| ExactError::AuxiliaryRoots)?;
    let zero = big(0);
    Ok(roots
        .iter()
        .any(|root| *root.exact_upper.numerator() < zero))
}

/// A count as an integer. The counts of a Routh table are small.
fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Whether a value is exactly zero, `+0` or `-0`.
#[allow(clippy::float_cmp)] // An exact zero is the rule of the exact table, not a tolerance.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
#[path = "exact_table_tests.rs"]
mod tests;
