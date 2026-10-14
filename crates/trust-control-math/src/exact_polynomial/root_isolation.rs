//! Sturm sequences and the isolation of the real roots of an exact polynomial (N7.4 and the exact
//! Routh regions). Each enclosure is a rational interval that holds exactly one distinct real root.
//! A root that is itself rational is enclosed by an interval of zero width.

use std::cmp::Ordering;

use super::{absolute, big, pow_big, ExactError, Rational, RationalPolynomial};

/// The most bisection nodes of one isolation before it is refused.
const NODE_LIMIT: usize = 50_000;
/// The deepest bisection of one isolation before it is refused.
const DEPTH_LIMIT: usize = 256;
/// The power of two below which an enclosure of one root counts as isolated: its width is `2^-48`.
const WIDTH_EXPONENT: u64 = 48;

/// One isolating enclosure of a real root: its exact endpoints and their nearest floating-point values.
#[derive(Debug, Clone, PartialEq)]
pub struct RootInterval {
    /// The lower endpoint, as a floating-point value.
    pub lower: f64,
    /// The upper endpoint, as a floating-point value.
    pub upper: f64,
    /// The exact lower endpoint.
    pub exact_lower: Rational,
    /// The exact upper endpoint.
    pub exact_upper: Rational,
}

/// The Sturm sequence of `input`, taken from its square-free primitive part (N7.4): the polynomial,
/// its derivative, then the negated remainders of successive divisions, each made primitive.
///
/// # Errors
/// Returns the errors of the polynomial operations.
pub fn sturm_sequence(input: &RationalPolynomial) -> Result<Vec<RationalPolynomial>, ExactError> {
    let polynomial = input.primitive()?.square_free()?;
    if polynomial.coefficients().len() <= 1 {
        return Ok(vec![polynomial]);
    }
    let derivative = polynomial.derivative()?.primitive()?;
    let mut sequence = vec![polynomial, derivative];
    while sequence[sequence.len() - 1].coefficients().len() > 1 {
        let n = sequence.len();
        let (_, remainder) = sequence[n - 2].divide(&sequence[n - 1])?;
        let next = remainder.negate().primitive()?;
        if next.is_zero() {
            break;
        }
        sequence.push(next);
    }
    Ok(sequence)
}

/// The number of sign changes of a Sturm sequence at `point`, with the zero values left out.
///
/// # Errors
/// Returns [`ExactError::Budget`] when an evaluation needs more than the bit budget.
pub fn sturm_variations(
    sequence: &[RationalPolynomial],
    point: &Rational,
) -> Result<i64, ExactError> {
    let mut signs = Vec::with_capacity(sequence.len());
    for polynomial in sequence {
        let sign = polynomial.evaluate(point)?.sign();
        if sign != 0 {
            signs.push(sign);
        }
    }
    let changes = signs.windows(2).filter(|pair| pair[0] != pair[1]).count();
    Ok(i64::try_from(changes).unwrap_or(i64::MAX))
}

/// The isolating enclosures of the distinct real roots of `input`, from the lowest, found by Sturm
/// counts and bisection (N7.4). Each enclosure holds exactly one distinct root: a repeated root is
/// isolated once, and a root at a bisection point is enclosed by an interval of zero width.
///
/// # Errors
/// Returns [`ExactError::ZeroPolynomial`] for the zero polynomial, [`ExactError::IsolationBudget`]
/// when the bisection exceeds its budget, [`ExactError::DisplayRange`] for a root outside the finite
/// floating-point range, [`ExactError::SturmCertificate`] for an inconsistent count, and the errors of
/// the polynomial operations.
pub fn isolate_real_roots(input: &RationalPolynomial) -> Result<Vec<RootInterval>, ExactError> {
    if input.is_zero() {
        return Err(ExactError::ZeroPolynomial);
    }
    if input.coefficients().len() == 1 {
        return Ok(Vec::new());
    }
    let sequence = sturm_sequence(input)?;
    let leading = sequence[0].coefficients()[0].clone();
    // Every root lies inside |x| ≤ bound, the Cauchy bound of the monic polynomial with a margin.
    let mut bound = big(2);
    for coefficient in &sequence[0].coefficients()[1..] {
        let ratio = coefficient.div(&leading)?;
        let ceiling = (absolute(ratio.numerator()) + ratio.denominator() - big(1))
            / ratio.denominator().clone()
            + big(2);
        if ceiling > bound {
            bound = ceiling;
        }
    }
    let mut search = Search {
        polynomial: &sequence[0],
        sequence: &sequence,
        width_limit: Rational::new(big(1), pow_big(2, WIDTH_EXPONENT)?)?,
        nodes: 0,
        intervals: Vec::new(),
    };
    let left = Rational::new(-bound.clone(), big(1))?;
    let right = Rational::new(bound, big(1))?;
    let left_count = sturm_variations(&sequence, &left)?;
    let right_count = sturm_variations(&sequence, &right)?;
    search.isolate(&left, &right, left_count, right_count, 0)?;
    Ok(search.intervals)
}

/// The state of one isolation: the polynomial, its Sturm sequence and the intervals found so far.
struct Search<'a> {
    polynomial: &'a RationalPolynomial,
    sequence: &'a [RationalPolynomial],
    width_limit: Rational,
    nodes: usize,
    intervals: Vec<RootInterval>,
}

impl Search<'_> {
    /// Isolates the roots in `(a, b]`, where `va` and `vb` are the Sturm counts at `a` and `b`.
    fn isolate(
        &mut self,
        a: &Rational,
        b: &Rational,
        va: i64,
        vb: i64,
        depth: usize,
    ) -> Result<(), ExactError> {
        self.nodes += 1;
        if self.nodes > NODE_LIMIT || depth > DEPTH_LIMIT {
            return Err(ExactError::IsolationBudget);
        }
        let right_is_root = i64::from(self.polynomial.evaluate(b)?.is_zero());
        let count = va - vb - right_is_root;
        if count == 0 {
            return Ok(());
        }
        if count < 0 {
            return Err(ExactError::SturmCertificate);
        }
        let narrow = b.sub(a)?.compare(&self.width_limit) != Ordering::Greater;
        if count == 1 && narrow && right_is_root == 0 && !self.polynomial.evaluate(a)?.is_zero() {
            return self.emit(a, b);
        }
        let middle = a.add(b)?.div(&Rational::integer(2))?;
        let middle_count = sturm_variations(self.sequence, &middle)?;
        self.isolate(a, &middle, va, middle_count, depth + 1)?;
        if self.polynomial.evaluate(&middle)?.is_zero() {
            self.emit(&middle, &middle)?;
        }
        self.isolate(&middle, b, middle_count, vb, depth + 1)
    }

    /// Records the enclosure `[a, b]`, refused when an endpoint has no finite floating-point value.
    fn emit(&mut self, a: &Rational, b: &Rational) -> Result<(), ExactError> {
        let lower = a.to_f64();
        let upper = b.to_f64();
        if !lower.is_finite() || !upper.is_finite() {
            return Err(ExactError::DisplayRange);
        }
        self.intervals.push(RootInterval {
            lower,
            upper,
            exact_lower: a.clone(),
            exact_upper: b.clone(),
        });
        Ok(())
    }
}

#[cfg(test)]
#[path = "root_isolation_tests.rs"]
mod tests;
