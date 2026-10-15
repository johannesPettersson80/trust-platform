//! The initialization formulas of the declared states (specification 35, C0 "Controller state initialization"; annex E6.4
//! `initialization_formulas`): where a state's value comes from at the task boundary, so that the transfer reads it fresh
//! when it starts rather than from an older snapshot. Ported from `controllerInitializationFormulas.ts`. The kinds
//! `scaled-measurement` (the measured-disturbance feedforward, C8) and `decoupled-output` (the static decoupler, C7) are served: the
//! transfer reads the measured disturbance or the decoupled plant inputs fresh. The kind `notch-history` (the notch of a program's
//! own law, C5) stays parked: `ControllerInitialization::new` refuses the declarations that use it.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::cascade_adapter::cascade_measurement_symbol;
use super::channel_mapping::{connected_model_channels, ConnectedChannels};
use super::initial_state::{
    decoupled_share, decoupler_not_square, parked_source_word, ControllerInitialization,
};
use super::texts;
use super::transfer::{TransferBinding, TransferValue};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    channel_signal, measured_disturbance, InitialSource, InitialValue, InitializedState,
    LoopDefinition,
};
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
    /// `scaled-measurement`: the gain times the measured disturbance, both read fresh (C8, `K_ff·d`).
    ScaledMeasurement {
        /// The measured disturbance, read fresh.
        process_variable: String,
        /// The feedforward gain, a guard that the snapshot holds.
        gain_symbol: String,
    },
    /// `decoupled-output`: row `row` of D⁻¹·u, with u the plant inputs read fresh and D the decoupler's gains (C7).
    DecoupledOutput {
        /// The plant inputs that the group's channels drive, read fresh, in the channels' order.
        output_symbols: Vec<String>,
        /// The decoupler's gains, guards that the snapshot holds: `matrix_symbols[i][j]` is D[i][j].
        matrix_symbols: Vec<Vec<String>>,
        /// The row of D⁻¹·u that the state takes.
        row: usize,
    },
}

/// The fresh source of each declared state that has one, keyed by state symbol (`initializationFormulas`). A state with a
/// constant initial value has none, and so has a state that the model does not give a source to.
///
/// # Errors
/// The loop's plant connection does not fit the model (see [`connected_model_channels`]) for a loop without a PID group, a source's
/// coordinate has no bound input or output, a Boolean predicate is not a level of the actuator, or a decoupler's channels do not
/// fit its matrix.
pub fn initialization_formulas(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    definition: &ControllerInitialization,
) -> Result<BTreeMap<String, InitializationFormula>, Failure> {
    // A group has a plant connection per channel, which the scalar connection does not describe, so only a loop without a group
    // reads that connection (as the TypeScript does, eagerly), and a group's sources read its channels.
    if loop_definition.controller.group.is_none() {
        connected_model_channels(loop_definition, model)?;
    }
    let mut formulas = BTreeMap::new();
    for state in definition.states() {
        if let Some(formula) = formula_of(loop_definition, model, state)? {
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
        InitializationFormula::ScaledMeasurement {
            process_variable,
            gain_symbol,
        } => {
            let gain = snapshot_value(snapshot, gain_symbol)?;
            let measured = snapshot_value(snapshot, process_variable)?;
            Ok(gain * measured)
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
        InitializationFormula::DecoupledOutput {
            output_symbols,
            matrix_symbols,
            row,
        } => decoupled_output_share(binding, snapshot, output_symbols, matrix_symbols, *row),
    }
}

/// The formula of one declared state, or `None` for a constant (`initializationFormulas`, per state). A decoupled state reads its
/// group's channels; every other source reads the loop's plant connection.
fn formula_of(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
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
    if let InitialSource::DecoupledOutput {
        index,
        matrix_symbols,
    } = source
    {
        return decoupled_output_formula(loop_definition, state, *index, matrix_symbols).map(Some);
    }
    let channels = connected_model_channels(loop_definition, model)?;
    scalar_formula(loop_definition, model, &channels, state, source).map(Some)
}

/// The formula of a state whose source reads the loop's plant connection (`initializationFormulas`, per kind).
fn scalar_formula(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    channels: &ConnectedChannels<'_>,
    state: &InitializedState,
    source: &InitialSource,
) -> Result<InitializationFormula, Failure> {
    let formula = match source {
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
            process_variable: process_symbol(loop_definition, model, channels, state, *index)?,
        },
        InitialSource::PidIntegral {
            index,
            input_index,
            gain_symbol,
            weight_symbol,
            weight,
        } => {
            let process_variable = process_symbol(loop_definition, model, channels, state, *index)?;
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
            process_variable: process_symbol(loop_definition, model, channels, state, *index)?,
            weight_symbol: weight_symbol.clone(),
            weight: *weight,
        },
        InitialSource::ScaledModelInput { index, gain_symbol } => {
            InitializationFormula::ScaledMeasurement {
                process_variable: disturbance_symbol(loop_definition, model, state, *index)?,
                gain_symbol: gain_symbol.clone(),
            }
        }
        // A model input that is the measured disturbance is read as a measurement, never as an actuator (C0, C8).
        InitialSource::ModelInput { index } => {
            match disturbance_symbol_at(loop_definition, model, *index) {
                Some(process_variable) => InitializationFormula::ModelOutput { process_variable },
                None => InitializationFormula::ModelInput {
                    output_symbol: input_symbol(channels, state, *index)?,
                },
            }
        }
        // A decoupled source is routed by `formula_of`, before this match.
        InitialSource::ModelState { .. }
        | InitialSource::DecoupledOutput { .. }
        | InitialSource::NotchHistory { .. } => {
            return Err(Failure::new(
                &texts::init_parked_source(&state.symbol, "a parked kind"),
                Cause::ControllerCapability,
                NextAction::None,
            ))
        }
    };
    Ok(formula)
}

/// The fresh source of a decoupled state: the plant inputs that the group's channels drive, in channel order, and the decoupler's
/// gains (`decoupled-output`, C7). The decoupler must be square over the channels, and the state's row must be one of them.
///
/// # Errors
/// The loop has no channels, a channel has no plant input, or the decoupler is not square over the channels.
fn decoupled_output_formula(
    loop_definition: &LoopDefinition,
    state: &InitializedState,
    row: usize,
    matrix_symbols: &[Vec<String>],
) -> Result<InitializationFormula, Failure> {
    let Some(loop_channels) = &loop_definition.channels else {
        return Err(no_bound_actuator(state));
    };
    let size = matrix_symbols.len();
    if size == 0
        || loop_channels.inputs.len() != size
        || row >= size
        || matrix_symbols.iter().any(|entries| entries.len() != size)
    {
        return Err(decoupler_not_square());
    }
    let output_symbols = loop_channels
        .inputs
        .iter()
        .map(|input| {
            channel_signal(loop_definition, &input.signal_id)
                .map(|signal| signal.symbol.to_owned())
                .ok_or_else(|| no_bound_actuator(state))
        })
        .collect::<Result<Vec<String>, Failure>>()?;
    Ok(InitializationFormula::DecoupledOutput {
        output_symbols,
        matrix_symbols: matrix_symbols.to_vec(),
        row,
    })
}

/// The refusal of a state whose group has no bound plant input for it (`formula_no_bound_actuator`).
fn no_bound_actuator(state: &InitializedState) -> Failure {
    Failure::new(
        &texts::formula_no_bound_actuator(&state.symbol),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

/// The signal of the measured disturbance at a model input, when the loop binds that input to a measured disturbance (C8): the loop's
/// channel for the input, whose signal has the disturbance role. The transfer reads its symbol fresh at the task boundary.
fn disturbance_symbol_at(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    index: usize,
) -> Option<String> {
    let model_input = model.identity().inputs.get(index)?;
    let channel = loop_definition
        .channels
        .as_ref()?
        .inputs
        .iter()
        .find(|channel| channel.model_channel_id == model_input.id)
        .filter(|channel| measured_disturbance(loop_definition, channel))?;
    channel_signal(loop_definition, &channel.signal_id).map(|signal| signal.symbol.to_owned())
}

/// The signal of the measured disturbance at a model input (C8, `scaled-measurement`), for a state that scales it by its gain.
///
/// # Errors
/// The loop binds no measured disturbance to the model input.
fn disturbance_symbol(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    state: &InitializedState,
    index: usize,
) -> Result<String, Failure> {
    disturbance_symbol_at(loop_definition, model, index).ok_or_else(|| {
        Failure::new(
            &texts::init_disturbance_unbound(&state.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })
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

/// The fresh value of a decoupled state: row `row` of D⁻¹·u, with u the fresh plant inputs and D the decoupler's gains from the
/// snapshot (`evaluateInitializationFormula`, `decoupled-output`, C7).
fn decoupled_output_share(
    binding: &TransferBinding,
    snapshot: &BTreeMap<String, TransferValue>,
    output_symbols: &[String],
    matrix_symbols: &[Vec<String>],
    row: usize,
) -> Result<f64, Failure> {
    let matrix = matrix_symbols
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|symbol| snapshot_value(snapshot, symbol))
                .collect::<Result<Vec<f64>, Failure>>()
        })
        .collect::<Result<Vec<Vec<f64>>, Failure>>()?;
    let inputs = output_symbols
        .iter()
        .map(|symbol| fresh_output(binding, snapshot, symbol))
        .collect::<Result<Vec<f64>, Failure>>()?;
    decoupled_share(&matrix, &inputs, row)
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
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    channels: &ConnectedChannels<'_>,
    state: &InitializedState,
    index: usize,
) -> Result<String, Failure> {
    // A cascade measures two outputs, so the measurement of each index is the loop's own (C6).
    if let Some(symbol) = cascade_measurement_symbol(loop_definition, model, index) {
        return Ok(symbol);
    }
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

#[cfg(test)]
#[path = "initialization_formulas_decoupled_tests.rs"]
mod decoupled_tests;
