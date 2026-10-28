//! The tuning model of a study (specification 35, B4.2 and B4.4; numerical annex N3.2, N5.1, N5.2 and N5.7): the low-order
//! model that Identify fitted, read back from the transfer function the study keeps as its selected model.
//!
//! Identify writes a low-order model as `K·e^(−L·s)/D(s)` (`identified_transfer_model` in
//! `identification::selected_model_analysis`): the numerator is the gain, and the denominator is one, the integrator `s`,
//! or the product of the factors `(τ·s + 1)` of the time constants. This module is the inverse. It reads the form, the
//! gain, the time constants and the dead time back, accepts only the denominators Identify writes, and takes the normalized
//! error that decides the model from the study's presentation of the fit. A supplied state-space model is read as its
//! transfer function (N4.7(c)), scaled to the same form (N5.13); an integrator with a lag is read too (N5.12(c)), although
//! Identify fits no such form.

#![allow(clippy::module_name_repetitions)] // `tuning_model` names the model the module reads.

use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::model::{state_space_transfer_polynomials, StateSpace};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::model::limits::MAXIMUM_CONVERSION_ORDER;
use crate::model::GeneralModel;
use crate::study::current_model::CurrentModel;
use crate::tuning::candidate_margins::plant_transfer;
use crate::tuning::structure::is_exactly_zero;

/// The relative tolerance within which a rebuilt denominator coefficient equals the stored one. A negative discriminant of
/// a second-order form within this fraction of `S²` is the rounding of a repeated time constant.
const REBUILD_TOLERANCE: f64 = 1e-9;

/// The refusal of a model that is not a transfer function of a low-order form Identify writes.
const NOT_LOW_ORDER: &str =
    "The model's structure is not a low-order form that the tuning rules take.";

/// The refusal of a model whose deciding normalized error the study does not record.
const ERROR_NOT_RECORDED: &str =
    "The model's normalized error is not recorded, so the model cannot be tuned.";

/// The tuning model of the study's current model (B3.7, B4.2): the form, the gain `K` (or the rate gain `K′` of an integrating
/// form), the time constants and the dead time of the transfer function Identify wrote or the engineer supplied. A fit's
/// normalized error is the one that decides it (N3.2), as the study's presentation records it: the error on the estimation
/// recording for a provisional fit, and the error on the independent validation recording otherwise. A supplied model was not
/// fitted, so it has no error and Tune's error check does not apply to it (B3.7). The rest of the fit evidence (the residuals,
/// the uncertainty and the search-boundary finding) is not part of the tuning model.
///
/// # Errors
/// Returns a [`Refusal`] when the model, or its transfer function after the state-space conversion (N4.7(c)), is not a
/// low-order form Identify writes, or when the study does not record the normalized error that decides a fit.
pub(crate) fn tuning_model(current: &CurrentModel) -> Result<IdentifiedModel, Refusal> {
    let (identity, numerator, denominator) = match &current.model {
        GeneralModel::TransferFunction { identity, transfer } => (
            identity,
            transfer.numerator.clone(),
            transfer.denominator.clone(),
        ),
        GeneralModel::ContinuousStateSpace {
            identity, system, ..
        } => {
            let (numerator, denominator) = converted(system)?;
            (identity, numerator, denominator)
        }
        GeneralModel::DiscreteStateSpace { .. } => return Err(Refusal::new(NOT_LOW_ORDER)),
    };
    let normalized_error = match &current.identify {
        Some(_) => Some(
            current
                .fit_error()
                .ok_or_else(|| Refusal::new(ERROR_NOT_RECORDED))?,
        ),
        None => None,
    };
    low_order_model(
        &numerator,
        &denominator,
        identity.delay_seconds,
        normalized_error,
    )
}

/// The numerator and denominator of a continuous state-space model, `C·(s·I − A)⁻¹·B + D` (N4.7(c)), scaled as Identify writes
/// its forms (N5.13): the lowest nonzero coefficient of the denominator is one. The conversion of `K/(T·s + 1)` is monic,
/// `(K/T)/(s + 1/T)`; dividing numerator and denominator by `1/T` gives `K/(T·s + 1)` again.
///
/// # Errors
/// Returns the structure refusal when the model is not single-input and single-output, or its order is above the conversion
/// limit (N4.7(c)).
fn converted(system: &StateSpace) -> Result<(Vec<f64>, Vec<f64>), Refusal> {
    let transfer = state_space_transfer_polynomials(system, MAXIMUM_CONVERSION_ORDER)
        .map_err(|_| Refusal::new(NOT_LOW_ORDER))?;
    let Some(pivot) = transfer
        .denominator
        .iter()
        .rev()
        .copied()
        .find(|value| !is_exactly_zero(*value))
    else {
        return Ok((transfer.numerator, transfer.denominator));
    };
    Ok((
        transfer
            .numerator
            .iter()
            .map(|value| value / pivot)
            .collect(),
        transfer
            .denominator
            .iter()
            .map(|value| value / pivot)
            .collect(),
    ))
}

/// The low-order model of `n·e^(−L·s)/D(s)`, with the coefficients in descending powers, in the forms Identify writes:
///
/// - `D = [d₀]`: a static gain, `K = n/d₀`;
/// - `D = [d₀, 0]`: an integrating form, with the rate gain `K′ = n/d₀`;
/// - `D = [d₀, d₁]`: a first-order form, `T = d₀/d₁` and `K = n/d₁`, with a dead time when `L > 0`;
/// - `D = [d₀, d₁, d₂]`: a second-order form, `K = n/d₂`, with a dead time when `L > 0`. Its time constants are the roots
///   of `x² − (d₁/d₂)·x + d₀/d₂`, the larger first, when they are real. Complex roots leave both absent;
/// - `D = [d₀, d₁, 0]`: an integrating form with a lag, `K′ = n/d₁` and `τ = d₀/d₁` (N5.12(c)). Identify fits no such form. A
///   supplied model may carry it, and only the pole placement of Automatic takes it.
///
/// The denominator rebuilt from the form and its time constants, as Identify builds it, must equal the stored one within a
/// relative `10⁻⁹`: every other transfer function is refused, so the lowest nonzero coefficient is one (the constant term of a
/// static, first-order or second-order form, or the `s` coefficient of an integrator, with or without its lag). A form with
/// complex roots has no time constants to rebuild it from; its coefficients over its constant term stand for the rebuilt
/// ones. The dead time is `L`, zero included. The model is stable when it integrates, or when every time constant it has is
/// finite and positive.
fn low_order_model(
    numerator: &[f64],
    denominator: &[f64],
    delay: f64,
    normalized_error: Option<f64>,
) -> Result<IdentifiedModel, Refusal> {
    let [gain_coefficient] = *numerator else {
        return Err(Refusal::new(NOT_LOW_ORDER));
    };
    let delayed = delay > 0.0;
    let (kind, divisor, time_constants) = match *denominator {
        [constant] => (LowOrderModelKind::StaticGain, constant, [None, None]),
        [rate, constant] if is_exactly_zero(constant) => {
            (LowOrderModelKind::Integrating, rate, [None, None])
        }
        [lag, constant] => {
            let kind = if delayed {
                LowOrderModelKind::FirstOrderDeadTime
            } else {
                LowOrderModelKind::FirstOrder
            };
            (kind, constant, [Some(lag / constant), None])
        }
        [quadratic, linear, constant] if !is_exactly_zero(constant) => {
            let kind = if delayed {
                LowOrderModelKind::SecondOrderDeadTime
            } else {
                LowOrderModelKind::SecondOrder
            };
            let time_constants =
                second_order_time_constants(linear / constant, quadratic / constant);
            (kind, constant, time_constants)
        }
        [lag, linear, constant] if is_exactly_zero(constant) && !is_exactly_zero(linear) => (
            LowOrderModelKind::Integrating,
            linear,
            [Some(lag / linear), None],
        ),
        _ => return Err(Refusal::new(NOT_LOW_ORDER)),
    };
    let complex_roots = denominator.len() == 3 && matches!(time_constants, [None, None]);
    let model = identified(
        kind,
        gain_coefficient / divisor,
        time_constants,
        delay,
        normalized_error,
    );
    let rebuilt: Vec<f64> = if complex_roots {
        denominator
            .iter()
            .map(|coefficient| coefficient / divisor)
            .collect()
    } else {
        plant_transfer(&model).denominator
    };
    if same_coefficients(&rebuilt, denominator) {
        Ok(model)
    } else {
        Err(Refusal::new(NOT_LOW_ORDER))
    }
}

/// The time constants `τ₁ ≥ τ₂` of `(τ₁·s + 1)(τ₂·s + 1) = P·s² + S·s + 1`: the roots of `x² − S·x + P`, or both absent
/// when the roots are complex. Identify may write a repeated time constant, whose discriminant `S² − 4·P` can round below
/// zero; a discriminant above `−10⁻⁹·S²` is therefore real, read as zero when it is negative, and the rebuilt denominator
/// decides. The root of the larger magnitude is computed first and the other as `P` over it, so neither loses digits.
fn second_order_time_constants(sum: f64, product: f64) -> [Option<f64>; 2] {
    let discriminant = sum.mul_add(sum, -4.0 * product);
    if discriminant < -REBUILD_TOLERANCE * sum * sum {
        return [None, None];
    }
    let larger = f64::midpoint(sum, discriminant.max(0.0).sqrt().copysign(sum));
    let other = product / larger;
    [Some(larger.max(other)), Some(larger.min(other))]
}

/// The tuning model of a form read back from its transfer function: the gain, the time constants and the dead time, the
/// normalized error that decides it, and its stability (N2.1): an integrating form, or a form whose time constants are all
/// finite and positive.
fn identified(
    kind: LowOrderModelKind,
    gain: f64,
    time_constants: [Option<f64>; 2],
    delay: f64,
    normalized_error: Option<f64>,
) -> IdentifiedModel {
    let [first, second] = time_constants;
    let stable = kind == LowOrderModelKind::Integrating
        || time_constants
            .iter()
            .flatten()
            .all(|time| time.is_finite() && *time > 0.0);
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: first,
        second_time_constant_seconds: second,
        dead_time_seconds: Some(delay),
        normalized_error,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable,
        on_search_boundary: false,
    }
}

/// Whether the rebuilt coefficients are the stored ones: as many, and each pair finite and within [`REBUILD_TOLERANCE`] of
/// the larger magnitude of the two.
fn same_coefficients(rebuilt: &[f64], stored: &[f64]) -> bool {
    rebuilt.len() == stored.len()
        && rebuilt.iter().zip(stored).all(|(left, right)| {
            left.is_finite()
                && right.is_finite()
                && (left - right).abs() <= REBUILD_TOLERANCE * left.abs().max(right.abs())
        })
}

#[cfg(test)]
#[path = "model_tests.rs"]
pub(crate) mod tests;
