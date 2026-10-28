//! Frequency response of linear models (N4.3), of a selected-channel loop `L = G·C` and its
//! sensitivities (N4.4, N7.7), the logarithmic grids the frequency results are read on, and the
//! plotted phase of a response series (N4.3).
//!
//! A continuous model is evaluated at `s = jω`; a discrete model at `z = exp(jωTs)`. A pure delay
//! `L` multiplies the response by `exp(−jωL)`. A discrete model's delay must be a whole number of
//! sample periods, so that the factor is a power of `z` (as the simulation module requires). A
//! request at a singular frequency is refused, and so is a discrete request at or above the
//! Nyquist frequency `π / Ts` (N4.3). Every returned value is finite.

#![allow(clippy::module_name_repetitions)] // `FrequencyError` names the module's subject.

use nalgebra::{Complex, DMatrix};

use crate::linalg::ComplexValue;
use crate::model::{StateSpace, Timing, TransferFunction};

/// Relative tolerance with which a discrete delay must be a whole number of sample periods.
const WHOLE_SAMPLE_TOLERANCE: f64 = 1e-10;

/// A linear model with the timing and the pure input delay that its frequency response needs.
#[derive(Debug, Clone, PartialEq)]
pub enum FrequencyModel {
    /// The transfer function `N(s)/D(s)` (or `N(z)/D(z)`), coefficients from the highest power down.
    Transfer {
        /// The numerator and the denominator.
        transfer: TransferFunction,
        /// Continuous time, or discrete time with the sample period of the model.
        timing: Timing,
        /// The pure input delay `L` in seconds, zero or more.
        delay_seconds: f64,
    },
    /// The state-space model `C(sI − A)⁻¹B + D` (or `C(zI − A)⁻¹B + D`).
    StateSpace {
        /// The matrices `A`, `B`, `C` and `D`.
        system: StateSpace,
        /// Continuous time, or discrete time with the sample period of the model.
        timing: Timing,
        /// The pure input delay `L` in seconds, zero or more.
        delay_seconds: f64,
    },
}

/// Why a frequency calculation is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrequencyError {
    /// A frequency, a delay, a grid bound or a polynomial is empty, not finite or out of its domain.
    #[error("frequency, delay, grid bound or polynomial is not finite or not in range")]
    InvalidInput,
    /// A grid has fewer than two points.
    #[error("a grid needs at least two points")]
    GridTooShort,
    /// The frequency is a pole of the model, so the response is not finite (N4.3).
    #[error("the frequency is singular for the model")]
    Singular,
    /// A discrete model is evaluated at or above the Nyquist frequency `π / Ts` (N4.3).
    #[error("the frequency is at or above the Nyquist frequency")]
    AboveNyquist,
    /// A discrete model's delay is not a whole number of sample periods.
    #[error("the discrete delay is not a whole number of sample periods")]
    NonIntegerDiscreteDelay,
    /// The two models of a selected-channel loop differ in timing or sample interval (N4.4).
    #[error("the loop's models differ in timing or sample interval")]
    TimingMismatch,
    /// A selected-channel loop needs single-input, single-output models (N4.4).
    #[error("the model is not single input and single output")]
    NotSiso,
}

/// The logarithmic grid of `points` frequencies from `minimum` to `maximum` rad/s:
/// `ωₖ = minimum · (maximum / minimum)^(k / (points − 1))` for `k = 0 … points − 1`. Both end
/// points are exact. The grid is strictly increasing.
///
/// # Errors
/// Returns [`FrequencyError::GridTooShort`] for fewer than two points, and
/// [`FrequencyError::InvalidInput`] unless `0 < minimum < maximum` with both finite, or the count
/// does not fit in `u32`.
pub fn logarithmic_grid(
    minimum: f64,
    maximum: f64,
    points: usize,
) -> Result<Vec<f64>, FrequencyError> {
    if points < 2 {
        return Err(FrequencyError::GridTooShort);
    }
    let count = u32::try_from(points).map_err(|_| FrequencyError::InvalidInput)?;
    if !(minimum.is_finite() && maximum.is_finite() && minimum > 0.0 && maximum > minimum) {
        return Err(FrequencyError::InvalidInput);
    }
    let decades_in_ln = (maximum / minimum).ln();
    let last = count - 1;
    Ok((0..count)
        .map(|index| {
            if index == 0 {
                minimum
            } else if index == last {
                maximum
            } else {
                minimum * (decades_in_ln * f64::from(index) / f64::from(last)).exp()
            }
        })
        .collect())
}

/// The frequency response `G(jω)` (continuous) or `G(exp(jωTs))` (discrete) of a model at `omega`,
/// as a matrix of outputs by inputs (N4.3). A pure delay multiplies every entry by `exp(−jωL)`.
///
/// # Errors
/// Returns [`FrequencyError::InvalidInput`] for a frequency or delay that is not finite and
/// positive (frequency) or nonnegative (delay), [`FrequencyError::AboveNyquist`] and
/// [`FrequencyError::NonIntegerDiscreteDelay`] for the discrete rules, and
/// [`FrequencyError::Singular`] at a pole or a nonfinite value.
pub fn response_at(
    model: &FrequencyModel,
    omega: f64,
) -> Result<Vec<Vec<ComplexValue>>, FrequencyError> {
    let response_rows = complex_response(model, omega)?;
    Ok(response_rows
        .into_iter()
        .map(|row| row.into_iter().map(to_value).collect())
        .collect())
}

/// The selected-channel open-loop response `L(jω) = G_selected(jω)·C_selected(jω)` (N4.4). Both
/// models are single-input, single-output, and both are continuous, or both are discrete with the
/// same sample interval. A selected-channel response is a plant response, not a closed-loop result.
///
/// # Errors
/// Returns [`FrequencyError::TimingMismatch`] when the timings differ, [`FrequencyError::NotSiso`]
/// for a model that is not single-input and single-output, and the errors of [`response_at`].
pub fn selected_loop_response(
    plant: &FrequencyModel,
    controller: &FrequencyModel,
    omega: f64,
) -> Result<ComplexValue, FrequencyError> {
    if !same_timing(timing_of(plant), timing_of(controller)) {
        return Err(FrequencyError::TimingMismatch);
    }
    let product = siso_value(plant, omega)? * siso_value(controller, omega)?;
    Ok(to_value(finite_value(product)?))
}

/// The sensitivity `S = 1 / (1 + L)` of a loop value (N7.7).
///
/// # Errors
/// Returns [`FrequencyError::Singular`] when `1 + L` is zero or the value is not finite.
pub fn sensitivity(loop_value: ComplexValue) -> Result<ComplexValue, FrequencyError> {
    let one_plus_loop = Complex::new(1.0, 0.0) + to_complex(loop_value);
    let value = Complex::new(1.0, 0.0) / one_plus_loop;
    Ok(to_value(finite_value(value)?))
}

/// The complementary sensitivity `T = L / (1 + L)` of a loop value (N7.7).
///
/// # Errors
/// Returns [`FrequencyError::Singular`] when `1 + L` is zero or the value is not finite.
pub fn complementary_sensitivity(loop_value: ComplexValue) -> Result<ComplexValue, FrequencyError> {
    let loop_complex = to_complex(loop_value);
    let value = loop_complex / (Complex::new(1.0, 0.0) + loop_complex);
    Ok(to_value(finite_value(value)?))
}

/// The magnitude in decibels, `20·log10|H|`, of a response value.
#[must_use]
pub fn magnitude_db(value: ComplexValue) -> f64 {
    20.0 * value.real.hypot(value.imaginary).log10()
}

/// The principal phase in degrees, `atan2(Im H, Re H)`, of a response value. A plotted phase is
/// continuous instead ([`plotted_phase_degrees`], N4.3); the margins read the response itself.
#[must_use]
pub fn phase_degrees(value: ComplexValue) -> f64 {
    value.imaginary.atan2(value.real).to_degrees()
}

/// The plotted phase of a response series in degrees, one value per frequency (N4.3).
///
/// `delay_free` holds the response without its pure delay at each of the increasing
/// `frequencies`, and `delay_seconds` is that delay `L`; a response without a known delay, such as
/// a measured one, passes zero. The first value takes its principal phase, in `(−180°, 180°]`.
/// Each further value is moved by whole turns, so that its step from the previous value lies in
/// `(−180°, 180°]`. The delay's phase `−ω·L` is then added in closed form, so a coarse grid can
/// neither lose nor add a turn of the delay.
#[must_use]
pub fn plotted_phase_degrees(
    delay_free: &[ComplexValue],
    frequencies: &[f64],
    delay_seconds: f64,
) -> Vec<f64> {
    let mut phases = Vec::with_capacity(delay_free.len());
    let mut previous: Option<f64> = None;
    for (&value, &omega) in delay_free.iter().zip(frequencies) {
        let principal = principal_degrees(value);
        // `n = ⌊(last − principal + 180°) / 360°⌋` turns put the step from `last` in (−180°, 180°].
        let continuous = previous.map_or(principal, |last| {
            principal + 360.0 * ((last - principal + 180.0) / 360.0).floor()
        });
        previous = Some(continuous);
        phases.push(continuous - (omega * delay_seconds).to_degrees());
    }
    phases
}

/// The principal phase in degrees, in `(−180°, 180°]`. `atan2` reads −180° for a negative real
/// part with an imaginary part of −0, which is the angle 180°.
fn principal_degrees(value: ComplexValue) -> f64 {
    let phase = phase_degrees(value);
    if phase <= -180.0 {
        phase + 360.0
    } else {
        phase
    }
}

/// The response matrix of a model at `omega`, outputs by inputs, as complex numbers.
pub(crate) fn complex_response(
    model: &FrequencyModel,
    omega: f64,
) -> Result<Vec<Vec<Complex<f64>>>, FrequencyError> {
    match model {
        FrequencyModel::Transfer {
            transfer,
            timing,
            delay_seconds,
        } => Ok(vec![vec![transfer_value(
            transfer,
            *timing,
            *delay_seconds,
            omega,
        )?]]),
        FrequencyModel::StateSpace {
            system,
            timing,
            delay_seconds,
        } => state_space_values(system, *timing, *delay_seconds, omega),
    }
}

/// The single value of a single-input, single-output model at `omega`, for loop calculations.
pub(crate) fn siso_value(
    model: &FrequencyModel,
    omega: f64,
) -> Result<Complex<f64>, FrequencyError> {
    match model {
        FrequencyModel::Transfer {
            transfer,
            timing,
            delay_seconds,
        } => transfer_value(transfer, *timing, *delay_seconds, omega),
        FrequencyModel::StateSpace { system, .. } => {
            if system.inputs() != 1 || system.outputs() != 1 {
                return Err(FrequencyError::NotSiso);
            }
            complex_response(model, omega)?
                .first()
                .and_then(|row| row.first())
                .copied()
                .ok_or(FrequencyError::NotSiso)
        }
    }
}

/// The timing of a model.
pub(crate) fn timing_of(model: &FrequencyModel) -> Timing {
    match model {
        FrequencyModel::Transfer { timing, .. } | FrequencyModel::StateSpace { timing, .. } => {
            *timing
        }
    }
}

/// The pure input delay of a model in seconds.
pub(crate) fn delay_of(model: &FrequencyModel) -> f64 {
    match model {
        FrequencyModel::Transfer { delay_seconds, .. }
        | FrequencyModel::StateSpace { delay_seconds, .. } => *delay_seconds,
    }
}

/// Converts a stored complex value to the arithmetic type.
pub(crate) fn to_complex(value: ComplexValue) -> Complex<f64> {
    Complex::new(value.real, value.imaginary)
}

/// Converts an arithmetic value to the stored complex type.
pub(crate) fn to_value(value: Complex<f64>) -> ComplexValue {
    ComplexValue {
        real: value.re,
        imaginary: value.im,
    }
}

/// The value itself when both parts are finite, otherwise [`FrequencyError::Singular`] (N4.3: a
/// singular frequency never produces a nonfinite point).
pub(crate) fn finite_value(value: Complex<f64>) -> Result<Complex<f64>, FrequencyError> {
    if value.re.is_finite() && value.im.is_finite() {
        Ok(value)
    } else {
        Err(FrequencyError::Singular)
    }
}

/// Whether two timings are the same: both continuous, or both discrete with equal sample periods.
fn same_timing(left: Timing, right: Timing) -> bool {
    match (left, right) {
        (Timing::Continuous, Timing::Continuous) => true,
        (
            Timing::Discrete {
                sample_time_seconds: left_period,
            },
            Timing::Discrete {
                sample_time_seconds: right_period,
            },
        ) => (left_period - right_period).abs() <= 1e-12 * left_period.max(right_period),
        _ => false,
    }
}

/// Checks a frequency is finite and positive.
fn check_frequency(omega: f64) -> Result<(), FrequencyError> {
    if omega.is_finite() && omega > 0.0 {
        Ok(())
    } else {
        Err(FrequencyError::InvalidInput)
    }
}

/// The evaluation point `jω` (continuous) or `exp(jωTs)` (discrete) after the checks of N4.3 and
/// N4.7: a discrete request must lie below the Nyquist frequency, and a discrete delay must be a
/// whole number of sample periods.
fn evaluation_point(
    timing: Timing,
    delay_seconds: f64,
    omega: f64,
) -> Result<Complex<f64>, FrequencyError> {
    check_frequency(omega)?;
    if !(delay_seconds.is_finite() && delay_seconds >= 0.0) {
        return Err(FrequencyError::InvalidInput);
    }
    match timing {
        Timing::Continuous => Ok(Complex::new(0.0, omega)),
        Timing::Discrete {
            sample_time_seconds,
        } => {
            if omega >= std::f64::consts::PI / sample_time_seconds {
                return Err(FrequencyError::AboveNyquist);
            }
            let sample_periods = delay_seconds / sample_time_seconds;
            if (sample_periods - sample_periods.round()).abs()
                > WHOLE_SAMPLE_TOLERANCE * sample_periods.max(1.0)
            {
                return Err(FrequencyError::NonIntegerDiscreteDelay);
            }
            Ok(Complex::new(0.0, omega * sample_time_seconds).exp())
        }
    }
}

/// The factor `exp(−jωL)` of a pure delay `L`. For a discrete model the delay is a whole number of
/// samples, so the factor equals `z^(−k)`.
fn delay_factor(omega: f64, delay_seconds: f64) -> Complex<f64> {
    Complex::new(0.0, -omega * delay_seconds).exp()
}

/// The polynomial with the given coefficients (highest power first) at `point`, by Horner's rule.
fn polynomial_at(coefficients: &[f64], point: Complex<f64>) -> Complex<f64> {
    coefficients
        .iter()
        .fold(Complex::new(0.0, 0.0), |accumulated, coefficient| {
            accumulated * point + Complex::new(*coefficient, 0.0)
        })
}

/// The value `N(p)/D(p)·exp(−jωL)` of a transfer function at `omega`, where `p` is the evaluation
/// point of its timing (N4.3).
pub(crate) fn transfer_value(
    transfer: &TransferFunction,
    timing: Timing,
    delay_seconds: f64,
    omega: f64,
) -> Result<Complex<f64>, FrequencyError> {
    if transfer.numerator.is_empty() || transfer.denominator.is_empty() {
        return Err(FrequencyError::InvalidInput);
    }
    let point = evaluation_point(timing, delay_seconds, omega)?;
    let numerator = polynomial_at(&transfer.numerator, point);
    let denominator = polynomial_at(&transfer.denominator, point);
    if denominator.re == 0.0 && denominator.im == 0.0 {
        return Err(FrequencyError::Singular);
    }
    let value = numerator / denominator * delay_factor(omega, delay_seconds);
    finite_value(value)
}

/// The matrix `C(pI − A)⁻¹B + D` of a state-space model at `omega`, with `p` the evaluation point of
/// its timing, times the delay factor. The resolvent is solved by LU decomposition over the complex
/// numbers; a singular resolvent is refused (N4.3).
pub(crate) fn state_space_values(
    system: &StateSpace,
    timing: Timing,
    delay_seconds: f64,
    omega: f64,
) -> Result<Vec<Vec<Complex<f64>>>, FrequencyError> {
    let point = evaluation_point(timing, delay_seconds, omega)?;
    let states = system.states();
    let inputs = system.inputs();
    let state_response: DMatrix<Complex<f64>> = if states == 0 {
        DMatrix::zeros(0, inputs)
    } else {
        let resolvent = DMatrix::from_fn(states, states, |row, column| {
            let entry = Complex::new(system.a[row][column], 0.0);
            if row == column {
                point - entry
            } else {
                -entry
            }
        });
        let drive = DMatrix::from_fn(states, inputs, |row, column| {
            Complex::new(system.b[row][column], 0.0)
        });
        resolvent
            .lu()
            .solve(&drive)
            .ok_or(FrequencyError::Singular)?
    };
    let factor = delay_factor(omega, delay_seconds);
    (0..system.outputs())
        .map(|output| {
            (0..inputs)
                .map(|input| {
                    let coupled: Complex<f64> = (0..states)
                        .map(|state| {
                            Complex::new(system.c[output][state], 0.0)
                                * state_response[(state, input)]
                        })
                        .sum();
                    let direct = Complex::new(system.d[output][input], 0.0);
                    finite_value((coupled + direct) * factor)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
mod tests;
