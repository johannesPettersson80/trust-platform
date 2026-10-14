//! The pole-zero map of a model (`poleZero`, B9.9; draft §2.3). The unreduced view is the model as declared, with every internal mode.
//! The minimal view removes the exact common factor of a single-input, single-output model, using exact rational arithmetic (N7.4).
//! Reduction is a view only: the selected model stays as it is for validation, so no result of this tool changes it.

use trust_control_math::classical::roots_of_real_polynomial;
use trust_control_math::exact_polynomial::{Rational, RationalPolynomial};
use trust_control_math::model::state_space_transfer_polynomials;

use crate::analysis::frequency::model_poles_and_zeros;
use crate::analysis::tool::input_failure;
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{
    AnalysisInputs, PoleDomain, PoleZeroAnswer, PoleZeroView, TransferPolynomials,
};
use crate::model::limits::MAXIMUM_CONVERSION_ORDER;
use crate::model::refusal::conversion_refusal;
use crate::model::GeneralModel;

/// The domain sentence of a continuous model (the page's words, `threePageAnalysis.ts`).
pub const CONTINUOUS_DOMAIN: &str =
    "Continuous s-plane; negative real parts are the local model stability direction.";
/// The domain sentence of a discrete model.
pub const DISCRETE_DOMAIN: &str = "Discrete z-plane; the unit circle is the stability boundary.";
/// The warning when a cancelled mode lies in the unstable region (B9.9).
pub const HIDDEN_MODE: &str = "An unstable or boundary internal mode is hidden.";
/// The note of a view that removes a common factor.
pub const REMOVED: &str = "Exact common factors are removed only from the input/output view; the original model remains selected for validation.";
/// The note of a model with no exact common factor.
pub const NO_COMMON_FACTOR: &str = "No exact common factors in the declared coefficient representation; near cancellations remain unreduced.";

/// The answer of `poleZero`: the poles and zeros of the view, with the transfer polynomials and the cancelled factor of a
/// single-input, single-output model. A multi-channel model's poles are the eigenvalues of its state matrix, with no zeros, and its
/// only view is the unreduced one.
///
/// # Errors
/// Returns the input failure of a minimal view that a multi-channel model asks for, and the failure of a conversion or a root
/// calculation that refuses the model.
pub fn pole_zero_answer(
    model: &GeneralModel,
    inputs: &AnalysisInputs,
) -> Result<PoleZeroAnswer, Failure> {
    let view = view_of(model, inputs)?;
    let discrete = matches!(model, GeneralModel::DiscreteStateSpace { .. });
    let domain = if discrete {
        PoleDomain::Z
    } else {
        PoleDomain::S
    };
    let domain_sentence = if discrete {
        DISCRETE_DOMAIN
    } else {
        CONTINUOUS_DOMAIN
    };
    let identity = model.identity();
    if identity.inputs.len() != 1 || identity.outputs.len() != 1 {
        let (poles, zeros) = model_poles_and_zeros(model)?;
        return Ok(PoleZeroAnswer {
            domain,
            view: PoleZeroView::Unreduced,
            poles,
            zeros,
            hidden_unstable_mode: false,
            unreduced: None,
            minimal: None,
            cancelled_factor: None,
            summary: vec![domain_sentence.to_owned()],
            details: Vec::new(),
        });
    }
    let (numerator, denominator) = transfer_of(model)?;
    let exact_numerator = exact(&numerator)?;
    let exact_denominator = exact(&denominator)?;
    let common = monic(
        &RationalPolynomial::gcd(&exact_numerator, &exact_denominator)
            .map_err(|_| exact_failure())?,
    )?;
    let minimal_numerator = exact_quotient(&exact_numerator, &common)?;
    let minimal_denominator = exact_quotient(&exact_denominator, &common)?;
    let cancelled_factor = floats(&common);
    let minimal_numerator = floats(&minimal_numerator);
    let minimal_denominator = floats(&minimal_denominator);
    let (view_numerator, view_denominator) = match view {
        PoleZeroView::Unreduced => (&numerator, &denominator),
        PoleZeroView::Minimal => (&minimal_numerator, &minimal_denominator),
    };
    let poles = roots_of_real_polynomial(view_denominator).ok_or_else(roots_failure)?;
    let zeros = if view_numerator.len() <= 1 {
        Vec::new()
    } else {
        roots_of_real_polynomial(view_numerator).ok_or_else(roots_failure)?
    };
    let cancelled_roots = if cancelled_factor.len() <= 1 {
        Vec::new()
    } else {
        roots_of_real_polynomial(&cancelled_factor).ok_or_else(roots_failure)?
    };
    let hidden_unstable_mode = cancelled_roots.iter().any(|root| {
        if discrete {
            root.real.hypot(root.imaginary) >= 1.0
        } else {
            root.real >= 0.0
        }
    });
    let mut summary = Vec::new();
    if cancelled_factor.len() > 1 {
        if hidden_unstable_mode {
            summary.push(HIDDEN_MODE.to_owned());
        }
        summary.push(REMOVED.to_owned());
    } else {
        summary.push(NO_COMMON_FACTOR.to_owned());
    }
    summary.push(domain_sentence.to_owned());
    Ok(PoleZeroAnswer {
        domain,
        view,
        poles,
        zeros,
        hidden_unstable_mode,
        unreduced: Some(TransferPolynomials {
            numerator: numerator.clone(),
            denominator: denominator.clone(),
        }),
        minimal: Some(TransferPolynomials {
            numerator: minimal_numerator,
            denominator: minimal_denominator,
        }),
        cancelled_factor: Some(cancelled_factor),
        summary,
        details: Vec::new(),
    })
}

/// The view that is asked for. The minimal view needs one input and one output (B9.9).
///
/// # Errors
/// Returns the input failure that names the view, for a minimal view of a multi-channel model.
pub fn view_of(model: &GeneralModel, inputs: &AnalysisInputs) -> Result<PoleZeroView, Failure> {
    let view = inputs.view.unwrap_or(PoleZeroView::Unreduced);
    let identity = model.identity();
    if view == PoleZeroView::Minimal && (identity.inputs.len() != 1 || identity.outputs.len() != 1)
    {
        return Err(input_failure(
            "The minimal view needs one input and one output: the view is unreduced for this model.",
        ));
    }
    Ok(view)
}

/// The numerator and denominator of a single-input, single-output model as a transfer function: the model itself, or the exact
/// polynomials of its state matrix in the plane of the model (`stateSpaceTransferPolynomials`). A discrete model keeps its
/// z-plane polynomials, so its poles are the z-plane poles of the model and not the poles of a held-input equivalent.
fn transfer_of(model: &GeneralModel) -> Result<(Vec<f64>, Vec<f64>), Failure> {
    let system = match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            return Ok((transfer.numerator.clone(), transfer.denominator.clone()));
        }
        GeneralModel::ContinuousStateSpace { system, .. }
        | GeneralModel::DiscreteStateSpace { system, .. } => system,
    };
    let transfer =
        state_space_transfer_polynomials(system, MAXIMUM_CONVERSION_ORDER).map_err(|error| {
            Failure::new(
                &conversion_refusal(error).to_string(),
                Cause::ModelOrData,
                NextAction::None,
            )
        })?;
    Ok((transfer.numerator, transfer.denominator))
}

/// The exact polynomial of decimal coefficients (N7.4).
fn exact(coefficients: &[f64]) -> Result<RationalPolynomial, Failure> {
    RationalPolynomial::from_decimals(coefficients).map_err(|_| exact_failure())
}

/// The exact quotient of `polynomial` by `divisor`, which divides it exactly.
fn exact_quotient(
    polynomial: &RationalPolynomial,
    divisor: &RationalPolynomial,
) -> Result<RationalPolynomial, Failure> {
    polynomial
        .exact_divide(divisor)
        .map_err(|_| exact_failure())?
        .ok_or_else(exact_failure)
}

/// The polynomial divided by its leading coefficient, so that its first coefficient is one.
fn monic(polynomial: &RationalPolynomial) -> Result<RationalPolynomial, Failure> {
    let coefficients = polynomial.coefficients();
    let Some(leading) = coefficients.first() else {
        return Ok(polynomial.clone());
    };
    let scaled = coefficients
        .iter()
        .map(|coefficient| coefficient.div(leading))
        .collect::<Result<Vec<Rational>, _>>()
        .map_err(|_| exact_failure())?;
    RationalPolynomial::new(scaled).map_err(|_| exact_failure())
}

/// The coefficients of an exact polynomial as numbers, from the highest power down.
fn floats(polynomial: &RationalPolynomial) -> Vec<f64> {
    polynomial
        .coefficients()
        .iter()
        .map(Rational::to_f64)
        .collect()
}

fn exact_failure() -> Failure {
    Failure::new(
        "The polynomial's exact coefficients could not be formed.",
        Cause::ModelOrData,
        NextAction::None,
    )
}

fn roots_failure() -> Failure {
    Failure::new(
        "Cancelled factors exceed the supported pole calculation.",
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "pole_view_tests.rs"]
mod tests;
