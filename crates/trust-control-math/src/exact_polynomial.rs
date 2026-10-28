//! Exact rational numbers and polynomials with rational coefficients: the arithmetic of the exact
//! Routh tables, the Hurwitz regions and the root isolation (N7.2 and N7.4).
//!
//! A decimal coefficient stands for the rational that its shortest decimal text denotes, so `0.1` is
//! exactly one tenth and not the nearest binary double. Every operation is exact. The integers are
//! arbitrary precision, and a value that would need more than [`MAX_BITS`] bits in its numerator or
//! denominator is refused with [`ExactError::Budget`]; it is never rounded (N7.3, A7).

use std::cmp::Ordering;
use std::fmt;

use num_bigint::BigInt;

pub mod root_isolation;

pub use root_isolation::{isolate_real_roots, sturm_sequence, sturm_variations, RootInterval};

/// The bits a numerator or a denominator may have before exact arithmetic refuses the value.
pub const MAX_BITS: u64 = 32_768;
/// The most coefficients an exact polynomial may have: a degree of 256.
pub const MAX_COEFFICIENTS: usize = 257;

/// Why exact arithmetic refuses a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExactError {
    /// A rational is divided by zero.
    #[error("Exact division by zero.")]
    DivisionByZero,
    /// A polynomial is divided by the zero polynomial.
    #[error("Exact polynomial division by zero.")]
    PolynomialDivisionByZero,
    /// A value needs more bits than the budget allows.
    #[error("Exact arithmetic exceeded its coefficient budget.")]
    Budget,
    /// A polynomial has more coefficients than an exact polynomial may have.
    #[error("Exact polynomial degree exceeds 256.")]
    Degree,
    /// A coefficient is not finite.
    #[error("Polynomial coefficients must be finite.")]
    Nonfinite,
    /// A fraction-free elimination step is not exactly divisible.
    #[error("Fraction-free determinant lost exact divisibility.")]
    Divisibility,
    /// A determinant is requested for a matrix that is not square, of order 1 to 16.
    #[error("Expected a square matrix of order 1–16.")]
    MatrixOrder,
    /// The zero polynomial has infinitely many roots.
    #[error("The zero polynomial has infinitely many roots.")]
    ZeroPolynomial,
    /// The Sturm count is negative, which a correct sequence never gives.
    #[error("Inconsistent Sturm certificate.")]
    SturmCertificate,
    /// Root isolation needs more subdivisions than its budget.
    #[error("Root isolation exceeded its subdivision budget.")]
    IsolationBudget,
    /// A root lies outside the finite range that can be displayed.
    #[error("A root lies outside finite display coordinates.")]
    DisplayRange,
    /// Two isolating enclosures overlap and need further separation.
    #[error("Root enclosures need further separation.")]
    EnclosureOverlap,
    /// The leading coefficient of a Hurwitz polynomial is identically zero.
    #[error("The characteristic leading coefficient is identically zero.")]
    ZeroLeadingCoefficient,
    /// The roots of an auxiliary polynomial need more isolation than the budget allows.
    #[error("Auxiliary roots exceed the exact isolation budget.")]
    AuxiliaryRoots,
    /// A formal denominator of an exact Routh table is zero.
    #[error("Zero formal denominator.")]
    ZeroFormalDenominator,
}

/// An exact rational number in lowest terms, with a positive denominator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rational {
    numerator: BigInt,
    denominator: BigInt,
}

impl Rational {
    /// The rational `numerator / denominator` in lowest terms.
    ///
    /// # Errors
    /// Returns [`ExactError::DivisionByZero`] for a zero denominator and [`ExactError::Budget`] when
    /// the reduced value needs more than [`MAX_BITS`] bits.
    pub fn new(numerator: BigInt, denominator: BigInt) -> Result<Self, ExactError> {
        if denominator == big(0) {
            return Err(ExactError::DivisionByZero);
        }
        let (numerator, denominator) = if denominator < big(0) {
            (-numerator, -denominator)
        } else {
            (numerator, denominator)
        };
        let common = gcd(&numerator, &denominator);
        let value = Self {
            numerator: numerator / common.clone(),
            denominator: denominator / common,
        };
        if value.numerator.bits() > MAX_BITS || value.denominator.bits() > MAX_BITS {
            return Err(ExactError::Budget);
        }
        Ok(value)
    }

    /// The exact rational that the shortest decimal text of `value` denotes, as JavaScript's
    /// `String(value)` writes it: `0.1` is one tenth, `1e-7` is one ten-millionth.
    ///
    /// # Errors
    /// Returns [`ExactError::Nonfinite`] for a value that is not finite, and [`ExactError::Budget`]
    /// when the exact value needs more than [`MAX_BITS`] bits.
    pub fn from_decimal(value: f64) -> Result<Self, ExactError> {
        if !value.is_finite() {
            return Err(ExactError::Nonfinite);
        }
        // Rust writes the shortest digits that read back as `value`, with an exponent.
        let text = format!("{value:e}");
        let (mantissa, exponent) = text.split_once('e').unwrap_or((text.as_str(), "0"));
        let exponent: i64 = exponent.parse().unwrap_or(0);
        let (negative, digits_text) = match mantissa.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, mantissa),
        };
        let fraction_length = digits_text
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let digits: String = digits_text.chars().filter(|c| *c != '.').collect();
        let mut numerator: BigInt = digits.parse().map_err(|_| ExactError::Nonfinite)?;
        if negative {
            numerator = -numerator;
        }
        let fraction_length = i64::try_from(fraction_length).unwrap_or(i64::MAX);
        let power = exponent.saturating_sub(fraction_length);
        if power >= 0 {
            Self::new(
                numerator * pow_big(10, u64::try_from(power).unwrap_or(0))?,
                big(1),
            )
        } else {
            Self::new(numerator, pow_big(10, power.unsigned_abs())?)
        }
    }

    /// The integer `value` as a rational.
    #[must_use]
    pub fn integer(value: i64) -> Self {
        Self {
            numerator: big(value),
            denominator: big(1),
        }
    }

    /// Zero.
    #[must_use]
    pub fn zero() -> Self {
        Self::integer(0)
    }

    /// One.
    #[must_use]
    pub fn one() -> Self {
        Self::integer(1)
    }

    /// The numerator, with the sign of the value.
    #[must_use]
    pub fn numerator(&self) -> &BigInt {
        &self.numerator
    }

    /// The denominator, which is positive.
    #[must_use]
    pub fn denominator(&self) -> &BigInt {
        &self.denominator
    }

    /// Whether the value is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numerator == big(0)
    }

    /// The sign of the value: `-1`, `0` or `1`.
    #[must_use]
    pub fn sign(&self) -> i8 {
        match self.numerator.cmp(&big(0)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    /// `self + other`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when the sum needs more than [`MAX_BITS`] bits.
    pub fn add(&self, other: &Self) -> Result<Self, ExactError> {
        Self::new(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }

    /// `self − other`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when the difference needs more than [`MAX_BITS`] bits.
    pub fn sub(&self, other: &Self) -> Result<Self, ExactError> {
        Self::new(
            &self.numerator * &other.denominator - &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }

    /// `self · other`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when the product needs more than [`MAX_BITS`] bits.
    pub fn mul(&self, other: &Self) -> Result<Self, ExactError> {
        Self::new(
            &self.numerator * &other.numerator,
            &self.denominator * &other.denominator,
        )
    }

    /// `self / other`.
    ///
    /// # Errors
    /// Returns [`ExactError::DivisionByZero`] when `other` is zero and [`ExactError::Budget`] when
    /// the quotient needs more than [`MAX_BITS`] bits.
    pub fn div(&self, other: &Self) -> Result<Self, ExactError> {
        if other.is_zero() {
            return Err(ExactError::DivisionByZero);
        }
        Self::new(
            &self.numerator * &other.denominator,
            &self.denominator * &other.numerator,
        )
    }

    /// `−self`. Negation adds no bits.
    #[must_use]
    pub fn negate(&self) -> Self {
        Self {
            numerator: -self.numerator.clone(),
            denominator: self.denominator.clone(),
        }
    }

    /// The order of two values, decided by exact cross-multiplication.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        (&self.numerator * &other.denominator).cmp(&(&other.numerator * &self.denominator))
    }

    /// The nearest floating-point value of the quotient, as the numerator and the denominator are each
    /// rounded to the nearest double and then divided. A value beyond the range of `f64` is infinite.
    #[must_use]
    pub fn to_f64(&self) -> f64 {
        let numerator = self
            .numerator
            .to_string()
            .parse::<f64>()
            .unwrap_or(f64::NAN);
        let denominator = self
            .denominator
            .to_string()
            .parse::<f64>()
            .unwrap_or(f64::NAN);
        numerator / denominator
    }
}

impl fmt::Display for Rational {
    /// The integer `n`, or the fraction `n/d` in lowest terms.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denominator == big(1) {
            write!(formatter, "{}", self.numerator)
        } else {
            write!(formatter, "{}/{}", self.numerator, self.denominator)
        }
    }
}

/// A polynomial with rational coefficients, listed from the highest power down. It always has a
/// coefficient, and its leading coefficient is zero only when it is the zero polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RationalPolynomial {
    coefficients: Vec<Rational>,
}

impl RationalPolynomial {
    /// The polynomial with these coefficients, leading zeros removed. An empty list is the zero
    /// polynomial.
    ///
    /// # Errors
    /// Returns [`ExactError::Degree`] for more than [`MAX_COEFFICIENTS`] coefficients.
    pub fn new(mut coefficients: Vec<Rational>) -> Result<Self, ExactError> {
        if coefficients.len() > MAX_COEFFICIENTS {
            return Err(ExactError::Degree);
        }
        if coefficients.is_empty() {
            return Ok(Self::zero());
        }
        let first = coefficients
            .iter()
            .position(|value| !value.is_zero())
            .unwrap_or(coefficients.len() - 1);
        coefficients.drain(..first);
        Ok(Self { coefficients })
    }

    /// The polynomial whose coefficients are the exact decimal values of `values` (N7.4).
    ///
    /// # Errors
    /// Returns [`ExactError::Nonfinite`] for a coefficient that is not finite, and the errors of
    /// [`Rational::from_decimal`] and [`RationalPolynomial::new`].
    pub fn from_decimals(values: &[f64]) -> Result<Self, ExactError> {
        let rationals = values
            .iter()
            .map(|value| Rational::from_decimal(*value))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(rationals)
    }

    /// The zero polynomial.
    #[must_use]
    pub fn zero() -> Self {
        Self::constant(Rational::zero())
    }

    /// The constant polynomial `value`.
    #[must_use]
    pub fn constant(value: Rational) -> Self {
        Self {
            coefficients: vec![value],
        }
    }

    /// The coefficients, from the highest power down.
    #[must_use]
    pub fn coefficients(&self) -> &[Rational] {
        &self.coefficients
    }

    /// Whether every coefficient is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.coefficients.iter().all(Rational::is_zero)
    }

    /// `−self`.
    #[must_use]
    pub fn negate(&self) -> Self {
        Self {
            coefficients: self.coefficients.iter().map(Rational::negate).collect(),
        }
    }

    /// `self + other`, each polynomial aligned at its constant term.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when a coefficient needs more than [`MAX_BITS`] bits.
    pub fn add(&self, other: &Self) -> Result<Self, ExactError> {
        let length = self.coefficients.len().max(other.coefficients.len());
        let coefficients = (0..length)
            .map(|index| {
                aligned(&self.coefficients, length, index).add(&aligned(
                    &other.coefficients,
                    length,
                    index,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(coefficients)
    }

    /// `self − other`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when a coefficient needs more than [`MAX_BITS`] bits.
    pub fn sub(&self, other: &Self) -> Result<Self, ExactError> {
        self.add(&other.negate())
    }

    /// `self · other`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when a coefficient needs more than [`MAX_BITS`] bits.
    pub fn mul(&self, other: &Self) -> Result<Self, ExactError> {
        let mut product =
            vec![Rational::zero(); self.coefficients.len() + other.coefficients.len() - 1];
        for (i, left) in self.coefficients.iter().enumerate() {
            for (j, right) in other.coefficients.iter().enumerate() {
                product[i + j] = product[i + j].add(&left.mul(right)?)?;
            }
        }
        Self::new(product)
    }

    /// The quotient and the remainder of the long division `self = quotient · divisor + remainder`.
    ///
    /// # Errors
    /// Returns [`ExactError::PolynomialDivisionByZero`] for a zero divisor.
    pub fn divide(&self, divisor: &Self) -> Result<(Self, Self), ExactError> {
        if divisor.is_zero() {
            return Err(ExactError::PolynomialDivisionByZero);
        }
        let quotient_length = (self.coefficients.len() + 1)
            .saturating_sub(divisor.coefficients.len())
            .max(1);
        let mut quotient = vec![Rational::zero(); quotient_length];
        let mut remainder = self.clone();
        while !remainder.is_zero() && remainder.coefficients.len() >= divisor.coefficients.len() {
            let power = remainder.coefficients.len() - divisor.coefficients.len();
            let factor = remainder.coefficients[0].div(&divisor.coefficients[0])?;
            quotient[quotient_length - 1 - power] = factor.clone();
            let mut next = Vec::with_capacity(remainder.coefficients.len());
            for (index, value) in remainder.coefficients.iter().enumerate() {
                let scaled = match divisor.coefficients.get(index) {
                    Some(coefficient) => factor.mul(coefficient)?,
                    None => Rational::zero(),
                };
                next.push(value.sub(&scaled)?);
            }
            remainder = Self::new(next.into_iter().skip(1).collect())?;
        }
        Ok((Self::new(quotient)?, remainder))
    }

    /// The exact quotient when `divisor` divides `self` with no remainder, and `None` otherwise.
    ///
    /// # Errors
    /// Returns the errors of [`RationalPolynomial::divide`].
    pub fn exact_divide(&self, divisor: &Self) -> Result<Option<Self>, ExactError> {
        let (quotient, remainder) = self.divide(divisor)?;
        Ok(remainder.is_zero().then_some(quotient))
    }

    /// The derivative `p′(s)`.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when a coefficient needs more than [`MAX_BITS`] bits.
    pub fn derivative(&self) -> Result<Self, ExactError> {
        let degree = self.coefficients.len() - 1;
        let coefficients = self
            .coefficients
            .iter()
            .take(degree)
            .enumerate()
            .map(|(index, coefficient)| {
                coefficient.mul(&Rational::integer(
                    i64::try_from(degree - index).unwrap_or(0),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(coefficients)
    }

    /// The value `p(x)`, by Horner's rule.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when an intermediate value needs more than [`MAX_BITS`] bits.
    pub fn evaluate(&self, point: &Rational) -> Result<Rational, ExactError> {
        let mut value = Rational::zero();
        for coefficient in &self.coefficients {
            value = value.mul(point)?.add(coefficient)?;
        }
        Ok(value)
    }

    /// The polynomial scaled by a positive rational to integer coefficients with no common factor.
    /// A positive scaling keeps the signs of a Sturm sequence.
    ///
    /// # Errors
    /// Returns [`ExactError::Budget`] when a coefficient needs more than [`MAX_BITS`] bits.
    pub fn primitive(&self) -> Result<Self, ExactError> {
        let mut denominator = big(1);
        for coefficient in &self.coefficients {
            let common = gcd(&denominator, &coefficient.denominator);
            denominator = denominator / common * &coefficient.denominator;
        }
        let values: Vec<BigInt> = self
            .coefficients
            .iter()
            .map(|coefficient| &coefficient.numerator * (&denominator / &coefficient.denominator))
            .collect();
        let mut common = big(0);
        for value in &values {
            common = gcd(&common, value);
        }
        if common == big(0) {
            common = big(1);
        }
        let coefficients = values
            .into_iter()
            .map(|value| Rational::new(value / common.clone(), big(1)))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(coefficients)
    }

    /// The greatest common divisor of two polynomials, as a primitive polynomial with a positive leading coefficient, so that a
    /// quotient by it keeps the dividend's sign.
    ///
    /// # Errors
    /// Returns the errors of [`RationalPolynomial::divide`] and [`RationalPolynomial::primitive`].
    pub fn gcd(a: &Self, b: &Self) -> Result<Self, ExactError> {
        let mut left = a.clone();
        let mut right = b.clone();
        while !right.is_zero() {
            let remainder = left.divide(&right)?.1.primitive()?;
            left = right;
            right = remainder;
        }
        let common = left.primitive()?;
        if common
            .coefficients
            .first()
            .is_some_and(|leading| leading.sign() < 0)
        {
            Ok(common.negate())
        } else {
            Ok(common)
        }
    }

    /// The square-free part `p / gcd(p, p′)`, as a primitive polynomial. A constant is returned
    /// unchanged.
    ///
    /// # Errors
    /// Returns [`ExactError::Divisibility`] when the gcd does not divide the polynomial exactly.
    pub fn square_free(&self) -> Result<Self, ExactError> {
        if self.coefficients.len() <= 1 {
            return Ok(self.clone());
        }
        let common = Self::gcd(self, &self.derivative()?)?;
        self.exact_divide(&common)?
            .ok_or(ExactError::Divisibility)?
            .primitive()
    }
}

/// The determinant of a square matrix of polynomials, by fraction-free (Bareiss) elimination with an
/// exact division at each step. The Hurwitz determinants of N7.4 are its users.
///
/// # Errors
/// Returns [`ExactError::MatrixOrder`] for a matrix that is not square of order 1 to 16,
/// [`ExactError::Divisibility`] when an elimination step is not exact, and the errors of the
/// polynomial operations.
pub fn determinant(matrix: &[Vec<RationalPolynomial>]) -> Result<RationalPolynomial, ExactError> {
    let order = matrix.len();
    if order == 0 || order > 16 || matrix.iter().any(|row| row.len() != order) {
        return Err(ExactError::MatrixOrder);
    }
    let mut rows = matrix.to_vec();
    let mut previous = RationalPolynomial::constant(Rational::one());
    let mut negative = false;
    for k in 0..order - 1 {
        let Some(pivot_row) = (k..order).find(|&row| !rows[row][k].is_zero()) else {
            return Ok(RationalPolynomial::zero());
        };
        if pivot_row != k {
            rows.swap(k, pivot_row);
            negative = !negative;
        }
        let pivot = rows[k][k].clone();
        for i in k + 1..order {
            for j in k + 1..order {
                let numerator = pivot.mul(&rows[i][j])?.sub(&rows[i][k].mul(&rows[k][j])?)?;
                rows[i][j] = numerator
                    .exact_divide(&previous)?
                    .ok_or(ExactError::Divisibility)?;
            }
        }
        for row in rows.iter_mut().skip(k + 1) {
            row[k] = RationalPolynomial::zero();
        }
        previous = pivot;
    }
    let last = rows[order - 1][order - 1].clone();
    Ok(if negative { last.negate() } else { last })
}

/// The coefficient at `index` of a polynomial padded at its front with zeros to `length` coefficients.
fn aligned(coefficients: &[Rational], length: usize, index: usize) -> Rational {
    let padding = length - coefficients.len();
    if index < padding {
        Rational::zero()
    } else {
        coefficients[index - padding].clone()
    }
}

/// An integer from a small constant.
pub(crate) fn big(value: i64) -> BigInt {
    BigInt::from(value)
}

/// `base` raised to `exponent`, refused when the result would exceed the bit budget.
fn pow_big(base: i64, exponent: u64) -> Result<BigInt, ExactError> {
    // A power of ten has about 3.3 bits per exponent step, so a larger exponent is over budget.
    if exponent > MAX_BITS {
        return Err(ExactError::Budget);
    }
    let mut result = big(1);
    for _ in 0..exponent {
        result *= big(base);
    }
    Ok(result)
}

/// The greatest common divisor of the magnitudes of two integers, which is zero only for two zeros.
pub(crate) fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let mut left = absolute(a);
    let mut right = absolute(b);
    while right != big(0) {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left
}

/// The magnitude of an integer.
pub(crate) fn absolute(value: &BigInt) -> BigInt {
    if *value < big(0) {
        -value.clone()
    } else {
        value.clone()
    }
}

#[cfg(test)]
#[path = "exact_polynomial_tests.rs"]
mod tests;
