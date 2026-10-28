//! The Nyquist encirclement count of an open loop on a logarithmic sweep (N7.10).
//!
//! The positive-frequency curve `L(jω)` is closed by its mirror image, which the negative frequencies
//! give for a real model, and by its value at zero frequency. The contour is then traversed from the
//! highest negative frequency to the highest positive one. The counter-clockwise encirclements of −1 by
//! this contour, subtracted from the open-loop right-half-plane poles, give the closed-loop count.
//!
//! The count is evidence for the sampled contour only. A truncated curve does not establish a complete
//! encirclement count, and the result keeps the contour and the pole count that it uses (N7.10).

use std::f64::consts::PI;

use crate::classical::{
    logarithmic_frequencies, magnitude, roots_of_real_polynomial, transfer_value,
    validate_transfer, ClassicalError, Sweep, Transfer,
};
use crate::linalg::ComplexValue;

/// The real part below which an open-loop pole counts as on the imaginary axis (N4.2).
const POLE_AXIS_TOLERANCE: f64 = 1e-9;
/// The magnitude of the curve's last point above which the contour is not closed near zero.
const CLOSURE_TOLERANCE: f64 = 0.02;
/// The distance from −1 below which the contour passes through the critical point.
const CRITICAL_DISTANCE: f64 = 0.005;

/// The result of a Nyquist count on a sweep.
#[derive(Debug, Clone, PartialEq)]
pub struct Nyquist {
    /// The counter-clockwise encirclements of −1 by the contour.
    pub encirclements_counterclockwise: i64,
    /// The open-loop poles with a positive real part.
    pub open_loop_right_half_plane_poles: usize,
    /// The closed-loop poles with a positive real part, `P − N` (N7.10).
    pub closed_loop_right_half_plane_poles: i64,
    /// Whether the closed loop has no right-half-plane poles on this evidence.
    pub closed_loop_stable: bool,
    /// The smallest distance from the contour to −1.
    pub minimum_distance_to_minus_one: f64,
    /// The magnitude of the curve at the highest frequency of the sweep.
    pub endpoint_magnitude: f64,
    /// The contour, from the highest negative frequency to the highest positive one.
    pub contour: Vec<ComplexValue>,
}

/// The Nyquist count of the open loop `transfer` on `sweep` (N7.10).
///
/// # Errors
/// Returns [`ClassicalError::InvalidPolynomial`] for an invalid transfer function,
/// [`ClassicalError::InvalidInput`] for an invalid sweep, [`ClassicalError::PoleOnContour`] for a pole
/// on the imaginary axis, [`ClassicalError::InsufficientContourClosure`] when the curve is not near zero
/// at the top of the sweep, [`ClassicalError::ContourNearCriticalPoint`] when the contour passes within
/// `0.005` of −1, and [`ClassicalError::SolverFailed`] when the poles cannot be computed.
#[allow(clippy::cast_possible_truncation)] // The winding is a whole number of turns, small by construction.
pub fn nyquist_stability(transfer: &Transfer, sweep: Sweep) -> Result<Nyquist, ClassicalError> {
    validate_transfer(transfer)?;
    let frequencies = logarithmic_frequencies(sweep)?;
    let open_loop_poles =
        roots_of_real_polynomial(&transfer.denominator).ok_or(ClassicalError::SolverFailed)?;
    if open_loop_poles
        .iter()
        .any(|pole| pole.real.abs() <= POLE_AXIS_TOLERANCE)
    {
        return Err(ClassicalError::PoleOnContour);
    }
    let positive: Vec<ComplexValue> = frequencies
        .iter()
        .map(|&frequency| transfer_value(transfer, frequency))
        .collect();
    let endpoint_magnitude = positive.last().map_or(0.0, |value| magnitude(*value));
    if endpoint_magnitude > CLOSURE_TOLERANCE {
        return Err(ClassicalError::InsufficientContourClosure);
    }
    let mut contour: Vec<ComplexValue> = positive
        .iter()
        .rev()
        .map(|value| conjugate(*value))
        .collect();
    contour.push(transfer_value(transfer, 0.0));
    contour.extend(positive.iter().copied());
    let minimum_distance_to_minus_one = contour
        .iter()
        .map(|value| {
            magnitude(ComplexValue {
                real: value.real + 1.0,
                imaginary: value.imaginary,
            })
        })
        .fold(f64::INFINITY, f64::min);
    if minimum_distance_to_minus_one < CRITICAL_DISTANCE {
        return Err(ClassicalError::ContourNearCriticalPoint);
    }
    let angles = unwrap_radians(
        &contour
            .iter()
            .map(|value| value.imaginary.atan2(value.real + 1.0))
            .collect::<Vec<_>>(),
    );
    // The number of turns is the rounded change of angle, rounded half upward as JavaScript rounds.
    let turns = ((angles[angles.len() - 1] - angles[0]) / (2.0 * PI) + 0.5).floor();
    let encirclements = turns as i64;
    let open_loop_right_half_plane_poles = open_loop_poles
        .iter()
        .filter(|pole| pole.real > POLE_AXIS_TOLERANCE)
        .count();
    // With this traversal, counter-clockwise encirclements subtract from the open-loop count (N7.10).
    let closed_loop_right_half_plane_poles =
        i64::try_from(open_loop_right_half_plane_poles).unwrap_or(i64::MAX) - encirclements;
    Ok(Nyquist {
        encirclements_counterclockwise: encirclements,
        open_loop_right_half_plane_poles,
        closed_loop_right_half_plane_poles,
        closed_loop_stable: closed_loop_right_half_plane_poles == 0,
        minimum_distance_to_minus_one,
        endpoint_magnitude,
        contour,
    })
}

/// The complex conjugate of a value.
fn conjugate(value: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: value.real,
        imaginary: -value.imaginary,
    }
}

/// The angles in radians, each moved by a multiple of 2π to lie within π of the one before.
fn unwrap_radians(values: &[f64]) -> Vec<f64> {
    let mut result: Vec<f64> = Vec::with_capacity(values.len());
    for &value in values {
        let mut current = value;
        if let Some(&previous) = result.last() {
            while current - previous > PI {
                current -= 2.0 * PI;
            }
            while current - previous < -PI {
                current += 2.0 * PI;
            }
        }
        result.push(current);
    }
    result
}

#[cfg(test)]
#[path = "nyquist_tests.rs"]
mod tests;
