//! Lead and lag design at a crossover frequency (the classical designs of the Control Studio's lead/lag
//! tool). Each design gives a compensator `K·(s/z + 1)/(s/p + 1)` whose gain makes the loop magnitude one
//! at the crossover.
//!
//! A lead adds a phase `φ` at the crossover with `α = (1 − sin φ)/(1 + sin φ)`, and places its zero and
//! pole a factor `√α` either side of the crossover. A lag places both its break points below the
//! crossover, a decade and a tenth below it, so that the crossover barely moves while the low-frequency
//! gain rises by the declared ratio.

use std::f64::consts::PI;

use crate::classical::{
    complex_divide, magnitude, transfer_value, validate_transfer, ClassicalError, Transfer,
};
use crate::linalg::ComplexValue;

/// The largest lead phase, in degrees, that the design takes (the sine must stay below one).
const MAXIMUM_LEAD_DEGREES: f64 = 85.0;

/// Which compensator to design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadLagMode {
    /// A lead compensator that adds phase at the crossover.
    Lead,
    /// A lag compensator that raises the low-frequency gain.
    Lag,
}

/// A designed compensator and the loop it gives at the crossover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadLagDesign {
    /// The gain `K` of the compensator, which makes `|K·C·G|` one at the crossover.
    pub gain: f64,
    /// The zero of the compensator in rad/s.
    pub zero_rad_per_second: f64,
    /// The pole of the compensator in rad/s.
    pub pole_rad_per_second: f64,
    /// The phase the compensator adds at the crossover, in degrees.
    pub phase_contribution_degrees: f64,
    /// The magnitude of the designed loop at the crossover; one by construction.
    pub open_loop_magnitude_at_crossover: f64,
    /// The declared low-frequency gain ratio of a lag; absent for a lead.
    pub low_frequency_gain_ratio: Option<f64>,
}

/// Designs a lead or a lag compensator for the crossover and the mode's value: the phase to add for a
/// lead, or the low-frequency gain ratio for a lag. It dispatches to [`design_lead`] or [`design_lag`].
///
/// # Errors
/// The errors of [`design_lead`] or [`design_lag`].
pub fn design_lead_lag(
    plant: &Transfer,
    mode: LeadLagMode,
    crossover_rad_per_second: f64,
    value: f64,
) -> Result<LeadLagDesign, ClassicalError> {
    match mode {
        LeadLagMode::Lead => design_lead(plant, crossover_rad_per_second, value),
        LeadLagMode::Lag => design_lag(plant, crossover_rad_per_second, value),
    }
}

/// Designs a lead that adds `phase_degrees` of phase at the crossover.
///
/// # Errors
/// Returns [`ClassicalError::InvalidPolynomial`] for an invalid plant,
/// [`ClassicalError::InvalidInput`] for a crossover that is not positive and finite or a phase outside
/// `(0°, 85°)`, and [`ClassicalError::NonfiniteResponse`] when the plant is singular at the crossover.
pub fn design_lead(
    plant: &Transfer,
    crossover_rad_per_second: f64,
    phase_degrees: f64,
) -> Result<LeadLagDesign, ClassicalError> {
    validate_transfer(plant)?;
    if !crossover_rad_per_second.is_finite()
        || crossover_rad_per_second <= 0.0
        || !phase_degrees.is_finite()
        || phase_degrees <= 0.0
        || phase_degrees >= MAXIMUM_LEAD_DEGREES
    {
        return Err(ClassicalError::InvalidInput);
    }
    let sine = (phase_degrees * PI / 180.0).sin();
    let alpha = (1.0 - sine) / (1.0 + sine);
    let time_constant = 1.0 / (crossover_rad_per_second * alpha.sqrt());
    let zero = 1.0 / time_constant;
    let pole = 1.0 / (alpha * time_constant);
    let plant_value = transfer_value(plant, crossover_rad_per_second);
    let lead_without_gain = complex_divide(
        ComplexValue {
            real: 1.0,
            imaginary: crossover_rad_per_second * time_constant,
        },
        ComplexValue {
            real: 1.0,
            imaginary: crossover_rad_per_second * alpha * time_constant,
        },
    );
    design(plant_value, lead_without_gain, zero, pole, None)
}

/// Designs a lag with a low-frequency gain ratio `ratio`: its zero is a tenth of the crossover and its
/// pole is the zero divided by the ratio.
///
/// # Errors
/// Returns [`ClassicalError::InvalidPolynomial`] for an invalid plant, [`ClassicalError::InvalidInput`] for
/// a crossover that is not positive and finite or a ratio that is not finite and above one, and
/// [`ClassicalError::NonfiniteResponse`] when the plant is singular at the crossover.
pub fn design_lag(
    plant: &Transfer,
    crossover_rad_per_second: f64,
    ratio: f64,
) -> Result<LeadLagDesign, ClassicalError> {
    validate_transfer(plant)?;
    if !crossover_rad_per_second.is_finite()
        || crossover_rad_per_second <= 0.0
        || !ratio.is_finite()
        || ratio <= 1.0
    {
        return Err(ClassicalError::InvalidInput);
    }
    let zero = crossover_rad_per_second / 10.0;
    let pole = zero / ratio;
    let plant_value = transfer_value(plant, crossover_rad_per_second);
    let lag_without_gain = complex_divide(
        ComplexValue {
            real: 1.0,
            imaginary: crossover_rad_per_second / zero,
        },
        ComplexValue {
            real: 1.0,
            imaginary: crossover_rad_per_second / pole,
        },
    );
    design(plant_value, lag_without_gain, zero, pole, Some(ratio))
}

/// The gain that makes the designed loop unity at the crossover, and the figures of the design. A
/// singular plant or compensator gives a gain that is not finite, which is refused.
fn design(
    plant_value: ComplexValue,
    shape: ComplexValue,
    zero: f64,
    pole: f64,
    low_frequency_gain_ratio: Option<f64>,
) -> Result<LeadLagDesign, ClassicalError> {
    let gain = 1.0 / (magnitude(plant_value) * magnitude(shape));
    let open_loop_magnitude = gain * magnitude(plant_value) * magnitude(shape);
    let phase_contribution_degrees = shape.imaginary.atan2(shape.real) * 180.0 / PI;
    if ![gain, open_loop_magnitude, phase_contribution_degrees]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err(ClassicalError::NonfiniteResponse);
    }
    Ok(LeadLagDesign {
        gain,
        zero_rad_per_second: zero,
        pole_rad_per_second: pole,
        phase_contribution_degrees,
        open_loop_magnitude_at_crossover: open_loop_magnitude,
        low_frequency_gain_ratio,
    })
}

#[cfg(test)]
#[path = "lead_lag_tests.rs"]
mod tests;
