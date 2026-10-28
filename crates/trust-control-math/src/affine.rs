//! Affine functions of one real parameter `p` and the polynomials whose coefficients they are (N7.4).
//! The characteristic polynomial `Dp·Dc + Np·Nc` of a declared loop, with one parameter that varies,
//! has coefficients of the form `c + s·p`.

/// The relative size below which a parameter slope counts as ill-conditioned (N7.4). It is the
/// tolerance of the float classification; the exact determinants of [`crate::routh::regions`] need none.
pub const RELATIVE_TOLERANCE: f64 = 1e-12;

/// An affine function `constant + slope · p` of the parameter `p`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    /// The value at `p = 0`.
    pub constant: f64,
    /// The change of the value per unit of `p`.
    pub slope: f64,
}

impl Affine {
    /// The constant function `value`.
    #[must_use]
    pub const fn constant(value: f64) -> Self {
        Self {
            constant: value,
            slope: 0.0,
        }
    }

    /// The function `scale · p`.
    #[must_use]
    pub const fn parameter(scale: f64) -> Self {
        Self {
            constant: 0.0,
            slope: scale,
        }
    }
}

/// The solution of `constant + slope · p > 0` over the real parameter axis (N7.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositiveSolution {
    /// Every real parameter satisfies the inequality: its slope is zero and its constant is positive.
    All,
    /// No real parameter satisfies it: its slope is zero and its constant is not positive.
    Empty,
    /// The inequality holds for `p > bound`, because the slope is positive.
    Above(f64),
    /// The inequality holds for `p < bound`, because the slope is negative.
    Below(f64),
    /// The solution is not finite, or the slope is too small against the constant to classify it.
    Uncertain,
}

/// Solves `constant + slope · p > 0` over the real parameter axis (N7.4).
///
/// The tolerance of a slope is [`RELATIVE_TOLERANCE`] times the largest of 1, `|constant|` and `|slope|`.
#[must_use]
pub fn solve_positive(value: Affine) -> PositiveSolution {
    if !value.constant.is_finite() || !value.slope.is_finite() {
        return PositiveSolution::Uncertain;
    }
    if is_exact_zero(value.slope) {
        return if value.constant > 0.0 {
            PositiveSolution::All
        } else {
            PositiveSolution::Empty
        };
    }
    let tolerance = value.constant.abs().max(value.slope.abs()).max(1.0) * RELATIVE_TOLERANCE;
    if value.slope.abs() <= tolerance {
        return PositiveSolution::Uncertain;
    }
    let bound = -value.constant / value.slope;
    if !bound.is_finite() {
        PositiveSolution::Uncertain
    } else if value.slope > 0.0 {
        PositiveSolution::Above(bound)
    } else {
        PositiveSolution::Below(bound)
    }
}

/// The product of a polynomial with real coefficients and a polynomial with affine coefficients. Both
/// are listed from the highest power down. The result stays affine, because one factor is a number.
/// An empty factor gives an empty product.
#[must_use]
pub fn convolve_numeric(numeric: &[f64], affine: &[Affine]) -> Vec<Affine> {
    if numeric.is_empty() || affine.is_empty() {
        return Vec::new();
    }
    let mut product = vec![Affine::constant(0.0); numeric.len() + affine.len() - 1];
    for (i, left) in numeric.iter().enumerate() {
        for (j, right) in affine.iter().enumerate() {
            product[i + j].constant += left * right.constant;
            product[i + j].slope += left * right.slope;
        }
    }
    product
}

/// The sum of two polynomials with affine coefficients, each aligned at its constant term: the shorter
/// one is padded with leading zeros.
#[must_use]
pub fn sum(left: &[Affine], right: &[Affine]) -> Vec<Affine> {
    let length = left.len().max(right.len());
    (0..length)
        .map(|index| {
            let a = aligned(left, length, index);
            let b = aligned(right, length, index);
            Affine {
                constant: a.constant + b.constant,
                slope: a.slope + b.slope,
            }
        })
        .collect()
}

/// The coefficient at `index` of an affine polynomial padded at its front with zeros to `length`.
fn aligned(values: &[Affine], length: usize, index: usize) -> Affine {
    let padding = length - values.len();
    if index < padding {
        Affine::constant(0.0)
    } else {
        values[index - padding]
    }
}

/// Whether a value is exactly zero, `+0` or `-0`. A slope is classified as zero only when it is.
#[allow(clippy::float_cmp)] // An exact zero slope has no parameter dependence; a tolerance is applied elsewhere.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
#[path = "affine_tests.rs"]
mod tests;
