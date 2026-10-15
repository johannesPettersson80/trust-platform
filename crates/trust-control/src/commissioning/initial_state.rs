//! The declared states of a controller and their initial values at its operating point (specification 35, C0
//! "Controller state initialization" and C1 "The `control-pid-v1` state initialization"; annex E6.4 `declared-state-v1`).
//! Ported from `controllerInitialState.ts`: the checks of a declared state list (`decodeControllerInitialization`) and the
//! values its states take (`initializationValues`). The formula kinds of the parked features are refused when the list is
//! built, so no later step meets them.

use std::collections::BTreeMap;

use trust_control_math::inverse::invert;
use trust_control_math::linalg::NumericalTolerance;

use super::channel_mapping::connected_model_channels;
use super::texts;
use super::transfer::{rounded_transfer_value, TransferMember, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    InitialSource, InitialValue, InitializedState, LoopDefinition, StateType,
};
use crate::model::{GeneralModel, ModelChannel};

/// The most states one controller instance declares (`decodeControllerInitialization`). A loop with `n` controller instances, such as
/// a PID group's members, declares at most `MAXIMUM_STATES * n` states: `LoopDefinition::check_state_budget` checks the total.
pub const MAXIMUM_STATES: usize = 32;

/// The declared states of one initialization, checked when they are built (`ControllerInitialization`).
#[derive(Debug, Clone, PartialEq)]
pub struct ControllerInitialization {
    states: Vec<InitializedState>,
}

impl ControllerInitialization {
    /// Checks the declared states (`decodeControllerInitialization`): distinct symbols, each with a unit and finite ordered bounds,
    /// a BOOL state dimensionless in `0..=1`, and an initial value whose fields agree. The number of states is checked per loop.
    ///
    /// # Errors
    /// A state breaks one of these rules, or uses a formula kind that the Rust engine does not run yet.
    pub fn new(states: Vec<InitializedState>) -> Result<Self, Failure> {
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
/// program's own law and the state models), or `None` for a formula it runs. The measured-disturbance feedforward's
/// `scaled-model-input` (C8) and the static decoupler's `decoupled-output` (C7) are served.
#[must_use]
pub(super) fn parked_source_word(initial: &InitialValue) -> Option<&'static str> {
    match initial {
        InitialValue::From(InitialSource::ModelState { .. }) => Some("model-state"),
        InitialValue::From(InitialSource::NotchHistory { .. }) => Some("notch-history"),
        InitialValue::Constant(_)
        | InitialValue::From(
            InitialSource::ModelInput { .. }
            | InitialSource::ModelOutput { .. }
            | InitialSource::ModelInputIs { .. }
            | InitialSource::PidIntegral { .. }
            | InitialSource::WeightedError { .. }
            | InitialSource::ScaledModelInput { .. }
            | InitialSource::DecoupledOutput { .. },
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

/// Refuses a decoupler that is not square over the plant inputs it drives (`decoupledInput`, C7).
pub(crate) fn decoupler_not_square() -> Failure {
    refusal("The decoupler matrix must be square over the bound plant inputs.")
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
        InitialValue::From(InitialSource::ScaledModelInput { gain_symbol, .. }) => {
            !gain_symbol.is_empty()
        }
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
        InitialValue::From(InitialSource::ScaledModelInput { index, gain_symbol }) => {
            scaled_model_input(
                loop_definition,
                model,
                state,
                *index,
                gain_symbol,
                parameters,
            )
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
        InitialValue::From(InitialSource::DecoupledOutput {
            index,
            matrix_symbols,
        }) => decoupled_start(
            loop_definition,
            model,
            state,
            *index,
            matrix_symbols,
            parameters,
        ),
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

/// The value of a decoupled state at the operating point: row `index` of D⁻¹·u, where D holds the decoupler's gains and u the operating
/// inputs of the plant inputs that the group's channels drive, in channel order (`decoupledInput`, C7).
///
/// # Errors
/// A gain is not a bound finite parameter, a channel has no plant input in the model, the row's input has another unit than the
/// state, or the decoupler is not square or cannot be inverted.
fn decoupled_start(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    state: &InitializedState,
    index: usize,
    matrix_symbols: &[Vec<String>],
    parameters: &BTreeMap<String, f64>,
) -> Result<f64, Failure> {
    let matrix = matrix_symbols
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|symbol| parameter_value(loop_definition, parameters, symbol))
                .collect::<Result<Vec<f64>, Failure>>()
        })
        .collect::<Result<Vec<Vec<f64>>, Failure>>()?;
    let inputs = (0..matrix.len())
        .map(|channel| plant_input_index(loop_definition, model, channel))
        .collect::<Option<Vec<usize>>>()
        .ok_or_else(|| refusal(&texts::init_coordinates_units(&state.symbol)))?;
    coordinate_fits(
        inputs
            .get(index)
            .and_then(|&input| model.identity().inputs.get(input)),
        state,
    )?;
    let operating: Vec<f64> = inputs
        .iter()
        .map(|&input| operating_input(model, input))
        .collect();
    decoupled_share(&matrix, &operating, index)
}

/// The model input that the group's channel at `channel` drives: the model input whose identifier is the channel's `modelChannelId`
/// (`controllerModelIndex` of a group, C7). `None` when the loop has no such channel or the model has no such input.
fn plant_input_index(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    channel: usize,
) -> Option<usize> {
    let input_channel = loop_definition.channels.as_ref()?.inputs.get(channel)?;
    model
        .identity()
        .inputs
        .iter()
        .position(|input| input.id == input_channel.model_channel_id)
}

/// The share of the operating inputs that a decoupled member starts from: row `row` of D⁻¹·`inputs`, where `matrix` is the decoupler D
/// (`decoupledInput`, C7). The matrix must be square over the inputs, and a singular one is refused.
///
/// # Errors
/// The matrix is not square over `inputs`, `row` lies outside it, or the matrix cannot be inverted within the numerical tolerance.
pub(crate) fn decoupled_share(
    matrix: &[Vec<f64>],
    inputs: &[f64],
    row: usize,
) -> Result<f64, Failure> {
    let size = matrix.len();
    if size == 0
        || inputs.len() != size
        || row >= size
        || matrix.iter().any(|entries| entries.len() != size)
    {
        return Err(decoupler_not_square());
    }
    let inverse = invert(&matrix.to_vec(), &DECOUPLER_TOLERANCE)
        .map_err(|_| refusal("The decoupler matrix is singular."))?;
    Ok(inverse[row]
        .iter()
        .zip(inputs)
        .map(|(weight, input)| weight * input)
        .sum())
}

/// The tolerance of a decoupler's inversion: the rank and conditioning rules of the numerical annex (N4.5), as the group design inverts
/// its gains (`TOLERANCE` of `tuning/structures/pid_group.rs`).
const DECOUPLER_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The value of a feedforward state at the operating point: its gain times the measured disturbance at its operating value, `K_ff·d`
/// (C8, `scaled-model-input`). The gain must be bound in the state's unit per the disturbance's unit, as the TypeScript checks it.
///
/// # Errors
/// The disturbance is not a model input, the gain is not a bound parameter in the state's unit per the input's unit, or the gain's value
/// is not finite.
fn scaled_model_input(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    state: &InitializedState,
    index: usize,
    gain_symbol: &str,
    parameters: &BTreeMap<String, f64>,
) -> Result<f64, Failure> {
    let input = model
        .identity()
        .inputs
        .get(index)
        .ok_or_else(|| refusal(&texts::init_coordinates_units(&state.symbol)))?;
    let expected = format!("{}/{}", state.unit, input.unit);
    let bound_unit = loop_definition
        .parameter_binding
        .as_ref()
        .and_then(|binding| {
            binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == gain_symbol)
        })
        .map(|parameter| parameter.unit.as_str());
    if bound_unit != Some(expected.as_str()) {
        return Err(refusal(&texts::init_scaled_gain(&state.symbol, &expected)));
    }
    Ok(parameter_value(loop_definition, parameters, gain_symbol)? * operating_input(model, index))
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
