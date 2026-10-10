//! Stability margins of a loop `L(jω)` (N7.7 to N7.12; B4.6 and C0 of the Control Studio
//! specification): the gain, phase and delay margins with their crossover frequencies, the peak
//! sensitivity `Ms`, and the closed-loop stability from the poles.
//!
//! A crossing is searched on the frequency grid by sign changes, and each one is refined by
//! bisection on the geometric mean, so its frequency is exact to the bisection tolerance. The grid
//! decides only where to look. The gain crossover is `|L| = 1`; the phase crossover is an imaginary
//! part of zero with a negative real part (a phase of −180°).
//!
//! Two facts are exact for a delay-free continuous transfer function. The magnitude crossings are the
//! positive roots of `|D(jω)|² − |N(jω)|²`, and the phase crossings are the positive roots of
//! `Im(N(jω)·conj D(jω)) = ω·R(ω²)`. When the coefficients of such a polynomial have no sign change,
//! Descartes' rule of signs leaves it no positive root, so the crossing is absent over the whole
//! frequency axis and its margin is unbounded (N7.7). A delay leaves the magnitude polynomial exact
//! (a delay does not change a magnitude), but it moves the phase crossings off the polynomial. An
//! absence within the grid is only not observed in range (N7.8). It is never reported as infinite,
//! and sampled or state-space loops, which have no such proof, are never unbounded.

use std::f64::consts::{PI, TAU};

use nalgebra::Complex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::frequency::{delay_of, siso_value, FrequencyError, FrequencyModel};
use crate::linalg::{eigenvalues, ComplexValue, DenseMatrix, NumericalTolerance};
use crate::model::Timing;

/// The relative bracket width at which a bisection stops.
const BRACKET_TOLERANCE: f64 = 1e-13;
/// The most bisection steps. A bracket reaches [`BRACKET_TOLERANCE`] long before this.
const BISECTION_STEPS: usize = 200;
/// The relative tolerance with which a pole counts as on the stability boundary (N4.2).
const BOUNDARY_TOLERANCE: f64 = 1e-9;
/// The relative size below which a leading coefficient counts as zero: an algebraic loop (N7.11).
const ALGEBRAIC_TOLERANCE: f64 = 1e-12;
/// The tolerance of the closed-loop pole solver (the eigenvalue tolerance of the library).
const POLE_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// A margin: finite, unbounded, or not observed in the evaluated range (N7.7, N7.8).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MarginValue {
    /// The value of the margin.
    Finite(f64),
    /// The loop has no such crossing over the whole frequency axis, so the margin is infinite (N7.7).
    Unbounded,
    /// No such crossing was found in the evaluated range. The margin is unavailable, never infinite (N7.8).
    NotObservedInRange,
}

/// Why the closed-loop poles are not computed (N7.11, N7.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClosedLoopUnavailable {
    /// A positive pure delay: the exact closed-loop pole calculation is unavailable (N7.12). This is
    /// not evidence of instability.
    PureDelay,
    /// A state-space loop: the closed-loop poles are not formed from a transfer function.
    StateSpaceLoop,
    /// `1 + L(∞) = 0`: the loop is algebraically singular, so it is rejected before the feedback
    /// analysis (N7.11).
    AlgebraicLoop,
    /// The characteristic polynomial `D + N` is zero, so it has no poles to report.
    DegenerateCharacteristic,
}

/// The closed-loop stability of a loop from its poles (N4.2, N7.11). The stable region is the open left
/// half-plane for a continuous loop and the open unit disc for a discrete one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClosedLoopStability {
    /// Every closed-loop pole lies strictly in the stable region.
    Stable {
        /// The closed-loop poles (`z` for a discrete loop).
        poles: Vec<ComplexValue>,
    },
    /// No pole lies outside the stable region, but at least one lies on its boundary. That is
    /// marginal, not stable (N4.2).
    Marginal {
        /// The closed-loop poles.
        poles: Vec<ComplexValue>,
    },
    /// At least one closed-loop pole lies outside the stable region.
    Unstable {
        /// The closed-loop poles.
        poles: Vec<ComplexValue>,
    },
    /// The closed-loop poles are not computed, for the reason given.
    Unavailable {
        /// Why the poles are not computed.
        reason: ClosedLoopUnavailable,
    },
}

/// The margins and the closed-loop stability of a loop on a frequency grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarginReport {
    /// The gain-crossover frequency `ωc` in rad/s, where `|L| = 1`. Absent when the crossover is
    /// unbounded or not observed in range.
    pub gain_crossover_rad_per_second: Option<f64>,
    /// The phase-crossover frequency `ω180` in rad/s of the reported gain margin.
    pub phase_crossover_rad_per_second: Option<f64>,
    /// The gain margin `−20·log10|L(jω180)|` in dB.
    pub gain_margin_db: MarginValue,
    /// The phase margin `180° + ∠L(jωc)` in degrees.
    pub phase_margin_degrees: MarginValue,
    /// The delay margin `PM in radians / ωc` in seconds. Negative for an unstable loop.
    pub delay_margin_seconds: MarginValue,
    /// The peak sensitivity `Ms = max |1/(1 + L(jω))|` over the grid (N7.7, N7.8).
    pub peak_sensitivity: f64,
    /// The grid frequency in rad/s at which `Ms` is attained.
    pub peak_sensitivity_rad_per_second: f64,
    /// The closed-loop stability from the poles.
    pub closed_loop: ClosedLoopStability,
}

/// Why the margins of a loop are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MarginError {
    /// The loop's frequency response is refused (N4.3).
    #[error("the loop's frequency response is refused: {0}")]
    Frequency(#[from] FrequencyError),
    /// The grid needs two or more finite, positive and strictly increasing frequencies.
    #[error(
        "the frequency grid needs two or more finite, positive, strictly increasing frequencies"
    )]
    InvalidGrid,
    /// More than one gain crossover lies in the range, so the gain crossover is ambiguous.
    #[error("more than one gain crossover lies in the range")]
    AmbiguousGainCrossover,
    /// A pure delay moves the phase by `L·Δω` per grid step. Above `π/2` the estimate is unavailable
    /// until the range is narrowed (N7.8).
    #[error("the grid is too coarse for the pure delay: L·Δω is above π/2")]
    DelayPhaseTooFast,
    /// A crossing polynomial is identically zero, so its crossings are undefined.
    #[error("a crossing polynomial of the loop is identically zero")]
    DegenerateLoop,
    /// The closed-loop poles could not be computed.
    #[error("the closed-loop poles could not be computed")]
    PoleSolverFailed,
}

/// The crossings of one kind: absent over the whole frequency axis, or the frequencies found in range.
enum Crossings {
    WholeAxisAbsent,
    InRange(Vec<f64>),
}

/// The loop read on its grid: the model to evaluate between samples, the grid, the sampled values and
/// their unwrapped phases in radians.
struct LoopGrid<'a> {
    model: &'a FrequencyModel,
    omegas: &'a [f64],
    values: Vec<Complex<f64>>,
    phases: Vec<f64>,
}

/// Reads the margins and the closed-loop stability of a loop on a frequency grid (N7.7 to N7.12).
///
/// The loop is the whole open loop `L = G·C` as one model, with its delay. A transfer-function loop
/// must keep the factors of its plant and controller uncancelled, so that the closed-loop
/// characteristic polynomial `D + N` (N7.1) holds every mode.
///
/// # Errors
/// Returns [`MarginError::InvalidGrid`] for a grid that is not positive, finite and strictly
/// increasing with two or more points, [`MarginError::Frequency`] for a refused frequency response,
/// [`MarginError::DelayPhaseTooFast`] when `L·Δω` exceeds `π/2`, [`MarginError::AmbiguousGainCrossover`]
/// when more than one gain crossover lies in the range, [`MarginError::DegenerateLoop`] for an
/// identically zero crossing polynomial, and [`MarginError::PoleSolverFailed`] when the poles cannot be
/// computed.
pub fn analyze_loop(
    loop_model: &FrequencyModel,
    grid: &[f64],
) -> Result<MarginReport, MarginError> {
    validate_grid(grid)?;
    let delay_seconds = delay_of(loop_model);
    if delay_seconds > 0.0 && max_spacing(grid) * delay_seconds > PI / 2.0 {
        return Err(MarginError::DelayPhaseTooFast);
    }
    let samples = grid
        .iter()
        .map(|&omega| siso_value(loop_model, omega))
        .collect::<Result<Vec<Complex<f64>>, FrequencyError>>()?;
    let phases = unwrapped_phases(&samples);
    let loop_grid = LoopGrid {
        model: loop_model,
        omegas: grid,
        values: samples,
        phases,
    };
    let (gain_crossover, phase_margin, delay_margin) = gain_outcome(&loop_grid)?;
    let (phase_crossover, gain_margin) = phase_outcome(&loop_grid)?;
    let (peak, peak_omega) = peak_sensitivity(&loop_grid)?;
    Ok(MarginReport {
        gain_crossover_rad_per_second: gain_crossover,
        phase_crossover_rad_per_second: phase_crossover,
        gain_margin_db: gain_margin,
        phase_margin_degrees: phase_margin,
        delay_margin_seconds: delay_margin,
        peak_sensitivity: peak,
        peak_sensitivity_rad_per_second: peak_omega,
        closed_loop: closed_loop(loop_model)?,
    })
}

/// The gain crossover, the phase margin in degrees and the delay margin in seconds (N7.7).
fn gain_outcome(grid: &LoopGrid) -> Result<(Option<f64>, MarginValue, MarginValue), MarginError> {
    match gain_crossings(grid)? {
        Crossings::WholeAxisAbsent => Ok((None, MarginValue::Unbounded, MarginValue::Unbounded)),
        Crossings::InRange(crossings) => match crossings.as_slice() {
            [] => Ok((
                None,
                MarginValue::NotObservedInRange,
                MarginValue::NotObservedInRange,
            )),
            [omega] => {
                let phase_margin = phase_margin_at(grid, *omega)?;
                let delay_margin = phase_margin.to_radians() / *omega;
                Ok((
                    Some(*omega),
                    MarginValue::Finite(phase_margin),
                    MarginValue::Finite(delay_margin),
                ))
            }
            _ => Err(MarginError::AmbiguousGainCrossover),
        },
    }
}

/// The phase-crossover frequency and the gain margin in dB. Where several phase crossovers lie in the
/// range, the smallest gain margin is reported, so the worst case stays visible.
fn phase_outcome(grid: &LoopGrid) -> Result<(Option<f64>, MarginValue), MarginError> {
    match phase_crossings(grid)? {
        Crossings::WholeAxisAbsent => Ok((None, MarginValue::Unbounded)),
        Crossings::InRange(crossings) => {
            let mut worst: Option<(f64, f64)> = None;
            for omega in crossings {
                let gain_margin = gain_margin_db(grid, omega)?;
                match worst {
                    Some((_, current)) if current <= gain_margin => {}
                    _ => worst = Some((omega, gain_margin)),
                }
            }
            match worst {
                None => Ok((None, MarginValue::NotObservedInRange)),
                Some((omega, gain_margin)) if gain_margin.is_finite() => {
                    Ok((Some(omega), MarginValue::Finite(gain_margin)))
                }
                Some((omega, _)) => Ok((Some(omega), MarginValue::Unbounded)),
            }
        }
    }
}

/// The gain-crossover frequencies: the grid bracketed roots of `|L| − 1`, or the proof that there are
/// none. The magnitude polynomial is exact only for a continuous transfer function.
fn gain_crossings(grid: &LoopGrid) -> Result<Crossings, MarginError> {
    if let FrequencyModel::Transfer {
        transfer,
        timing: Timing::Continuous,
        ..
    } = grid.model
    {
        let polynomial = gain_polynomial(
            &ascending(&transfer.numerator),
            &ascending(&transfer.denominator),
        );
        if all_zero(&polynomial) {
            return Err(MarginError::DegenerateLoop);
        }
        if sign_variations(&polynomial) == 0 {
            return Ok(Crossings::WholeAxisAbsent);
        }
    }
    Ok(Crossings::InRange(
        grid.roots(&|value: Complex<f64>| value.norm() - 1.0)?,
    ))
}

/// The phase-crossover frequencies: the frequencies where `Im L = 0` with `Re L < 0`. For a delay-free
/// continuous transfer function the imaginary part is exact, so its absence is proven.
fn phase_crossings(grid: &LoopGrid) -> Result<Crossings, MarginError> {
    if let FrequencyModel::Transfer {
        transfer,
        timing: Timing::Continuous,
        delay_seconds,
    } = grid.model
    {
        if *delay_seconds == 0.0 {
            let polynomial = imaginary_polynomial(
                &ascending(&transfer.numerator),
                &ascending(&transfer.denominator),
            );
            if all_zero(&polynomial) {
                return Err(MarginError::DegenerateLoop);
            }
            if sign_variations(&polynomial) == 0 {
                return Ok(Crossings::WholeAxisAbsent);
            }
        }
    }
    let mut crossings = Vec::new();
    for omega in grid.roots(&|value: Complex<f64>| value.im)? {
        if grid.value_at(omega)?.re < 0.0 {
            crossings.push(omega);
        }
    }
    Ok(Crossings::InRange(crossings))
}

/// The phase margin in degrees at the gain crossover `omega`: `180° + ∠L(jω)`. The phase is the
/// unwrapped grid phase of the sample below `omega`, plus the wrapped change from that sample to
/// `omega`, so it is continuous from the start of the grid.
fn phase_margin_at(grid: &LoopGrid, omega: f64) -> Result<f64, MarginError> {
    let value = grid.value_at(omega)?;
    let bracket = grid
        .omegas
        .partition_point(|&candidate| candidate <= omega)
        .saturating_sub(1);
    let phase = grid.phases[bracket] + wrap_pi(value.arg() - grid.values[bracket].arg());
    Ok(180.0 + phase.to_degrees())
}

/// The gain margin `−20·log10|L(jω)|` in dB at a phase crossover.
fn gain_margin_db(grid: &LoopGrid, omega: f64) -> Result<f64, MarginError> {
    let magnitude = grid.value_at(omega)?.norm();
    Ok(-20.0 * magnitude.log10())
}

/// The peak sensitivity over the grid and the frequency where it is attained (N7.7).
fn peak_sensitivity(grid: &LoopGrid) -> Result<(f64, f64), MarginError> {
    let mut peak: Option<(f64, f64)> = None;
    for (omega, sample) in grid.omegas.iter().zip(&grid.values) {
        let sensitivity = 1.0 / (Complex::new(1.0, 0.0) + *sample).norm();
        if !sensitivity.is_finite() {
            return Err(MarginError::Frequency(FrequencyError::Singular));
        }
        match peak {
            Some((best, _)) if best >= sensitivity => {}
            _ => peak = Some((sensitivity, *omega)),
        }
    }
    peak.ok_or(MarginError::InvalidGrid)
}

/// The closed-loop stability from the poles of `1 + L = 0` (N7.11). A pure delay and a state-space loop
/// have no such poles here (N7.12).
fn closed_loop(model: &FrequencyModel) -> Result<ClosedLoopStability, MarginError> {
    let FrequencyModel::Transfer {
        transfer,
        timing,
        delay_seconds,
    } = model
    else {
        return Ok(ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::StateSpaceLoop,
        });
    };
    if *delay_seconds > 0.0 {
        return Ok(ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::PureDelay,
        });
    }
    if transfer.numerator.len() == transfer.denominator.len() {
        if let (Some(leading_denominator), Some(leading_numerator)) =
            (transfer.denominator.first(), transfer.numerator.first())
        {
            let sum = leading_denominator + leading_numerator;
            if sum.abs()
                <= ALGEBRAIC_TOLERANCE * (leading_denominator.abs() + leading_numerator.abs())
            {
                return Ok(ClosedLoopStability::Unavailable {
                    reason: ClosedLoopUnavailable::AlgebraicLoop,
                });
            }
        }
    }
    let Some(characteristic) = trimmed(characteristic_polynomial(
        &transfer.numerator,
        &transfer.denominator,
    )) else {
        return Ok(ClosedLoopStability::Unavailable {
            reason: ClosedLoopUnavailable::DegenerateCharacteristic,
        });
    };
    if characteristic.len() == 1 {
        return Ok(ClosedLoopStability::Stable { poles: Vec::new() });
    }
    let poles = polynomial_roots(&characteristic)?;
    Ok(classify(poles, *timing))
}

/// Classifies poles against the stable region of their timing (N4.2). A pole inside the region is
/// stable, one on its boundary is marginal, and one outside it is unstable. Any unstable pole makes
/// the loop unstable.
fn classify(poles: Vec<ComplexValue>, timing: Timing) -> ClosedLoopStability {
    let mut unstable = false;
    let mut marginal = false;
    for pole in &poles {
        let modulus = pole.real.hypot(pole.imaginary);
        let tolerance = BOUNDARY_TOLERANCE * (1.0 + modulus);
        let distance = match timing {
            Timing::Continuous => pole.real,
            Timing::Discrete { .. } => modulus - 1.0,
        };
        if distance > tolerance {
            unstable = true;
        } else if distance.abs() <= tolerance {
            marginal = true;
        }
    }
    if unstable {
        ClosedLoopStability::Unstable { poles }
    } else if marginal {
        ClosedLoopStability::Marginal { poles }
    } else {
        ClosedLoopStability::Stable { poles }
    }
}

/// The roots of a descending polynomial with a nonzero leading coefficient and at least one more
/// coefficient, as the eigenvalues of its companion matrix.
fn polynomial_roots(descending: &[f64]) -> Result<Vec<ComplexValue>, MarginError> {
    let Some((&leading, rest)) = descending.split_first() else {
        return Ok(Vec::new());
    };
    let degree = rest.len();
    let monic: Vec<f64> = rest
        .iter()
        .map(|coefficient| coefficient / leading)
        .collect();
    let companion: DenseMatrix = (0..degree)
        .map(|row| {
            (0..degree)
                .map(|column| {
                    if row == 0 {
                        -monic[column]
                    } else if column + 1 == row {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    let solution =
        eigenvalues(&companion, &POLE_TOLERANCE).map_err(|_| MarginError::PoleSolverFailed)?;
    Ok(solution.values)
}

/// The characteristic polynomial `D(s) + N(s)` of the unit-feedback loop `L = N/D` (N7.1), descending.
/// The shorter polynomial is padded at its front with zeros, so the powers line up.
fn characteristic_polynomial(numerator: &[f64], denominator: &[f64]) -> Vec<f64> {
    let length = numerator.len().max(denominator.len());
    let padded = |coefficients: &[f64]| {
        let mut result = vec![0.0; length - coefficients.len()];
        result.extend_from_slice(coefficients);
        result
    };
    padded(denominator)
        .iter()
        .zip(padded(numerator))
        .map(|(denominator_term, numerator_term)| denominator_term + numerator_term)
        .collect()
}

/// The coefficients without their leading zeros, or `None` when all of them are zero.
fn trimmed(coefficients: Vec<f64>) -> Option<Vec<f64>> {
    let start = coefficients
        .iter()
        .position(|coefficient| *coefficient != 0.0)?;
    Some(coefficients.into_iter().skip(start).collect())
}

/// `|D(jω)|² − |N(jω)|²` as a polynomial in `x = ω²`, ascending. Its positive roots are the gain
/// crossings, and it is positive where the loop gain is below one.
fn gain_polynomial(numerator: &[f64], denominator: &[f64]) -> Vec<f64> {
    let denominator_modulus = modulus_squared(denominator);
    let numerator_modulus = modulus_squared(numerator);
    let length = denominator_modulus.len().max(numerator_modulus.len());
    (0..length)
        .map(|power| {
            coefficient_at(&denominator_modulus, power) - coefficient_at(&numerator_modulus, power)
        })
        .collect()
}

/// `Im(N(jω)·conj D(jω)) = ω·R(ω²)`, with `R` ascending in `x = ω²`: `R_m = (−1)^m Σ_{i+j=2m+1} (−1)^j nᵢdⱼ`.
/// Both polynomials are ascending. The imaginary part of `L` has the sign of `R`.
fn imaginary_polynomial(numerator: &[f64], denominator: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; numerator.len().midpoint(denominator.len()) + 1];
    for (row, numerator_coefficient) in numerator.iter().enumerate() {
        for (column, denominator_coefficient) in denominator.iter().enumerate() {
            if (row + column) % 2 == 1 {
                let power = (row + column - 1) / 2;
                result[power] += parity_sign(power)
                    * parity_sign(column)
                    * *numerator_coefficient
                    * *denominator_coefficient;
            }
        }
    }
    result
}

/// `|p(jω)|²` as a polynomial in `x = ω²`, ascending, for an ascending polynomial `p`. Its coefficient of
/// `x^m` is `(−1)^m Σ_{i+j=2m} (−1)^j aᵢaⱼ`.
fn modulus_squared(ascending_coefficients: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; 2 * ascending_coefficients.len().saturating_sub(1) + 1];
    for (row, first) in ascending_coefficients.iter().enumerate() {
        for (column, second) in ascending_coefficients.iter().enumerate() {
            if (row + column).is_multiple_of(2) {
                let power = row.midpoint(column);
                result[power] += parity_sign(power) * parity_sign(column) * *first * *second;
            }
        }
    }
    result
}

/// The coefficients of a descending polynomial, reversed to ascending powers.
fn ascending(descending: &[f64]) -> Vec<f64> {
    descending.iter().rev().copied().collect()
}

/// `(−1)^exponent`.
fn parity_sign(exponent: usize) -> f64 {
    if exponent.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// The coefficient of `x^power`, or zero beyond the end of the polynomial.
fn coefficient_at(coefficients: &[f64], power: usize) -> f64 {
    coefficients.get(power).copied().unwrap_or(0.0)
}

fn all_zero(coefficients: &[f64]) -> bool {
    coefficients.iter().all(|coefficient| *coefficient == 0.0)
}

/// The number of sign changes among the nonzero coefficients. By Descartes' rule of signs it bounds
/// the number of positive roots, and zero changes leave no positive root.
fn sign_variations(coefficients: &[f64]) -> usize {
    let mut variations = 0;
    let mut previous_positive: Option<bool> = None;
    for coefficient in coefficients {
        if *coefficient == 0.0 {
            continue;
        }
        let positive = *coefficient > 0.0;
        if previous_positive.is_some_and(|before| before != positive) {
            variations += 1;
        }
        previous_positive = Some(positive);
    }
    variations
}

/// The unwrapped phases in radians: the first is the principal argument, and each next one adds the
/// wrapped change of the argument.
fn unwrapped_phases(samples: &[Complex<f64>]) -> Vec<f64> {
    let mut phases = Vec::with_capacity(samples.len());
    let mut previous: Option<(f64, f64)> = None;
    for sample in samples {
        let argument = sample.arg();
        let unwrapped = match previous {
            None => argument,
            Some((last_argument, last_unwrapped)) => {
                last_unwrapped + wrap_pi(argument - last_argument)
            }
        };
        phases.push(unwrapped);
        previous = Some((argument, unwrapped));
    }
    phases
}

/// The angle wrapped into `[−π, π]`.
fn wrap_pi(angle: f64) -> f64 {
    angle - TAU * (angle / TAU).round()
}

/// The largest spacing between neighbouring grid frequencies.
fn max_spacing(grid: &[f64]) -> f64 {
    grid.windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max)
}

fn validate_grid(grid: &[f64]) -> Result<(), MarginError> {
    let valid = grid.len() >= 2
        && grid.iter().all(|omega| omega.is_finite() && *omega > 0.0)
        && grid.windows(2).all(|pair| pair[1] > pair[0]);
    if valid {
        Ok(())
    } else {
        Err(MarginError::InvalidGrid)
    }
}

impl LoopGrid<'_> {
    /// The loop value at any frequency, not only at a grid point.
    fn value_at(&self, omega: f64) -> Result<Complex<f64>, MarginError> {
        siso_value(self.model, omega).map_err(MarginError::Frequency)
    }

    /// The frequencies, ascending, where `quantity` of the loop changes sign across the grid, each
    /// refined by bisection, and the grid points where it is exactly zero.
    fn roots(&self, quantity: &dyn Fn(Complex<f64>) -> f64) -> Result<Vec<f64>, MarginError> {
        let samples: Vec<f64> = self.values.iter().map(|&value| quantity(value)).collect();
        let mut found = Vec::new();
        for (index, pair) in samples.windows(2).enumerate() {
            let (before, after) = (pair[0], pair[1]);
            if before == 0.0 {
                found.push(self.omegas[index]);
            } else if before * after < 0.0 {
                found.push(self.bisect(
                    self.omegas[index],
                    self.omegas[index + 1],
                    before,
                    quantity,
                )?);
            }
        }
        if let (Some(last_sample), Some(last_omega)) = (samples.last(), self.omegas.last()) {
            if *last_sample == 0.0 {
                found.push(*last_omega);
            }
        }
        Ok(found)
    }

    /// The root of `quantity` between `low` and `high`, where `quantity(low)` has the sign of
    /// `low_sample`. The bisection works on the geometric mean, because the grid is logarithmic.
    fn bisect(
        &self,
        mut low: f64,
        mut high: f64,
        low_sample: f64,
        quantity: &dyn Fn(Complex<f64>) -> f64,
    ) -> Result<f64, MarginError> {
        let low_is_positive = low_sample > 0.0;
        for _ in 0..BISECTION_STEPS {
            if high <= low * (1.0 + BRACKET_TOLERANCE) {
                break;
            }
            let middle = (low * high).sqrt();
            let sample = quantity(self.value_at(middle)?);
            if sample == 0.0 {
                return Ok(middle);
            }
            if (sample > 0.0) == low_is_positive {
                low = middle;
            } else {
                high = middle;
            }
        }
        Ok((low * high).sqrt())
    }
}

#[cfg(test)]
mod tests;
