//! The parameter regions in which a polynomial with affine coefficients is Hurwitz stable (N7.4).
//!
//! The strict Hurwitz inequalities are the leading principal minors of the Hurwitz matrix: each
//! `Δk(p)`, multiplied by the leading coefficient for odd `k`, must be positive. Their exact polynomials
//! in `p` change sign only at their real roots. The roots of their product are isolated exactly, and
//! every interval between two consecutive roots keeps the sign of each inequality, so it is tested at a
//! single rational probe. The result is a union of intervals whose ends are exact algebraic boundaries.

use std::cmp::Ordering;

use crate::affine::Affine;
use crate::exact_polynomial::{
    determinant, isolate_real_roots, ExactError, Rational, RationalPolynomial, RootInterval,
};

/// The shape of a stability region on the parameter axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// No parameter value is stable.
    Empty,
    /// Every real parameter value is stable.
    AllReal,
    /// One bounded or half-bounded interval is stable.
    Open,
    /// Several disjoint intervals are stable.
    Union,
}

/// One stable interval of the parameter. A missing end is unbounded.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionInterval {
    /// The lower end, as a floating-point value.
    pub lower: Option<f64>,
    /// The upper end, as a floating-point value.
    pub upper: Option<f64>,
    /// The index in [`HurwitzRegion::Regions::boundaries`] of the root at the lower end.
    pub lower_boundary: Option<usize>,
    /// The index in [`HurwitzRegion::Regions::boundaries`] of the root at the upper end.
    pub upper_boundary: Option<usize>,
}

/// An exact boundary of a region: one real root of the boundary polynomial, with its enclosure.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionBoundary {
    /// The coefficients of the boundary polynomial, from the highest power down.
    pub polynomial: Vec<Rational>,
    /// The exact lower end of the enclosure of the root.
    pub lower: Rational,
    /// The exact upper end of the enclosure of the root.
    pub upper: Rational,
}

/// The stability region of a polynomial with affine coefficients.
#[derive(Debug, Clone, PartialEq)]
pub enum HurwitzRegion {
    /// A strict determinant inequality is identically zero in `p`, so it holds nowhere.
    Degenerate,
    /// The stable parameter values, with their exact boundaries.
    Regions {
        /// The shape of the region.
        kind: RegionKind,
        /// The stable intervals, in increasing order.
        intervals: Vec<RegionInterval>,
        /// The exact boundary roots that the intervals end at.
        boundaries: Vec<RegionBoundary>,
    },
}

/// The stability region of the polynomial whose coefficients are `coefficients`, from the highest power
/// down, with the first coefficient the leading one. Its degree is at least one.
///
/// # Errors
/// Returns [`ExactError::Nonfinite`] for a coefficient that is not finite,
/// [`ExactError::ZeroLeadingCoefficient`] when the leading coefficient is identically zero, and the errors
/// of the exact determinants and root isolation, among them [`ExactError::EnclosureOverlap`] when two root
/// enclosures overlap and cannot be separated.
pub fn hurwitz_region(coefficients: &[Affine]) -> Result<HurwitzRegion, ExactError> {
    let polynomials = coefficients
        .iter()
        .map(|value| RationalPolynomial::from_decimals(&[value.slope, value.constant]))
        .collect::<Result<Vec<_>, _>>()?;
    let leading = polynomials
        .first()
        .ok_or(ExactError::ZeroLeadingCoefficient)?
        .clone();
    if leading.is_zero() {
        return Err(ExactError::ZeroLeadingCoefficient);
    }
    let degree = coefficients.len() - 1;
    let mut constraints = Vec::with_capacity(degree);
    for size in 1..=degree {
        let matrix: Vec<Vec<RationalPolynomial>> = (0..size)
            .map(|row| {
                (0..size)
                    .map(|column| hurwitz_entry(&polynomials, row, column))
                    .collect()
            })
            .collect();
        let minor = determinant(&matrix)?;
        // The sign of `Δk / a0^k` is the sign of `Δk · a0` for odd `k`, and of `Δk` for even `k`, because
        // `a0^(2i)` is positive off its roots. Only odd minors take the leading coefficient.
        let constraint = if size % 2 == 1 {
            minor.mul(&leading)?
        } else {
            minor
        };
        constraints.push(constraint.primitive()?);
    }
    if constraints.iter().any(RationalPolynomial::is_zero) {
        return Ok(HurwitzRegion::Degenerate);
    }
    let mut boundary = RationalPolynomial::constant(Rational::one());
    for constraint in &constraints {
        boundary = boundary.mul(&constraint.square_free()?)?.square_free()?;
    }
    let roots = isolate_real_roots(&boundary)?;
    let boundaries = roots
        .iter()
        .map(|root| RegionBoundary {
            polynomial: boundary.coefficients().to_vec(),
            lower: root.exact_lower.clone(),
            upper: root.exact_upper.clone(),
        })
        .collect();
    let mut intervals = Vec::new();
    for index in 0..=roots.len() {
        let left = index.checked_sub(1).map(|previous| &roots[previous]);
        let right = roots.get(index);
        if let (Some(left), Some(right)) = (left, right) {
            if left.exact_upper.compare(&right.exact_lower) != Ordering::Less {
                return Err(ExactError::EnclosureOverlap);
            }
        }
        let probe = probe_between(left, right)?;
        let mut stable = true;
        for constraint in &constraints {
            if constraint.evaluate(&probe)?.sign() <= 0 {
                stable = false;
                break;
            }
        }
        if stable {
            intervals.push(RegionInterval {
                lower: left.map(display_root).transpose()?,
                upper: right.map(display_root).transpose()?,
                lower_boundary: left.map(|_| index - 1),
                upper_boundary: right.map(|_| index),
            });
        }
    }
    let kind = match intervals.as_slice() {
        [] => RegionKind::Empty,
        [only] if only.lower.is_none() && only.upper.is_none() => RegionKind::AllReal,
        [_] => RegionKind::Open,
        _ => RegionKind::Union,
    };
    Ok(HurwitzRegion::Regions {
        kind,
        intervals,
        boundaries,
    })
}

/// The entry of the Hurwitz matrix at `row` and `column`: the coefficient `c[2·column − row + 1]` of the
/// polynomial, or zero when that index lies outside the coefficients (N7.4).
fn hurwitz_entry(
    polynomials: &[RationalPolynomial],
    row: usize,
    column: usize,
) -> RationalPolynomial {
    (2 * column + 1)
        .checked_sub(row)
        .and_then(|index| polynomials.get(index).cloned())
        .unwrap_or_else(RationalPolynomial::zero)
}

/// A rational value strictly between two consecutive roots, or beyond the outermost ones.
fn probe_between(
    left: Option<&RootInterval>,
    right: Option<&RootInterval>,
) -> Result<Rational, ExactError> {
    match (left, right) {
        (Some(left), Some(right)) => left
            .exact_upper
            .add(&right.exact_lower)?
            .div(&Rational::integer(2)),
        (Some(left), None) => left.exact_upper.add(&Rational::one()),
        (None, Some(right)) => right.exact_lower.sub(&Rational::one()),
        (None, None) => Ok(Rational::zero()),
    }
}

/// The floating-point value at the middle of a root's enclosure.
fn display_root(root: &RootInterval) -> Result<f64, ExactError> {
    Ok(root
        .exact_lower
        .add(&root.exact_upper)?
        .div(&Rational::integer(2))?
        .to_f64())
}

#[cfg(test)]
#[path = "regions_tests.rs"]
mod tests;
