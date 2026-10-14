//! The declared states of a controller and their initial values at its operating point (specification 35, C0
//! "Controller state initialization" and C1 "The `control-pid-v1` state initialization"; annex E6.4 `declared-state-v1`).
//! Ported from `controllerInitialState.ts`: the checks of a declared state list (`decodeControllerInitialization`) and the
//! values its states take (`initializationValues`). The formula kinds of the parked features are refused when the list is
//! built, so no later step meets them.

use std::collections::BTreeMap;

use super::channel_mapping::connected_model_channels;
use super::texts;
use super::transfer::{rounded_transfer_value, TransferMember, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    InitialSource, InitialValue, InitializedState, LoopDefinition, StateType,
};
use crate::model::{GeneralModel, ModelChannel};

/// The most states one declared initialization holds (`decodeControllerInitialization`).
pub const MAXIMUM_STATES: usize = 32;

/// The declared states of one initialization, checked when they are built (`ControllerInitialization`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerInitialization {
    states: Vec<InitializedState>,
}

impl ControllerInitialization {
    /// Checks the declared states (`decodeControllerInitialization`): at most 32 states with distinct symbols, each with a
    /// unit and finite ordered bounds, a BOOL state dimensionless in `0..=1`, and an initial value whose fields agree.
    ///
    /// # Errors
    /// A state breaks one of these rules, or uses a formula kind that the Rust engine does not run yet.
    pub fn new(states: Vec<InitializedState>) -> Result<Self, Failure> {
        if states.len() > MAXIMUM_STATES {
            return Err(refusal(&texts::init_states_too_many(
                states.len(),
                MAXIMUM_STATES,
            )));
        }
        for (index, state) in states.iter().enumerate() {
            if states[..index]
                .iter()
                .any(|earlier| earlier.symbol == state.symbol)
            {
                return Err(refusal(&texts::init_state_duplicate(&state.symbol)));
            }
            validate_state(state)?;
        }
        Ok(Self { states })
    }

    /// The states, in the order they were declared.
    #[must_use]
    pub fn states(&self) -> &[InitializedState] {
        &self.states
    }
}

/// The kind word of a formula that the Rust engine does not run yet (the parked features of checklist V7: the notch of a
/// program's own law, the measured-disturbance feedforward, the decoupler and the state models), or `None` for a formula it
/// runs.
#[must_use]
pub(super) fn parked_source_word(initial: &InitialValue) -> Option<&'static str> {
    match initial {
        InitialValue::From(InitialSource::ModelState { .. }) => Some("model-state"),
        InitialValue::From(InitialSource::ScaledModelInput { .. }) => Some("scaled-model-input"),
        InitialValue::From(InitialSource::DecoupledModelInput { .. }) => {
            Some("decoupled-model-input")
        }
        InitialValue::From(InitialSource::NotchHistory { .. }) => Some("notch-history"),
        InitialValue::Constant(_)
        | InitialValue::From(
            InitialSource::ModelInput { .. }
            | InitialSource::ModelOutput { .. }
            | InitialSource::ModelInputIs { .. }
            | InitialSource::PidIntegral { .. }
            | InitialSource::WeightedError { .. },
        ) => None,
    }
}

/// The operating-point input at `index`, or 0 when the model holds none there (`operatingPoint?.inputs[index] ?? 0`).
#[must_use]
pub fn operating_input(model: &GeneralModel, index: usize) -> f64 {
    model
        .identity()
        .operating_point
        .as_ref()
        .and_then(|point| point.inputs.get(index))
        .copied()
        .unwrap_or(0.0)
}

/// The operating-point output at `index`, or 0 when the model holds none there (`operatingPoint?.outputs[index] ?? 0`).
#[must_use]
pub fn operating_output(model: &GeneralModel, index: usize) -> f64 {
    model
        .identity()
        .operating_point
        .as_ref()
        .and_then(|point| point.outputs.get(index))
        .copied()
        .unwrap_or(0.0)
}

/// The value each declared state takes at the operating point of `model` (`initializationValues`, C0 and C1), rounded as
/// its state is typed (E6.2, A6.4): a constant; the model's input or output coordinate; the Boolean predicate of an
/// operating command; or the integral or weighted-error history of a PID, from the gains in `parameters`.
///
/// # Errors
/// A coordinate or unit does not match the model, a parameter that a value needs is missing or not finite, a Boolean
/// predicate is not a level of the actuator, or a value does not fit its state.
pub fn initialization_values(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    definition: &ControllerInitialization,
    parameters: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut values = BTreeMap::new();
    for state in definition.states() {
        let value = initial_value(loop_definition, model, state, parameters)?;
        let member = transfer_member(state);
        values.insert(
            state.symbol.clone(),
            rounded_transfer_value(&member, value)?,
        );
    }
    Ok(values)
}

/// The declared state as a transfer member: its IEC type, unit and bounds (E6.2).
#[must_use]
pub fn transfer_member(state: &InitializedState) -> TransferMember {
    TransferMember {
        symbol: state.symbol.clone(),
        iec_type: state.iec_type.into(),
        unit: state.unit.clone(),
        minimum: state.minimum,
        maximum: state.maximum,
    }
}

impl From<StateType> for TransferValueType {
    fn from(value: StateType) -> Self {
        match value {
            StateType::Real => Self::Real,
            StateType::Lreal => Self::Lreal,
            StateType::Bool => Self::Bool,
        }
    }
}

/// Refuses a declaration with the settings cause and the action of opening the loop's setup.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

/// Checks one state's type, unit, bounds and initial value (`decodeControllerInitialization`, per state).
#[allow(clippy::float_cmp)] // The bounds of a BOOL are exact: the TypeScript accepts only 0 and 1.
fn validate_state(state: &InitializedState) -> Result<(), Failure> {
    let bounds_fit =
        state.minimum.is_finite() && state.maximum.is_finite() && state.minimum <= state.maximum;
    let bool_fits = state.iec_type != StateType::Bool
        || (state.unit == "1" && state.minimum == 0.0 && state.maximum == 1.0);
    if state.symbol.is_empty() || state.unit.is_empty() || !bounds_fit || !bool_fits {
        return Err(refusal(&texts::init_state_declaration_invalid(
            &state.symbol,
        )));
    }
    if let Some(word) = parked_source_word(&state.initial) {
        return Err(Failure::new(
            &texts::init_parked_source(&state.symbol, word),
            Cause::ControllerCapability,
            NextAction::None,
        ));
    }
    let fits = match &state.initial {
        InitialValue::Constant(value)
        | InitialValue::From(InitialSource::ModelInputIs { value, .. }) => value.is_finite(),
        InitialValue::From(InitialSource::PidIntegral {
            gain_symbol,
            weight_symbol,
            weight,
            ..
        }) => !gain_symbol.is_empty() && weight_fits(weight_symbol.as_deref(), *weight),
        InitialValue::From(InitialSource::WeightedError {
            weight_symbol,
            weight,
            ..
        }) => weight_fits(weight_symbol.as_deref(), *weight),
        InitialValue::From(_) => true,
    };
    if fits {
        Ok(())
    } else {
        Err(refusal(&texts::init_state_declaration_invalid(
            &state.symbol,
        )))
    }
}

/// A weight is a symbol or a finite number, never both (`decodeControllerInitialization`).
fn weight_fits(symbol: Option<&str>, weight: Option<f64>) -> bool {
    match (symbol, weight) {
        (Some(symbol), None) => !symbol.is_empty(),
        (None, Some(weight)) => weight.is_finite(),
        (None, None) => true,
        (Some(_), Some(_)) => false,
    }
}

/// The value of one declared state at the operating point, before it is rounded (`initializationValues`).
fn initial_value(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    state: &InitializedState,
    parameters: &BTreeMap<String, f64>,
) -> Result<f64, Failure> {
    let identity = model.identity();
    match &state.initial {
        InitialValue::Constant(value) => Ok(*value),
        InitialValue::From(InitialSource::ModelInput { index }) => {
            coordinate_fits(identity.inputs.get(*index), state)?;
            Ok(operating_input(model, *index))
        }
        InitialValue::From(InitialSource::ModelOutput { index }) => {
            coordinate_fits(identity.outputs.get(*index), state)?;
            Ok(operating_output(model, *index))
        }
        InitialValue::From(InitialSource::ModelInputIs { index, value }) => {
            boolean_state_value(loop_definition, model, state, *index, *value)
        }
        InitialValue::From(InitialSource::PidIntegral {
            index,
            input_index,
            gain_symbol,
            weight_symbol,
            weight,
        }) => {
            if identity.outputs.get(*index).is_none() {
                return Err(output_unavailable(&state.symbol));
            }
            if identity
                .inputs
                .get(*input_index)
                .is_none_or(|input| input.unit != state.unit)
            {
                return Err(refusal(&texts::init_integral_unit(&state.symbol)));
            }
            let weight = weight_of(
                loop_definition,
                parameters,
                weight_symbol.as_deref(),
                *weight,
                1.0,
            )?;
            let gain = parameter_value(loop_definition, parameters, gain_symbol)?;
            Ok(operating_input(model, *input_index)
                - gain * (weight - 1.0) * operating_output(model, *index))
        }
        InitialValue::From(InitialSource::WeightedError {
            index,
            weight_symbol,
            weight,
        }) => {
            let output = identity
                .outputs
                .get(*index)
                .ok_or_else(|| output_unavailable(&state.symbol))?;
            if output.unit != state.unit {
                return Err(refusal(&texts::init_filter_unit(&state.symbol)));
            }
            let weight = weight_of(
                loop_definition,
                parameters,
                weight_symbol.as_deref(),
                *weight,
                0.0,
            )?;
            Ok((weight - 1.0) * operating_output(model, *index))
        }
        InitialValue::From(_) => {
            let word = parked_source_word(&state.initial).unwrap_or("a parked kind");
            Err(Failure::new(
                &texts::init_parked_source(&state.symbol, word),
                Cause::ControllerCapability,
                NextAction::None,
            ))
        }
    }
}

/// The value of a Boolean state that follows an operating command: 1 when the operating input equals the predicate, else 0
/// (`model-input-is`, C0). The predicate must be a finite level inside the actuator's range, and the state dimensionless.
#[allow(clippy::float_cmp)] // The TypeScript compares the operating command with the predicate exactly (`===`).
fn boolean_state_value(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    state: &InitializedState,
    index: usize,
    predicate: f64,
) -> Result<f64, Failure> {
    let connected = connected_model_channels(loop_definition, model)?;
    let range = connected.input.range;
    let level =
        index == 0 && predicate.is_finite() && (range.minimum..=range.maximum).contains(&predicate);
    if !level || state.unit != "1" {
        return Err(refusal(&texts::init_boolean_incompatible(&state.symbol)));
    }
    Ok(f64::from(u8::from(
        operating_input(model, index) == predicate,
    )))
}

/// The model channel that a state takes its coordinate from must exist and share the state's unit (`channels[index].unit`).
fn coordinate_fits(
    channel: Option<&ModelChannel>,
    state: &InitializedState,
) -> Result<(), Failure> {
    match channel {
        Some(channel) if channel.unit == state.unit => Ok(()),
        _ => Err(refusal(&texts::init_coordinates_units(&state.symbol))),
    }
}

/// The refusal of a state whose model output does not exist.
fn output_unavailable(symbol: &str) -> Failure {
    refusal(&texts::init_output_unavailable(symbol))
}

/// A weight: the parameter it names, or its fixed number, or the default (`weightSymbol ? parameter(...) : weight ?? default`).
fn weight_of(
    loop_definition: &LoopDefinition,
    parameters: &BTreeMap<String, f64>,
    weight_symbol: Option<&str>,
    weight: Option<f64>,
    default: f64,
) -> Result<f64, Failure> {
    match weight_symbol {
        Some(symbol) => parameter_value(loop_definition, parameters, symbol),
        None => Ok(weight.unwrap_or(default)),
    }
}

/// A parameter of the loop's record, read as a finite value (`parameter(symbol)` of `initializationValues`).
fn parameter_value(
    loop_definition: &LoopDefinition,
    parameters: &BTreeMap<String, f64>,
    symbol: &str,
) -> Result<f64, Failure> {
    let bound = loop_definition
        .parameter_binding
        .as_ref()
        .is_some_and(|binding| {
            binding
                .parameters
                .iter()
                .any(|parameter| parameter.symbol == symbol)
        });
    match parameters.get(symbol) {
        Some(value) if bound && value.is_finite() => Ok(*value),
        _ => Err(Failure::new(
            &texts::init_parameter_unavailable(symbol),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )),
    }
}

#[cfg(test)]
#[path = "initial_state_tests.rs"]
mod tests;
