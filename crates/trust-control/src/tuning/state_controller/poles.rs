//! The closed-loop poles of mapped coefficients (N7.11; specification 35, C4). The poles are read back from the coefficient
//! values a program holds, because REAL rounding and a manual pin change them: the controller's poles are the eigenvalues
//! of `A − B·K`, and an LQG law adds those of the estimator, `(I − L·C)·A`. A law whose values are not strictly stable in
//! the sampled model is refused.

use std::collections::BTreeMap;

use trust_control_math::linalg::{eigenvalues, ComplexValue, DenseMatrix};

use super::binding::{decode_state_controller_binding, StateControllerBinding};
use super::sampling::{sample_model, SampledModel};
use super::{texts, StateControllerFamily, DESIGN_TOLERANCE};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;

/// The closed-loop poles of the law that `loop_definition` declares, on `model` at the loop's task period, with the
/// coefficient values in `values`.
///
/// # Errors
/// Returns a failure when the law is not declared or the model is not a state-space model, when the coefficients do not
/// match the model, when a pole cannot be computed, or when a pole is not strictly inside the unit circle.
pub fn state_controller_poles(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<Vec<ComplexValue>, Failure> {
    let binding = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| {
            decode_state_controller_binding(value, loop_definition.parameter_binding.as_ref())
        })
        .ok_or_else(|| {
            Failure::new(
                texts::COORDINATES_UNAVAILABLE,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let sampled = sample_model(model, loop_definition.controller.task_period_ms / 1000.0)?;
    closed_loop_poles(&binding, &sampled, values)
}

/// The closed-loop poles of `binding` on the sampled model, with the coefficient values in `values`: the controller's,
/// and for LQG the estimator's. Every pole must be strictly inside the unit circle.
pub(super) fn closed_loop_poles(
    binding: &StateControllerBinding,
    sampled: &SampledModel,
    values: &BTreeMap<String, f64>,
) -> Result<Vec<ComplexValue>, Failure> {
    let mut poles = controller_poles(binding, sampled, values)?;
    if binding.family == StateControllerFamily::Lqg {
        poles.extend(observer_poles(binding, sampled, values)?);
    }
    if poles
        .iter()
        .any(|pole| pole.real.hypot(pole.imaginary) >= 1.0)
    {
        return Err(Failure::new(
            texts::NOT_STRICTLY_STABLE,
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(poles)
}

/// The eigenvalues of `A − B·K` for the gain named by the declaration and valued by `values`.
fn controller_poles(
    binding: &StateControllerBinding,
    sampled: &SampledModel,
    values: &BTreeMap<String, f64>,
) -> Result<Vec<ComplexValue>, Failure> {
    let system = &sampled.system;
    let (states, inputs) = (system.states(), system.inputs());
    let gain = coefficient_matrix(&binding.gain_symbols, values)
        .filter(|gain| gain.len() == inputs && gain.iter().all(|row| row.len() == states))
        .ok_or_else(|| {
            Failure::new(
                texts::GAIN_COORDINATES,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let closed_loop: DenseMatrix = (0..states)
        .map(|row| {
            (0..states)
                .map(|column| {
                    let feedback: f64 = (0..inputs)
                        .map(|input| system.b[row][input] * gain[input][column])
                        .sum();
                    system.a[row][column] - feedback
                })
                .collect()
        })
        .collect();
    let spectrum = eigenvalues(&closed_loop, &DESIGN_TOLERANCE).map_err(|_| {
        Failure::new(
            texts::POLE_CALCULATION_FAILED,
            Cause::Unclassified,
            NextAction::None,
        )
    })?;
    Ok(spectrum.values)
}

/// The eigenvalues of the estimator `(I − L·C)·A` for the observer gain named by the declaration and valued by `values`.
fn observer_poles(
    binding: &StateControllerBinding,
    sampled: &SampledModel,
    values: &BTreeMap<String, f64>,
) -> Result<Vec<ComplexValue>, Failure> {
    let system = &sampled.system;
    let (states, outputs) = (system.states(), system.outputs());
    let symbols = binding.observer_gain_symbols.as_deref().ok_or_else(|| {
        Failure::new(
            texts::OBSERVER_COORDINATES,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })?;
    let observer = coefficient_matrix(symbols, values)
        .filter(|observer| {
            observer.len() == states && observer.iter().all(|row| row.len() == outputs)
        })
        .ok_or_else(|| {
            Failure::new(
                texts::OBSERVER_COORDINATES,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let correction: DenseMatrix = (0..states)
        .map(|row| {
            (0..states)
                .map(|column| {
                    let injected: f64 = (0..outputs)
                        .map(|output| observer[row][output] * system.c[output][column])
                        .sum();
                    f64::from(u8::from(row == column)) - injected
                })
                .collect()
        })
        .collect();
    let estimator: DenseMatrix = correction
        .iter()
        .map(|row| {
            (0..states)
                .map(|column| {
                    row.iter()
                        .enumerate()
                        .map(|(k, value)| value * system.a[k][column])
                        .sum::<f64>()
                })
                .collect()
        })
        .collect();
    let spectrum = eigenvalues(&estimator, &DESIGN_TOLERANCE).map_err(|_| {
        Failure::new(
            texts::OBSERVER_POLE_FAILED,
            Cause::Unclassified,
            NextAction::None,
        )
    })?;
    Ok(spectrum.values)
}

/// The rows of coefficient values named by `symbols`, or `None` when a symbol has no finite value in `values`.
fn coefficient_matrix(
    symbols: &[Vec<String>],
    values: &BTreeMap<String, f64>,
) -> Option<DenseMatrix> {
    symbols
        .iter()
        .map(|row| {
            row.iter()
                .map(|symbol| {
                    values
                        .get(symbol)
                        .copied()
                        .filter(|value| value.is_finite())
                })
                .collect::<Option<Vec<f64>>>()
        })
        .collect()
}
