//! The simulated plant of a selected model for an isolated Verify run (modelSimulation.ts). The
//! binding is checked, the model picks its adapter (the exact delayed realization, the exact scalar
//! adapter of a first-order transfer function, the static gain, or the state-space matrices), and the
//! plant is written as the runtime's TOML. The numerics come from the library; the names, the runtime
//! words and the sentences are the product's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::model::{transfer_function_state_space, StateSpace, TransferFunction};
use trust_control_math::simulate::{is_exact_scalar_delay, scalar_delay_adapter, ScalarDelayForm};

use super::general::{decode_general_model, GeneralModel, ModelChannel, ModelIdentity};
use super::prediction::{delayed_model_realization, delayed_system_of};
use super::refusal::{companion_refusal, simulation_refusal, ModelRefusal};

#[cfg(test)]
mod tests;

/// The kind of an actuator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActuatorKind {
    /// Any value in its range.
    Continuous,
    /// Only its two end values.
    OnOff,
}

impl ActuatorKind {
    /// The word the runtime's plant uses for the kind.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Continuous => "continuous",
            Self::OnOff => "on-off",
        }
    }
}

// From loopChannels.ts; moves with V7.
/// The capability and limits of one actuator.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActuatorCapabilities {
    /// The kind of the actuator.
    pub kind: ActuatorKind,
    /// The smallest value in the actuator's range.
    pub minimum: f64,
    /// The largest value in the actuator's range.
    pub maximum: f64,
    /// The largest rise per second, when limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_up_per_s: Option<f64>,
    /// The largest fall per second, when limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_down_per_s: Option<f64>,
    /// The shortest time an on-off actuator stays on, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_on_seconds: Option<f64>,
    /// The shortest time an on-off actuator stays off, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_off_seconds: Option<f64>,
}

/// Whether an actuator's capability is valid: an on-off actuator has no rate limits, the range is
/// finite with a minimum below the maximum, and each rate or minimum time that is present is finite
/// and not negative.
#[must_use]
pub fn valid_actuator(actuator: &ActuatorCapabilities) -> bool {
    let rates_allowed = actuator.kind == ActuatorKind::Continuous
        || (actuator.rate_up_per_s.is_none() && actuator.rate_down_per_s.is_none());
    rates_allowed
        && actuator.minimum.is_finite()
        && actuator.maximum.is_finite()
        && actuator.minimum < actuator.maximum
        && [
            actuator.rate_up_per_s,
            actuator.rate_down_per_s,
            actuator.minimum_on_seconds,
            actuator.minimum_off_seconds,
        ]
        .into_iter()
        .flatten()
        .all(|value| value.is_finite() && value >= 0.0)
}

/// The binding of a selected model to its runtime plant: the plant's identifier, its ordered command
/// and feedback addresses, its initial values, its step in milliseconds and its actuators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedModelSimulationBinding {
    /// The plant's identifier.
    pub plant_id: String,
    /// The command address or variable of each input, in order.
    pub command_sources: Vec<String>,
    /// The measurement address or variable of each output, in order.
    pub feedback_targets: Vec<String>,
    /// The input address that reports each actuator's applied value, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actuator_feedback_targets: Option<Vec<Option<String>>>,
    /// The plant's initial state, one value per state.
    pub initial_state: Vec<f64>,
    /// The input held before the first sample, one value per input.
    pub initial_input: Vec<f64>,
    /// The input before the delay, for a delayed model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_delayed_input: Option<f64>,
    /// The simulation step in milliseconds: a whole number from 1 to 1000.
    pub step_ms: f64,
    /// The actuators of the inputs, when they are limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actuators: Option<Vec<ActuatorCapabilities>>,
}

/// The inline plant that an isolated Verify run consumes: the three fields of the runtime request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SimulationOverride {
    /// The revision of the model.
    pub model_revision: String,
    /// The provenance: the source and the validation of the model.
    pub model_provenance: String,
    /// The plant as TOML.
    pub simulation_toml: String,
}

/// The materialized plant. Its coordinate description is editor metadata: the runtime request accepts
/// only the simulation override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SelectedModelSimulation {
    /// The override that the runtime request carries.
    #[serde(rename = "simulation_override")]
    pub simulation_override: SimulationOverride,
    /// What the plant's state coordinates mean, for the editor.
    #[serde(rename = "stateCoordinateDescription")]
    pub state_coordinate_description: String,
}

/// Produces the inline, simulation-only plant of a selected model. The delay and the form of the model
/// choose the adapter: the exact delayed realization for a delay (except an exact scalar first-order
/// transfer function), the exact scalar adapter for that transfer function, the static gain, or the
/// state-space matrices.
///
/// # Errors
/// Returns the sentence of the first failed rule: the model, the binding, the actuators, the sample
/// periods, the adapters, or the plant's shape.
pub fn materialize_selected_model_simulation(
    value: &Value,
    binding: &SelectedModelSimulationBinding,
) -> Result<SelectedModelSimulation, ModelRefusal> {
    let model = decode_general_model(value).ok_or(ModelRefusal::SelectValidGeneralModel)?;
    validate_binding(binding)?;
    let identity = model.identity();
    if binding
        .actuators
        .as_ref()
        .is_some_and(|actuators| actuators.len() != identity.inputs.len())
    {
        return Err(ModelRefusal::ActuatorCountMismatch);
    }
    let exact_scalar_delay = match &model {
        GeneralModel::TransferFunction { transfer, .. } => is_exact_scalar_delay(transfer),
        GeneralModel::ContinuousStateSpace { .. } | GeneralModel::DiscreteStateSpace { .. } => {
            false
        }
    };
    let materialized = if identity.delay_seconds > 0.0 && !exact_scalar_delay {
        materialize_delayed_model(&model, binding)?
    } else {
        match &model {
            GeneralModel::TransferFunction { transfer, .. } => {
                materialize_transfer_function(identity, transfer, binding)?
            }
            GeneralModel::ContinuousStateSpace { states, system, .. } => {
                materialize_state_space(identity, states, system, None, binding)?
            }
            GeneralModel::DiscreteStateSpace {
                states,
                system,
                sample_time_seconds,
                ..
            } => materialize_state_space(
                identity,
                states,
                system,
                Some(*sample_time_seconds),
                binding,
            )?,
        }
    };
    Ok(SelectedModelSimulation {
        simulation_override: SimulationOverride {
            model_revision: identity.revision.clone(),
            model_provenance: format!(
                "declared-{}-{}",
                identity.source.word(),
                identity.validation.word()
            ),
            simulation_toml: materialized.toml,
        },
        state_coordinate_description: materialized.coordinates,
    })
}

/// One parameter of the plant's TOML: a number, written by the number rules, or text written as it is.
enum Parameter {
    Number(f64),
    Text(String),
}

/// A materialized plant: its TOML and its state coordinate description.
struct Materialized {
    toml: String,
    coordinates: String,
}

fn materialize_delayed_model(
    model: &GeneralModel,
    binding: &SelectedModelSimulationBinding,
) -> Result<Materialized, ModelRefusal> {
    if let GeneralModel::DiscreteStateSpace {
        sample_time_seconds,
        ..
    } = model
    {
        if (sample_time_seconds * 1000.0 - binding.step_ms).abs() > 1e-9 {
            return Err(ModelRefusal::DiscreteStepDisagrees);
        }
    }
    let plant = delayed_system_of(model)?;
    validate_plant_shape(
        binding,
        plant.system.states(),
        plant.system.inputs(),
        plant.system.outputs(),
    )?;
    let realized = delayed_model_realization(
        model,
        binding.step_ms / 1000.0,
        &binding.initial_state,
        &binding.initial_input,
    )?;
    let parameters = matrix_parameters(&realized.system)?;
    let toml = simulation_toml(
        binding,
        &realized.initial_state,
        model.identity(),
        "discrete_state_space",
        &parameters,
    )?;
    Ok(Materialized {
        toml,
        coordinates: format!(
            "Original model coordinates followed by {} ordered input-history slots. Exact held-input delay response at task sample instants.",
            realized.history
        ),
    })
}

fn materialize_state_space(
    identity: &ModelIdentity,
    states: &[ModelChannel],
    system: &StateSpace,
    sample_time_seconds: Option<f64>,
    binding: &SelectedModelSimulationBinding,
) -> Result<Materialized, ModelRefusal> {
    if let Some(sample_time_seconds) = sample_time_seconds {
        if (sample_time_seconds * 1000.0 - binding.step_ms).abs() > 1e-9 {
            return Err(ModelRefusal::DiscreteUpdatePeriod);
        }
    }
    if identity.delay_seconds != 0.0 {
        return Err(ModelRefusal::ContinuousDelayAdapter);
    }
    validate_plant_shape(binding, system.states(), system.inputs(), system.outputs())?;
    let model_word = if sample_time_seconds.is_some() {
        "discrete_state_space"
    } else {
        "state_space"
    };
    let parameters = matrix_parameters(system)?;
    let toml = simulation_toml(
        binding,
        &binding.initial_state,
        identity,
        model_word,
        &parameters,
    )?;
    let names: Vec<&str> = states.iter().map(|state| state.id.as_str()).collect();
    Ok(Materialized {
        toml,
        coordinates: format!(
            "Supplied state coordinates in declared order: {}.",
            names.join(", ")
        ),
    })
}

fn materialize_transfer_function(
    identity: &ModelIdentity,
    transfer: &TransferFunction,
    binding: &SelectedModelSimulationBinding,
) -> Result<Materialized, ModelRefusal> {
    validate_plant_shape(binding, transfer.denominator.len().saturating_sub(1), 1, 1)?;
    let companion = transfer_function_state_space(&transfer.numerator, &transfer.denominator)
        .map_err(companion_refusal)?;
    if identity.delay_seconds > 0.0 {
        let initial_delayed_input = binding
            .initial_delayed_input
            .filter(|value| value.is_finite())
            .ok_or(ModelRefusal::ExactDelayAdapterDomain)?;
        let initial_state = binding.initial_state.first().copied().unwrap_or_default();
        let adapter = scalar_delay_adapter(transfer, identity.delay_seconds, initial_state)
            .map_err(simulation_refusal)?;
        let (model_word, mut parameters, coordinates) = match adapter.form {
            ScalarDelayForm::IntegratorDeadTime { gain } => (
                "integrator_dead_time",
                vec![("gain", Parameter::Number(gain))],
                "Output-deviation state for the delayed integrating model, with the declared input history and operating-point offsets.",
            ),
            ScalarDelayForm::FirstOrderDeadTime {
                gain,
                time_constant_seconds,
            } => (
                "first_order_dead_time",
                vec![
                    ("gain", Parameter::Number(gain)),
                    ("time_constant_s", Parameter::Number(time_constant_seconds)),
                ],
                "Runtime output-state coordinate for the exact first-order dead-time model; it is not a physical measured-state claim.",
            ),
        };
        parameters.push(("dead_time_s", Parameter::Number(adapter.dead_time_seconds)));
        parameters.push((
            "initial_delayed_input",
            Parameter::Number(initial_delayed_input),
        ));
        let toml = simulation_toml(
            binding,
            &[adapter.initial_output_state],
            identity,
            model_word,
            &parameters,
        )?;
        return Ok(Materialized {
            toml,
            coordinates: coordinates.to_owned(),
        });
    }
    if companion.order == 0 {
        if !binding.initial_state.is_empty() {
            return Err(ModelRefusal::StaticNoStates);
        }
        let toml = simulation_toml(
            binding,
            &binding.initial_state,
            identity,
            "static_gain",
            &[("gain", Parameter::Number(companion.numerator[0]))],
        )?;
        return Ok(Materialized {
            toml,
            coordinates: "Static transfer function has no dynamic state coordinates.".to_owned(),
        });
    }
    let parameters = matrix_parameters(&companion.system)?;
    let toml = simulation_toml(
        binding,
        &binding.initial_state,
        identity,
        "state_space",
        &parameters,
    )?;
    let coordinates = (1..=companion.order)
        .map(|index| format!("x{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Materialized {
        toml,
        coordinates: format!(
            "Controllable companion coordinates [{coordinates}] for the unreduced transfer-function denominator; they are not physical measured states."
        ),
    })
}

/// The matrices of a plant as the four TOML parameters `a`, `b`, `c` and `d`.
fn matrix_parameters(system: &StateSpace) -> Result<Vec<(&'static str, Parameter)>, ModelRefusal> {
    Ok(vec![
        ("a", Parameter::Text(matrix_text(&system.a)?)),
        ("b", Parameter::Text(matrix_text(&system.b)?)),
        ("c", Parameter::Text(matrix_text(&system.c)?)),
        ("d", Parameter::Text(matrix_text(&system.d)?)),
    ])
}

fn validate_binding(binding: &SelectedModelSimulationBinding) -> Result<(), ModelRefusal> {
    if let Some(targets) = &binding.actuator_feedback_targets {
        if targets.len() != binding.command_sources.len()
            || targets
                .iter()
                .flatten()
                .any(|address| !address.starts_with("%I"))
        {
            return Err(ModelRefusal::ActuatorFeedbackAddresses);
        }
    }
    if let Some(actuators) = &binding.actuators {
        if !actuators.iter().all(valid_actuator) {
            return Err(ModelRefusal::InvalidActuator);
        }
    }
    if !is_plant_id(&binding.plant_id) {
        return Err(ModelRefusal::PlantId);
    }
    let whole_step = binding.step_ms.fract() == 0.0 && (1.0..=1000.0).contains(&binding.step_ms);
    if !whole_step {
        return Err(ModelRefusal::SimulationStep);
    }
    if binding.command_sources.is_empty()
        || binding
            .command_sources
            .iter()
            .any(|address| !ordered_address_is_valid(address, &["%Q", "%M"], false))
    {
        return Err(ModelRefusal::OrderedCommandBindings);
    }
    if binding.feedback_targets.is_empty()
        || binding
            .feedback_targets
            .iter()
            .any(|address| !ordered_address_is_valid(address, &["%I"], true))
    {
        return Err(ModelRefusal::OrderedFeedbackBindings);
    }
    if !binding
        .initial_state
        .iter()
        .chain(&binding.initial_input)
        .all(|value| value.is_finite())
    {
        return Err(ModelRefusal::InitialValuesFinite);
    }
    Ok(())
}

fn validate_plant_shape(
    binding: &SelectedModelSimulationBinding,
    states: usize,
    inputs: usize,
    outputs: usize,
) -> Result<(), ModelRefusal> {
    let shaped = binding.command_sources.len() == inputs
        && binding.feedback_targets.len() == outputs
        && binding.initial_state.len() == states
        && binding.initial_input.len() == inputs;
    if shaped {
        Ok(())
    } else {
        Err(ModelRefusal::PlantShape)
    }
}

/// A plant identifier: a letter, then letters, digits, underscores or hyphens.
fn is_plant_id(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_alphabetic())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
}

/// An address the bindings accept: an address with one of the prefixes, a `pid:` symbol path when
/// the binding allows a PID measurement, or a symbol path (a program variable).
fn ordered_address_is_valid(address: &str, prefixes: &[&str], allows_pid: bool) -> bool {
    if address.starts_with('%') {
        return prefixes.iter().any(|prefix| address.starts_with(prefix));
    }
    if allows_pid {
        if let Some(path) = address.strip_prefix("pid:") {
            return is_symbol_path(path);
        }
    }
    is_symbol_path(address)
}

/// A dotted symbol path whose every segment is an identifier.
fn is_symbol_path(text: &str) -> bool {
    text.split('.').all(is_identifier)
}

fn is_identifier(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// The TOML of a plant, in the order the runtime reads it: the simulation, the plant table, its
/// parameters, and its actuators, each with its offset from the operating point.
fn simulation_toml(
    binding: &SelectedModelSimulationBinding,
    initial_state: &[f64],
    identity: &ModelIdentity,
    model: &str,
    parameters: &[(&str, Parameter)],
) -> Result<String, ModelRefusal> {
    let mut lines = vec![
        "[simulation]".to_owned(),
        "enabled = true".to_owned(),
        format!("plant_step_ms = {}", js_number(binding.step_ms)),
        String::new(),
        "[[plants]]".to_owned(),
        format!("id = {}", quote(&binding.plant_id)),
        format!("model = {}", quote(model)),
        format!("command_sources = {}", array_text(&binding.command_sources)),
        format!(
            "feedback_targets = {}",
            array_text(&binding.feedback_targets)
        ),
    ];
    if let Some(targets) = &binding.actuator_feedback_targets {
        if targets.iter().any(Option::is_some) {
            let entries: Vec<String> = targets
                .iter()
                .map(|address| match address {
                    None => "{}".to_owned(),
                    Some(address) => format!("{{ address = {} }}", quote(address)),
                })
                .collect();
            lines.push(format!(
                "actuator_feedback_targets = [{}]",
                entries.join(", ")
            ));
        }
    }
    lines.push(format!("initial_state = {}", array_numbers(initial_state)?));
    lines.push(format!(
        "initial_input = {}",
        array_numbers(&binding.initial_input)?
    ));
    if let Some(point) = &identity.operating_point {
        let negated: Vec<f64> = point.inputs.iter().map(|value| -value).collect();
        lines.push(format!("input_offset = {}", array_numbers(&negated)?));
        lines.push(format!(
            "output_offset = {}",
            array_numbers(&point.outputs)?
        ));
    }
    lines.push(String::new());
    lines.push("[plants.parameters]".to_owned());
    for (key, value) in parameters {
        let text = match value {
            Parameter::Number(number) => scalar(*number)?,
            Parameter::Text(text) => text.clone(),
        };
        lines.push(format!("{key} = {text}"));
    }
    lines.push(String::new());
    for (index, actuator) in binding.actuators.iter().flatten().enumerate() {
        let offset = identity
            .operating_point
            .as_ref()
            .and_then(|point| point.inputs.get(index))
            .copied()
            .unwrap_or_default();
        let applied = binding
            .initial_input
            .get(index)
            .copied()
            .unwrap_or_default();
        lines.push("[[plants.actuators]]".to_owned());
        lines.push(format!("kind = {}", quote(actuator.kind.word())));
        if let Some(value) = actuator.minimum_on_seconds {
            lines.push(format!("minimum_on_s = {}", scalar(value)?));
        }
        if let Some(value) = actuator.minimum_off_seconds {
            lines.push(format!("minimum_off_s = {}", scalar(value)?));
        }
        lines.push(format!("minimum = {}", scalar(actuator.minimum - offset)?));
        lines.push(format!("maximum = {}", scalar(actuator.maximum - offset)?));
        lines.push(format!("initial_applied = {}", scalar(applied)?));
        if let Some(value) = actuator.rate_up_per_s {
            lines.push(format!("rate_up_per_s = {}", scalar(value)?));
        }
        if let Some(value) = actuator.rate_down_per_s {
            lines.push(format!("rate_down_per_s = {}", scalar(value)?));
        }
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

/// ECMAScript `Number::toString` (what `String(value)` prints): the shortest digits that round-trip,
/// in fixed notation for decimal exponents from −6 up to 21 and in exponent notation otherwise.
/// Negative zero prints as `0`.
fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let count = i64::try_from(digits.len()).unwrap_or(i64::MAX);
    let point = exponent + 1;
    let body = if count <= point && point <= 21 {
        let zeros = usize::try_from(point - count).unwrap_or(0);
        format!("{digits}{}", "0".repeat(zeros))
    } else if 0 < point && point <= 21 {
        let split = usize::try_from(point).unwrap_or(0);
        format!("{}.{}", &digits[..split], &digits[split..])
    } else if -6 < point && point <= 0 {
        let zeros = usize::try_from(-point).unwrap_or(0);
        format!("0.{}{digits}", "0".repeat(zeros))
    } else {
        let power = point - 1;
        let suffix = if power < 0 {
            format!("e-{}", -power)
        } else {
            format!("e+{power}")
        };
        if count == 1 {
            format!("{digits}{suffix}")
        } else {
            format!("{}.{}{suffix}", &digits[..1], &digits[1..])
        }
    };
    format!("{sign}{body}")
}

/// A simulation value as TOML: its ECMAScript text, refused when it is not finite.
fn scalar(value: f64) -> Result<String, ModelRefusal> {
    if value.is_finite() {
        Ok(js_number(value))
    } else {
        Err(ModelRefusal::SimulationValuesFinite)
    }
}

/// A JSON string literal, which is what `JSON.stringify` writes for a string.
fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

fn array_numbers(values: &[f64]) -> Result<String, ModelRefusal> {
    let items = values
        .iter()
        .map(|value| scalar(*value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", items.join(", ")))
}

fn array_text(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|value| quote(value)).collect();
    format!("[{}]", items.join(", "))
}

fn matrix_text(rows: &DenseMatrix) -> Result<String, ModelRefusal> {
    let items = rows
        .iter()
        .map(|row| array_numbers(row))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", items.join(", ")))
}
