//! The classical frequency calculations of a transfer function with a pure delay (N4.3, N7.7 and N7.8):
//! the Bode data, the bounded first-crossover margin estimate, and the roots of a real polynomial.
//!
//! A transfer function is `N(s)/D(s)·e^(−sL)` with real coefficients listed from the highest power down
//! and a delay `L` in seconds. The frequency points of a sweep are logarithmic: `count = ⌈decades · points⌉ + 1`
//! points from the minimum to the maximum, the last one at the maximum. A crossing is found between two
//! points, and interpolated on the logarithm of the frequency.

use std::f64::consts::PI;

use thiserror::Error;

use crate::linalg::ComplexValue;
use crate::polynomial;

/// The smallest number of points per decade a sweep may have.
const MINIMUM_POINTS_PER_DECADE: u32 = 10;

/// A transfer function with real coefficients, listed from the highest power down, and a pure delay.
#[derive(Debug, Clone, PartialEq)]
pub struct Transfer {
    /// The numerator coefficients.
    pub numerator: Vec<f64>,
    /// The denominator coefficients.
    pub denominator: Vec<f64>,
    /// The pure delay `L` in seconds, zero or more.
    pub delay_seconds: f64,
}

/// A logarithmic frequency sweep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sweep {
    /// The lowest frequency in rad/s; positive.
    pub minimum_rad_per_second: f64,
    /// The highest frequency in rad/s; above the minimum.
    pub maximum_rad_per_second: f64,
    /// The points per decade; at least ten.
    pub points_per_decade: u32,
}

/// Why a classical calculation is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ClassicalError {
    /// A frequency, a sweep bound or a delay is not finite or not in range.
    #[error("nonfinite input")]
    InvalidInput,
    /// A coefficient is not finite, the denominator is zero, or the delay is negative.
    #[error("invalid polynomial")]
    InvalidPolynomial,
    /// The roots of a polynomial could not be computed.
    #[error("solver failed")]
    SolverFailed,
    /// The response at a frequency is singular or not finite (N4.3).
    #[error("nonfinite response")]
    NonfiniteResponse,
    /// A pole lies on the imaginary axis, so the Nyquist contour passes through it.
    #[error("pole on contour")]
    PoleOnContour,
    /// The Nyquist curve does not close near zero within the sweep.
    #[error("insufficient contour closure")]
    InsufficientContourClosure,
    /// The Nyquist curve passes within a small distance of the point −1.
    #[error("contour near critical point")]
    ContourNearCriticalPoint,
}

/// The data of a transfer function on a frequency grid.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferAnalysis {
    /// The poles: the roots of the denominator.
    pub poles: Vec<ComplexValue>,
    /// The zeros: the roots of the numerator, empty for a constant numerator.
    pub zeros: Vec<ComplexValue>,
    /// The response at each frequency of the grid.
    pub bode: Vec<BodePoint>,
}

/// The response of a transfer function at one frequency (N4.3, N7.1).
#[derive(Debug, Clone, PartialEq)]
pub struct BodePoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The magnitude of `G(jω)` in dB: `20·log10|G|`.
    pub magnitude_db: f64,
    /// The phase of `G(jω)` in degrees, in `(−180, 180]`.
    pub phase_degrees: f64,
    /// The sensitivity `S = 1 / (1 + G)`.
    pub sensitivity: ComplexValue,
    /// The complementary sensitivity `T = G·S`.
    pub complementary_sensitivity: ComplexValue,
    /// The distance of `S + T` from one, which is zero for an exact calculation.
    pub sensitivity_complementarity_residual: f64,
}

/// The gain margin of a loop: unbounded when its phase never crosses −180° on the grid (N7.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GainMargin {
    /// No phase crossover was found in the sweep.
    Unbounded,
    /// The gain margin in dB, `−20·log10|L(jω180)|`.
    Decibels(f64),
}

/// The margins of a loop estimated on a sweep: its first gain crossover (N7.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    /// The first gain crossover `|L| = 1` on the grid, in rad/s.
    pub gain_crossover_rad_per_second: f64,
    /// The first phase crossover `∠L = −180°` on the grid, in rad/s, when there is one.
    pub phase_crossover_rad_per_second: Option<f64>,
    /// The gain margin.
    pub gain_margin: GainMargin,
    /// The phase margin in degrees, `180° + ∠L` at the gain crossover.
    pub phase_margin_degrees: f64,
    /// The delay margin in seconds, `PM (rad) / ωc`, or zero when the phase margin is not positive.
    pub delay_margin_seconds: f64,
}

/// The logarithmic frequency points of a sweep, from the minimum to the maximum (N4.3).
///
/// # Errors
/// Returns [`ClassicalError::InvalidInput`] for a minimum that is not positive, a maximum that is not
/// finite and above the minimum, or fewer than ten points per decade.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The count is a positive whole number.
pub fn logarithmic_frequencies(sweep: Sweep) -> Result<Vec<f64>, ClassicalError> {
    let minimum = sweep.minimum_rad_per_second;
    let maximum = sweep.maximum_rad_per_second;
    if !minimum.is_finite()
        || minimum <= 0.0
        || !maximum.is_finite()
        || maximum <= minimum
        || sweep.points_per_decade < MINIMUM_POINTS_PER_DECADE
    {
        return Err(ClassicalError::InvalidInput);
    }
    let ratio = maximum / minimum;
    let intervals = (ratio.log10() * f64::from(sweep.points_per_decade)).ceil() as usize;
    let count = intervals + 1;
    Ok((0..count)
        .map(|index| minimum * ratio.powf(index_fraction(index, intervals)))
        .collect())
}

/// The response `G(jω)` of a transfer function at `frequency`, with the factor `e^(−jωL)` of its delay.
/// A zero denominator gives a value that is not finite; the callers refuse it.
#[must_use]
pub fn transfer_value(transfer: &Transfer, frequency: f64) -> ComplexValue {
    let s = ComplexValue {
        real: 0.0,
        imaginary: frequency,
    };
    let mut value = complex_divide(
        evaluate(&transfer.numerator, s),
        evaluate(&transfer.denominator, s),
    );
    if transfer.delay_seconds > 0.0 {
        let angle = -frequency * transfer.delay_seconds;
        value = complex_multiply(
            value,
            ComplexValue {
                real: angle.cos(),
                imaginary: angle.sin(),
            },
        );
    }
    value
}

/// The data of a transfer function on a grid of positive frequencies: its poles, its zeros, and its
/// response, sensitivity and complementary sensitivity at each frequency (N4.3, N7.1, N7.7).
///
/// # Errors
/// Returns [`ClassicalError::InvalidPolynomial`] for an invalid transfer function,
/// [`ClassicalError::InvalidInput`] for an empty grid or a frequency that is not positive and finite,
/// [`ClassicalError::SolverFailed`] when a root cannot be computed, and
/// [`ClassicalError::NonfiniteResponse`] when a frequency is singular (N4.3).
pub fn analyze_transfer(
    transfer: &Transfer,
    frequencies: &[f64],
) -> Result<TransferAnalysis, ClassicalError> {
    validate_transfer(transfer)?;
    if frequencies.is_empty()
        || !frequencies
            .iter()
            .all(|frequency| frequency.is_finite() && *frequency > 0.0)
    {
        return Err(ClassicalError::InvalidInput);
    }
    let poles =
        roots_of_real_polynomial(&transfer.denominator).ok_or(ClassicalError::SolverFailed)?;
    let zeros = if transfer.numerator.len() <= 1 {
        Vec::new()
    } else {
        roots_of_real_polynomial(&transfer.numerator).ok_or(ClassicalError::SolverFailed)?
    };
    let one = ComplexValue {
        real: 1.0,
        imaginary: 0.0,
    };
    let bode: Vec<BodePoint> = frequencies
        .iter()
        .map(|&frequency| {
            let loop_value = transfer_value(transfer, frequency);
            let sensitivity = complex_divide(one, complex_add(one, loop_value));
            let complementary = complex_multiply(loop_value, sensitivity);
            let complementarity = complex_add(sensitivity, complementary);
            BodePoint {
                frequency_rad_per_second: frequency,
                magnitude_db: 20.0 * magnitude(loop_value).log10(),
                phase_degrees: loop_value.imaginary.atan2(loop_value.real) * 180.0 / PI,
                sensitivity,
                complementary_sensitivity: complementary,
                sensitivity_complementarity_residual: magnitude(ComplexValue {
                    real: complementarity.real - 1.0,
                    imaginary: complementarity.imaginary,
                }),
            }
        })
        .collect();
    if bode.iter().any(|point| {
        ![
            point.magnitude_db,
            point.phase_degrees,
            point.sensitivity.real,
            point.sensitivity.imaginary,
            point.complementary_sensitivity.real,
            point.complementary_sensitivity.imaginary,
        ]
        .iter()
        .all(|value| value.is_finite())
    }) {
        return Err(ClassicalError::NonfiniteResponse);
    }
    Ok(TransferAnalysis { poles, zeros, bode })
}

/// The margins of the loop `G` on a logarithmic sweep, from its first crossings (N7.7, N7.8). The gain
/// crossover is the first point where `|G|` passes 0 dB; the phase crossover is the first where the
/// unwrapped phase passes −180°. Each is interpolated on the logarithm of the frequency.
///
/// # Errors
/// Returns [`ClassicalError::InvalidInput`] for an invalid sweep, and [`ClassicalError::SolverFailed`]
/// when the loop has no gain crossover on the grid.
pub fn classical_stability_margins(
    transfer: &Transfer,
    sweep: Sweep,
) -> Result<Margins, ClassicalError> {
    let frequencies = logarithmic_frequencies(sweep)?;
    let values: Vec<ComplexValue> = frequencies
        .iter()
        .map(|&frequency| transfer_value(transfer, frequency))
        .collect();
    let phases = unwrap_degrees(
        &values
            .iter()
            .map(|value| value.imaginary.atan2(value.real) * 180.0 / PI)
            .collect::<Vec<_>>(),
    );
    let magnitudes: Vec<f64> = values
        .iter()
        .map(|value| 20.0 * magnitude(*value).log10())
        .collect();
    let gain_crossover =
        crossing(&frequencies, &magnitudes, 0.0).ok_or(ClassicalError::SolverFailed)?;
    let phase_margin = 180.0 + interpolate_series(&frequencies, &phases, gain_crossover);
    let phase_crossover = crossing(&frequencies, &phases, -180.0);
    let gain_margin = match phase_crossover {
        None => GainMargin::Unbounded,
        Some(frequency) => {
            GainMargin::Decibels(-interpolate_series(&frequencies, &magnitudes, frequency))
        }
    };
    let delay_margin = if phase_margin > 0.0 {
        phase_margin * PI / 180.0 / gain_crossover
    } else {
        0.0
    };
    Ok(Margins {
        gain_crossover_rad_per_second: gain_crossover,
        phase_crossover_rad_per_second: phase_crossover,
        gain_margin,
        phase_margin_degrees: phase_margin,
        delay_margin_seconds: delay_margin,
    })
}

/// The roots of a real polynomial with descending coefficients, after its leading coefficients of size
/// at most `1e-15` are removed and the polynomial is made monic. `None` for an empty, nonfinite or
/// all-but-zero polynomial, or when the eigenvalue solver fails.
#[must_use]
pub fn roots_of_real_polynomial(coefficients: &[f64]) -> Option<Vec<ComplexValue>> {
    let normalized = normalized_polynomial(coefficients)?;
    polynomial::roots(&normalized).ok()
}

/// The polynomial without its leading coefficients of size at most `1e-15`, divided by its leading
/// coefficient. `None` when it is empty, not finite, or has no coefficient above that size.
#[must_use]
pub fn normalized_polynomial(coefficients: &[f64]) -> Option<Vec<f64>> {
    if coefficients.is_empty() || !coefficients.iter().all(|value| value.is_finite()) {
        return None;
    }
    let first = coefficients.iter().position(|value| value.abs() > 1e-15)?;
    let trimmed = &coefficients[first..];
    let leading = trimmed[0];
    Some(trimmed.iter().map(|value| value / leading).collect())
}

/// The transfer function is valid: a finite, nonnegative delay, and a denominator and a numerator
/// that are not zero polynomials.
pub(crate) fn validate_transfer(transfer: &Transfer) -> Result<(), ClassicalError> {
    if !transfer.delay_seconds.is_finite()
        || transfer.delay_seconds < 0.0
        || normalized_polynomial(&transfer.denominator).is_none()
        || normalized_polynomial(&transfer.numerator).is_none()
    {
        return Err(ClassicalError::InvalidPolynomial);
    }
    Ok(())
}

/// The first point where `y − target` changes sign or is zero, with the crossing interpolated on
/// the logarithm of `x`. `None` when there is none.
fn crossing(x: &[f64], y: &[f64], target: f64) -> Option<f64> {
    for index in 1..y.len() {
        let before = y[index - 1] - target;
        let after = y[index] - target;
        if is_exact_zero(before) {
            return Some(x[index - 1]);
        }
        if before * after <= 0.0 {
            let fraction = before.abs() / (before.abs() + after.abs());
            return Some(
                (x[index - 1].ln() + fraction * (x[index].ln() - x[index - 1].ln())).exp(),
            );
        }
    }
    None
}

/// The value of a series `y` at `value`, interpolated linearly on the logarithm of `x`. A value at or
/// below the first point, or beyond the last, takes the nearest end of the series.
fn interpolate_series(x: &[f64], y: &[f64], value: f64) -> f64 {
    match x.iter().position(|candidate| *candidate >= value) {
        None | Some(0) => y[0],
        Some(upper) => {
            let fraction = (value.ln() - x[upper - 1].ln()) / (x[upper].ln() - x[upper - 1].ln());
            y[upper - 1] + fraction * (y[upper] - y[upper - 1])
        }
    }
}

/// The phases in degrees, each moved by a multiple of 360° to lie within 180° of the one before.
fn unwrap_degrees(values: &[f64]) -> Vec<f64> {
    let mut result: Vec<f64> = Vec::with_capacity(values.len());
    for &value in values {
        let mut current = value;
        if let Some(&previous) = result.last() {
            while current - previous > 180.0 {
                current -= 360.0;
            }
            while current - previous < -180.0 {
                current += 360.0;
            }
        }
        result.push(current);
    }
    result
}

/// The fraction `index / last` of a sweep position, as a floating-point number.
fn index_fraction(index: usize, last: usize) -> f64 {
    if last == 0 {
        return 0.0;
    }
    f64::from(u32::try_from(index).unwrap_or(u32::MAX))
        / f64::from(u32::try_from(last).unwrap_or(u32::MAX))
}

/// The value of a polynomial with real coefficients at a complex point, by Horner's rule.
fn evaluate(coefficients: &[f64], point: ComplexValue) -> ComplexValue {
    coefficients.iter().fold(
        ComplexValue {
            real: 0.0,
            imaginary: 0.0,
        },
        |result, coefficient| {
            complex_add(
                complex_multiply(result, point),
                ComplexValue {
                    real: *coefficient,
                    imaginary: 0.0,
                },
            )
        },
    )
}

/// The sum of two complex numbers.
pub(crate) fn complex_add(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: left.real + right.real,
        imaginary: left.imaginary + right.imaginary,
    }
}

/// The product of two complex numbers.
pub(crate) fn complex_multiply(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: left.real * right.real - left.imaginary * right.imaginary,
        imaginary: left.real * right.imaginary + left.imaginary * right.real,
    }
}

/// The quotient of two complex numbers, by the conjugate of the divisor. A zero divisor gives values
/// that are not finite.
pub(crate) fn complex_divide(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    let denominator = right.real * right.real + right.imaginary * right.imaginary;
    ComplexValue {
        real: (left.real * right.real + left.imaginary * right.imaginary) / denominator,
        imaginary: (left.imaginary * right.real - left.real * right.imaginary) / denominator,
    }
}

/// The modulus of a complex number.
pub(crate) fn magnitude(value: ComplexValue) -> f64 {
    value.real.hypot(value.imaginary)
}

/// Whether a value is exactly zero, `+0` or `-0`. A crossing at exactly zero is taken at its point.
#[allow(clippy::float_cmp)] // The crossing rule takes an exact zero as the crossing itself.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
#[path = "classical_tests.rs"]
mod tests;
