//! The initialization formulas of the declared states (specification 35, C0 "Controller state initialization"; annex E6.4
//! `initialization_formulas`): where a state's value comes from at the task boundary, so that the transfer reads it fresh
//! when it starts rather than from an older snapshot. Ported from `controllerInitializationFormulas.ts`. The kinds
//! `notch-history` (the notch of a program's own law, C5), `scaled-measurement` (the measured-disturbance feedforward, C8,
//! which needs a second model input) and `decoupled-output` (the static decoupler, C7) are parked: they are not ported, and
//! `ControllerInitialization::new` refuses the declarations that use them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::channel_mapping::{connected_model_channels, ConnectedChannels};
use super::initial_state::{parked_source_word, ControllerInitialization};
use super::texts;
use super::transfer::{TransferBinding, TransferValue};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{InitialSource, InitialValue, InitializedState, LoopDefinition};
use crate::model::GeneralModel;

/// Where a declared state's value comes from at the start of a transfer (annex E6.4 `initialization_formulas`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InitializationFormula {
    /// `pid-integral`: the output less `Kp·(β − 1)·PV`, so that the integral holds the operating command.
    PidIntegral {
        /// The actuator's command, read fresh.
        output_symbol: String,
        /// The process value, read fresh.
        process_variable: String,
        /// The proportional gain, a guard.
        gain_symbol: String,
        /// The setpoint weight β, when the record holds it as a guard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight_symbol: Option<String>,
        /// The setpoint weight β as a fixed number; 1 when neither is given.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight: Option<f64>,
    },
    /// `weighted-error`: `(β − 1)·PV`, the proportional history of a weighted error.
    WeightedError {
        /// The process value, read fresh.
        process_variable: String,
        /// The setpoint weight β, when the record holds it as a guard.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight_symbol: Option<String>,
        /// The setpoint weight β as a fixed number; 0 when neither is given.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        weight: Option<f64>,
    },
    /// `model-input`: the plant input, read fresh.
    ModelInput {
        /// The actuator's command, read fresh.
        output_symbol: String,
    },
    /// `model-output`: the measured output, read fresh.
    ModelOutput {
        /// The process value, read fresh.
        process_variable: String,
    },
    /// `model-input-is`: 1 when the plant input, read fresh, is the predicate's level, else 0.
    ModelInputIs {
        /// The actuator's command, read fresh.
        output_symbol: String,
        /// The predicate's level, a level of the actuator.
        value: f64,
    },
}

/// The fresh source of each declared state that has one, keyed by state symbol (`initializationFormulas`). A state with a
/// constant initial value has none, and so has a state that the model does not give a source to.
///
/// # Errors
/// The loop's plant connection does not fit the model (see [`connected_model_channels`]), a source's coordinate has no bound
/// input or output, or a Boolean predicate is not a level of the actuator.
pub fn initialization_formulas(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    definition: &ControllerInitialization,
) -> Result<BTreeMap<String, InitializationFormula>, Failure> {
    let channels = connected_model_channels(loop_definition, model)?;
    let mut formulas = BTreeMap::new();
    for state in definition.states() {
        if let Some(formula) = formula_of(&channels, state)? {
            formulas.insert(state.symbol.clone(), formula);
        }
    }
    Ok(formulas)
}

/// The value of a formula at the start of a transfer, read from a snapshot of the runtime's fresh values
/// (`evaluateInitializationFormula`, C0, E6.4). An output is read at its feedback guard when the binding tracks one.
///
/// # Errors
/// A value that the formula needs is missing from the snapshot or is not finite.
pub fn evaluate_initialization_formula(
    binding: &TransferBinding,
    formula: &InitializationFormula,
    snapshot: &BTreeMap<String, TransferValue>,
) -> Result<f64, Failure> {
    match formula {
        InitializationFormula::ModelInput { output_symbol } => {
            fresh_output(binding, snapshot, output_symbol)
        }
        InitializationFormula::ModelInputIs {
            output_symbol,
            value,
        } => Ok(boolean_level(
            fresh_output(binding, snapshot, output_symbol)?,
            *value,
        )),
        InitializationFormula::ModelOutput { process_variable } => {
            snapshot_value(snapshot, process_variable)
        }
        InitializationFormula::PidIntegral {
            output_symbol,
            process_variable,
            gain_symbol,
            weight_symbol,
            weight,
        } => {
            let command = fresh_output(binding, snapshot, output_symbol)?;
            let gain = snapshot_value(snapshot, gain_symbol)?;
            let weight = snapshot_weight(snapshot, weight_symbol.as_deref(), *weight, 1.0)?;
            let process = snapshot_value(snapshot, process_variable)?;
            Ok(command - gain * (weight - 1.0) * process)
        }
        InitializationFormula::WeightedError {
            process_variable,
            weight_symbol,
            weight,
        } => {
            let weight = snapshot_weight(snapshot, weight_symbol.as_deref(), *weight, 0.0)?;
            let process = snapshot_value(snapshot, process_variable)?;
            Ok((weight - 1.0) * process)
        }
    }
}

/// The formula of one declared state, or `None` for a constant (`initializationFormulas`, per state).
fn formula_of(
    channels: &ConnectedChannels<'_>,
    state: &InitializedState,
) -> Result<Option<InitializationFormula>, Failure> {
    let InitialValue::From(source) = &state.initial else {
        return Ok(None);
    };
    if let Some(word) = parked_source_word(&state.initial) {
        return Err(Failure::new(
            &texts::init_parked_source(&state.symbol, word),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    let formula = match source {
        InitialSource::ModelInput { index } => InitializationFormula::ModelInput {
            output_symbol: input_symbol(channels, state, *index)?,
        },
        InitialSource::ModelInputIs { index, value } => {
            let output_symbol = input_symbol(channels, state, *index)?;
            let range = channels.input.range;
            if !value.is_finite() || *value < range.minimum || *value > range.maximum {
                return Err(Failure::new(
                    &texts::formula_predicate_not_level(&state.symbol),
                    Cause::Settings,
                    NextAction::OpenLoopSetup,
                ));
            }
            InitializationFormula::ModelInputIs {
                output_symbol,
                value: *value,
            }
        }
        InitialSource::ModelOutput { index } => InitializationFormula::ModelOutput {
            process_variable: process_symbol(channels, state, *index)?,
        },
        InitialSource::PidIntegral {
            index,
            input_index,
            gain_symbol,
            weight_symbol,
            weight,
        } => {
            let process_variable = process_symbol(channels, state, *index)?;
            let output_symbol = input_symbol(channels, state, *input_index)?;
            InitializationFormula::PidIntegral {
                output_symbol,
                process_variable,
                gain_symbol: gain_symbol.clone(),
                weight_symbol: weight_symbol.clone(),
                weight: *weight,
            }
        }
        InitialSource::WeightedError {
            index,
            weight_symbol,
            weight,
        } => InitializationFormula::WeightedError {
            process_variable: process_symbol(channels, state, *index)?,
            weight_symbol: weight_symbol.clone(),
            weight: *weight,
        },
        InitialSource::ModelState { .. }
        | InitialSource::ScaledModelInput { .. }
        | InitialSource::DecoupledModelInput { .. }
        | InitialSource::NotchHistory { .. } => {
            return Err(Failure::new(
                &texts::init_parked_source(&state.symbol, "a parked kind"),
                Cause::ControllerCapability,
                NextAction::None,
            ))
        }
    };
    Ok(Some(formula))
}

/// The fresh value of an actuator command: the snapshot's value at its feedback guard when the binding tracks one
/// (`freshOutput`).
fn fresh_output(
    binding: &TransferBinding,
    snapshot: &BTreeMap<String, TransferValue>,
    symbol: &str,
) -> Result<f64, Failure> {
    let guard = binding
        .tracking_outputs
        .get(symbol)
        .map_or(symbol, String::as_str);
    snapshot_value(snapshot, guard)
}

/// The actuator's symbol for a plant input index: the loop's single input, index 0 (`inputs[index]?.signal.symbol`).
fn input_symbol(
    channels: &ConnectedChannels<'_>,
    state: &InitializedState,
    index: usize,
) -> Result<String, Failure> {
    if index != 0 || channels.input.symbol.is_empty() {
        return Err(Failure::new(
            &texts::formula_no_bound_actuator(&state.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(channels.input.symbol.clone())
}

/// The process value's symbol for a measured output index: the loop's single output, index 0 (`outputs[index]?.signal.symbol`).
fn process_symbol(
    channels: &ConnectedChannels<'_>,
    state: &InitializedState,
    index: usize,
) -> Result<String, Failure> {
    if index != 0 || channels.output.symbol.is_empty() {
        return Err(Failure::new(
            &texts::formula_no_bound_process(&state.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(channels.output.symbol.clone())
}

/// A value read from the snapshot, as a finite number. A BOOL reads as 0 or 1 (`Number(initial[symbol])`).
fn snapshot_value(
    snapshot: &BTreeMap<String, TransferValue>,
    symbol: &str,
) -> Result<f64, Failure> {
    let number = match snapshot.get(symbol) {
        Some(TransferValue::Number(number)) => Some(*number),
        Some(TransferValue::Boolean(flag)) => Some(f64::from(u8::from(*flag))),
        None => None,
    };
    match number {
        Some(number) if number.is_finite() => Ok(number),
        _ => Err(Failure::new(
            &texts::formula_snapshot_missing(symbol),
            Cause::Runtime,
            NextAction::None,
        )),
    }
}

/// The weight of a snapshot formula: the guard it names, or its fixed number, or the default.
fn snapshot_weight(
    snapshot: &BTreeMap<String, TransferValue>,
    weight_symbol: Option<&str>,
    weight: Option<f64>,
    default: f64,
) -> Result<f64, Failure> {
    match weight_symbol {
        Some(symbol) => snapshot_value(snapshot, symbol),
        None => Ok(weight.unwrap_or(default)),
    }
}

/// 1 when the fresh input equals the predicate's level, else 0: the exact comparison of `model-input-is`.
#[allow(clippy::float_cmp)] // The TypeScript compares the fresh command with the predicate exactly (`===`).
fn boolean_level(fresh: f64, level: f64) -> f64 {
    f64::from(u8::from(fresh == level))
}

#[cfg(test)]
#[path = "initialization_formulas_tests.rs"]
mod tests;
