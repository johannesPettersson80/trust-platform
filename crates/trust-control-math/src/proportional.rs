//! Proportional control of a first-order or integrating plant: the closed-loop pole, the steady-error and
//! transient bounds that Tune predicts for a proportional gain (N5.8, N5.9), the compatible integrating gain
//! that meets the error and settling allowances (N5.8), and the steady error of the proportional-derivative
//! alternative (N5.9).
//!
//! The predictions take signed gains, because the sign decides whether the loop is stable (N6). The
//! compatible gain returns a magnitude, and the caller gives it the sign of its action.

#![allow(clippy::module_name_repetitions)] // `ProportionalRefusal` names the module's subject.

/// The signed quantities of a proportional prediction on a delay-free plant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictionInputs {
    /// The plant gain: the rate gain `k` of `k/s` (N5.8), or the static gain `K` of `K/(τs + 1)`
    /// (N5.9). Signed (N6).
    pub plant_gain: f64,
    /// The proportional gain `Kp`, signed (N6).
    pub proportional_gain: f64,
    /// The operating-point input `u0`.
    pub operating_input: f64,
    /// The constant input disturbance `d`.
    pub input_disturbance: f64,
    /// The setpoint change `ΔR`.
    pub reference_change: f64,
    /// The settling time `Tset` in seconds.
    pub settling_time_seconds: f64,
}

/// The prediction of a delay-free integrator `k/s` under proportional control (N5.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntegratingPrediction {
    /// The closed-loop pole `−k·Kp`. Zero when `k·Kp` is zero, and positive for an unstable loop.
    pub closed_loop_pole: f64,
    /// The constant error bound `E∞ = max(|u0|, |u0 − d|)/|Kp|`, when the loop is stable.
    pub steady_error_bound: Option<f64>,
    /// The transient envelope `(|ΔR| + E∞)·exp(−λ·Tset)` at the settling time, when the loop is stable.
    pub settling_residual_bound: Option<f64>,
}

/// The prediction of a delay-free first-order plant `K/(τs + 1)` under proportional control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstOrderPrediction {
    /// The closed-loop pole `−(1 + K·Kp)/τ`. Zero when the closed-loop factor is zero.
    pub closed_loop_pole: f64,
    /// The steady-error bound `max(|ΔR + K·u0|, |K·d|)/(1 + K·Kp)`, when the pole is negative.
    pub steady_error_bound: Option<f64>,
    /// The reference transient bound `max(|ΔR|, |q/(1 + q)·ΔR − K·u0/(1 + q)|)·exp(pole·Tset)`, with
    /// `q = K·Kp`, when the pole is negative.
    pub settling_residual_bound: Option<f64>,
}

/// The task of a compatible integrating gain (N5.8): the rate gain and operating point of the plant,
/// the disturbance and the setpoint change, the error and settling allowances, and the bounds of
/// the gain magnitude `|Kp|`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntegratingTask {
    /// The rate gain `k` of `k/s`.
    pub rate_gain: f64,
    /// The operating-point input `u0`.
    pub operating_input: f64,
    /// The constant input disturbance `d`.
    pub input_disturbance: f64,
    /// The setpoint change `ΔR`.
    pub reference_change: f64,
    /// The declared steady-error allowance, in output units.
    pub maximum_steady_error: f64,
    /// The settling band `b`, in output units.
    pub settling_band: f64,
    /// The settling time `Tset` in seconds.
    pub settling_time_seconds: f64,
    /// The smallest allowed gain magnitude `|Kp|`.
    pub minimum_gain: f64,
    /// The largest allowed gain magnitude `|Kp|`.
    pub maximum_gain: f64,
}

/// Why a proportional calculation is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProportionalRefusal {
    /// An input is not finite, or a limit is outside its domain.
    #[error("an input is not finite or a limit is outside its domain")]
    InvalidInput,
    /// A result is not finite, or the closed loop is too close to zero to classify (underflow).
    #[error("the prediction has numerical uncertainty")]
    NumericalUncertainty,
    /// The closed-loop DC denominator `1 + K·Kp` is not positive (N5.9).
    #[error("the closed-loop DC denominator is not positive")]
    NonpositiveDcDenominator,
    /// No gain within the bounds meets the error and settling allowances (N5.8).
    #[error("no gain within the bounds meets the error and settling allowances")]
    NoCompatibleGain,
}

/// The prediction of a delay-free integrator `k/s` under proportional control `Kp` (N5.8). The
/// closed-loop pole is `−k·Kp`, and a stable loop (`k·Kp > 0`) has the constant error bound
/// `E∞ = max(|u0|, |u0 − d|)/|Kp|` and the transient envelope `(|ΔR| + E∞)·exp(−λ·Tset)` with
/// `λ = k·Kp`. Otherwise no asymptotic bound is established.
///
/// # Errors
/// Returns [`ProportionalRefusal::InvalidInput`] for a nonfinite input,
/// [`ProportionalRefusal::NumericalUncertainty`] for a zero rate gain, a result that is not finite,
/// or a product that underflows to zero while `Kp` is nonzero.
pub fn integrating_prediction(
    inputs: PredictionInputs,
) -> Result<IntegratingPrediction, ProportionalRefusal> {
    validate(inputs)?;
    let decay = inputs.plant_gain * inputs.proportional_gain;
    if inputs.plant_gain == 0.0
        || !decay.is_finite()
        || (decay == 0.0 && inputs.proportional_gain != 0.0)
    {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    let pole = if decay == 0.0 { 0.0 } else { -decay };
    if decay <= 0.0 {
        return Ok(IntegratingPrediction {
            closed_loop_pole: pole,
            steady_error_bound: None,
            settling_residual_bound: None,
        });
    }
    let operating = inputs.operating_input;
    let load = operating
        .abs()
        .max((operating - inputs.input_disturbance).abs());
    let steady_error_bound = load / inputs.proportional_gain.abs();
    let settling_residual_bound = (inputs.reference_change.abs() + steady_error_bound)
        * (pole * inputs.settling_time_seconds).exp();
    if !steady_error_bound.is_finite() || !settling_residual_bound.is_finite() {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    Ok(IntegratingPrediction {
        closed_loop_pole: pole,
        steady_error_bound: Some(steady_error_bound),
        settling_residual_bound: Some(settling_residual_bound),
    })
}

/// The prediction of a delay-free first-order plant `K/(τs + 1)` under proportional control `Kp`.
/// With `q = K·Kp`, the closed-loop pole is `−(1 + q)/τ`. A negative pole gives the steady-error
/// bound `max(|ΔR + K·u0|, |K·d|)/(1 + q)` and the reference transient bound
/// `max(|ΔR|, |q/(1 + q)·ΔR − K·u0/(1 + q)|)·exp(pole·Tset)`. A pole that is not negative gives no
/// asymptotic bound.
///
/// # Errors
/// Returns [`ProportionalRefusal::InvalidInput`] for a nonfinite input or a time constant that is not
/// positive, and [`ProportionalRefusal::NumericalUncertainty`] for a result that is not finite (an
/// operating or load effect that overflows included, whatever the pole) or a pole that underflows to
/// zero while the closed-loop factor is nonzero.
pub fn first_order_prediction(
    inputs: PredictionInputs,
    time_constant_seconds: f64,
) -> Result<FirstOrderPrediction, ProportionalRefusal> {
    validate(inputs)?;
    if !(time_constant_seconds.is_finite() && time_constant_seconds > 0.0) {
        return Err(ProportionalRefusal::InvalidInput);
    }
    let gain = inputs.plant_gain;
    let q = gain * inputs.proportional_gain;
    let factor = 1.0 + q;
    let raw_pole = -factor / time_constant_seconds;
    if !raw_pole.is_finite() || !factor.is_finite() || (raw_pole == 0.0 && factor != 0.0) {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    // The operating and load effects are checked before the pole is classified: an effect that overflows is numerical
    // uncertainty, whatever the pole (contract reference §9.2).
    let operating_effect = gain * inputs.operating_input;
    let worst_final_error = (inputs.reference_change + operating_effect)
        .abs()
        .max((gain * inputs.input_disturbance).abs());
    if !worst_final_error.is_finite() {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    let pole = if raw_pole == 0.0 { 0.0 } else { raw_pole };
    if pole >= 0.0 {
        return Ok(FirstOrderPrediction {
            closed_loop_pole: pole,
            steady_error_bound: None,
            settling_residual_bound: None,
        });
    }
    let transient_amplitude = inputs
        .reference_change
        .abs()
        .max((q / factor * inputs.reference_change - operating_effect / factor).abs());
    let steady_error_bound = worst_final_error / factor;
    let settling_residual_bound = transient_amplitude * (pole * inputs.settling_time_seconds).exp();
    if !steady_error_bound.is_finite() || !settling_residual_bound.is_finite() {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    Ok(FirstOrderPrediction {
        closed_loop_pole: pole,
        steady_error_bound: Some(steady_error_bound),
        settling_residual_bound: Some(settling_residual_bound),
    })
}

/// The magnitude `|Kp|` of the compatible gain of an integrating plant (N5.8). With `λ = |k|·|Kp|`, the
/// constant error `E∞ = load/|Kp|` (for `load = max(|u0|, |u0 − d|)`) meets the allowance when
/// `λ ≥ |k|·load/E_max`, and the envelope `(|ΔR| + E_max)·e^(−λ·Tset)` reaches the band when
/// `λ ≥ ln((|ΔR| + E_max)/b)/Tset`. The smallest `λ` that meets both, and the gain bounds, is chosen.
/// The result is a magnitude, and the caller gives it the sign of its action (N6).
///
/// # Errors
/// Returns [`ProportionalRefusal::InvalidInput`] for a nonfinite input, a zero rate gain, a
/// nonpositive allowance, band or settling time, or bounds that are negative or out of order. Returns
/// [`ProportionalRefusal::NoCompatibleGain`] when the smallest compatible `λ` is above the largest bound.
pub fn compatible_integrating_gain(task: IntegratingTask) -> Result<f64, ProportionalRefusal> {
    let values = [
        task.rate_gain,
        task.operating_input,
        task.input_disturbance,
        task.reference_change,
        task.maximum_steady_error,
        task.settling_band,
        task.settling_time_seconds,
        task.minimum_gain,
        task.maximum_gain,
    ];
    let valid = values.iter().all(|number| number.is_finite())
        && task.rate_gain != 0.0
        && task.maximum_steady_error > 0.0
        && task.settling_band > 0.0
        && task.settling_time_seconds > 0.0
        && task.minimum_gain >= 0.0
        && task.maximum_gain >= task.minimum_gain;
    if !valid {
        return Err(ProportionalRefusal::InvalidInput);
    }
    let rate = task.rate_gain.abs();
    let operating = task.operating_input;
    let load = operating
        .abs()
        .max((operating - task.input_disturbance).abs());
    let transient_bound = task.reference_change.abs() + task.maximum_steady_error;
    let from_error = rate * load / task.maximum_steady_error;
    let from_settling = (transient_bound / task.settling_band).ln() / task.settling_time_seconds;
    let lowest = rate * task.minimum_gain;
    let highest = rate * task.maximum_gain;
    let decay = from_error.max(from_settling).max(lowest).max(f64::EPSILON);
    if !decay.is_finite() || decay > highest {
        return Err(ProportionalRefusal::NoCompatibleGain);
    }
    Ok(decay / rate)
}

/// The steady error of the proportional-derivative alternative of a stable first-order model (N5.9):
/// `|ΔR + K·u0| / (1 + K·Kp)`, with the signed gain `K` and the signed `Kp`. For a loop consistent with
/// its action (`K·Kp > 0`) the denominator is `1 + |K|·|Kp|`. A nonpositive closed-loop DC denominator
/// is refused.
///
/// # Errors
/// Returns [`ProportionalRefusal::NumericalUncertainty`] when a value is not finite,
/// [`ProportionalRefusal::NonpositiveDcDenominator`] when `1 + K·Kp ≤ 0`.
pub fn proportional_steady_error(
    plant_gain: f64,
    proportional_gain: f64,
    operating_input: f64,
    reference_change: f64,
) -> Result<f64, ProportionalRefusal> {
    let offset = reference_change + plant_gain * operating_input;
    let denominator = 1.0 + plant_gain * proportional_gain;
    if !(offset.is_finite() && denominator.is_finite()) {
        return Err(ProportionalRefusal::NumericalUncertainty);
    }
    if denominator <= 0.0 {
        return Err(ProportionalRefusal::NonpositiveDcDenominator);
    }
    Ok(offset.abs() / denominator)
}

/// The smallest proportional magnitude `|Kp|` whose steady error (N5.9) does not exceed the allowance,
/// for an offset `|ΔR + K·u0|` and the plant gain magnitude `|K|`: `max(0, offset/allowance − 1)/|K|`.
///
/// # Errors
/// Returns [`ProportionalRefusal::InvalidInput`] unless the offset is finite and nonnegative, the
/// allowance is positive, and the gain magnitude is positive and finite.
pub fn minimum_proportional_magnitude(
    offset: f64,
    allowance: f64,
    plant_gain_magnitude: f64,
) -> Result<f64, ProportionalRefusal> {
    let valid = offset.is_finite()
        && offset >= 0.0
        && allowance.is_finite()
        && allowance > 0.0
        && plant_gain_magnitude.is_finite()
        && plant_gain_magnitude > 0.0;
    if !valid {
        return Err(ProportionalRefusal::InvalidInput);
    }
    Ok((offset / allowance - 1.0).max(0.0) / plant_gain_magnitude)
}

fn validate(inputs: PredictionInputs) -> Result<(), ProportionalRefusal> {
    let values = [
        inputs.plant_gain,
        inputs.proportional_gain,
        inputs.operating_input,
        inputs.input_disturbance,
        inputs.reference_change,
        inputs.settling_time_seconds,
    ];
    if values.iter().all(|number| number.is_finite()) {
        Ok(())
    } else {
        Err(ProportionalRefusal::InvalidInput)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ported_tests;
