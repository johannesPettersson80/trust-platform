//! The root locus of a transfer function over a set of gains (N7.5): for each gain `k` the roots of
//! `D(s) + k·N(s)`. Only the gain changes; every other parameter is already inside the polynomials.

use crate::classical::{normalized_polynomial, roots_of_real_polynomial, ClassicalError, Transfer};
use crate::linalg::ComplexValue;
use crate::polynomial::add_aligned;

/// The closed-loop poles at one gain.
#[derive(Debug, Clone, PartialEq)]
pub struct GainBranch {
    /// The gain `k`.
    pub gain: f64,
    /// The roots of `D(s) + k·N(s)`.
    pub poles: Vec<ComplexValue>,
}

/// The poles of `D(s) + k·N(s)` for each gain in `gains`. The numerator may be zero, which gives the
/// open loop as the starting point of a sweep. The delay must be zero, because a delay has no
/// polynomial characteristic equation (N7.5).
///
/// # Errors
/// Returns [`ClassicalError::InvalidPolynomial`] for a negative or nonfinite delay, a zero denominator, an
/// empty or nonfinite numerator, [`ClassicalError::InvalidInput`] for a nonzero delay, an empty list of
/// gains, or a gain that is negative or not finite, and [`ClassicalError::SolverFailed`] when the roots of
/// a characteristic polynomial cannot be computed.
pub fn root_locus(transfer: &Transfer, gains: &[f64]) -> Result<Vec<GainBranch>, ClassicalError> {
    if !transfer.delay_seconds.is_finite()
        || transfer.delay_seconds < 0.0
        || normalized_polynomial(&transfer.denominator).is_none()
        || transfer.numerator.is_empty()
        || !transfer.numerator.iter().all(|value| value.is_finite())
    {
        return Err(ClassicalError::InvalidPolynomial);
    }
    // The delay is nonnegative here, so a nonzero delay is a positive one.
    if transfer.delay_seconds > 0.0
        || gains.is_empty()
        || !gains.iter().all(|gain| gain.is_finite() && *gain >= 0.0)
    {
        return Err(ClassicalError::InvalidInput);
    }
    gains
        .iter()
        .map(|&gain| -> Result<GainBranch, ClassicalError> {
            let scaled: Vec<f64> = transfer
                .numerator
                .iter()
                .map(|value| value * gain)
                .collect();
            let characteristic = add_aligned(&transfer.denominator, &scaled);
            let poles =
                roots_of_real_polynomial(&characteristic).ok_or(ClassicalError::SolverFailed)?;
            Ok(GainBranch { gain, poles })
        })
        .collect()
}

#[cfg(test)]
#[path = "root_locus_tests.rs"]
mod tests;
