//! The general model of Control Studio (generalModel.ts): its identity (revision, source, validation,
//! evidence, channels, delay and operating point) with its dynamics, and the decoder that accepts a
//! model only when every product rule holds. The mathematical checks are the library's constructors.

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::model::{OperatingPoint, StateSpace, Timing, TransferFunction};

use super::limits::{MAXIMUM_CHANNELS, MAXIMUM_POLYNOMIAL_LENGTH};

/// The smallest and largest value of one channel, inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperatingRange {
    /// The smallest value the channel may take.
    pub minimum: f64,
    /// The largest value the channel may take.
    pub maximum: f64,
}

/// One input, output or state of a model: its identifier, its unit and its operating range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelChannel {
    /// The identifier, unique among the channels of the same kind.
    pub id: String,
    /// The unit of the channel's values.
    pub unit: String,
    /// The values the channel may take.
    pub operating_range: OperatingRange,
}

/// Where a model came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    /// Supplied by the engineer.
    Provided,
    /// Fitted from a recording.
    Fitted,
    /// Taken from a simulation.
    Simulation,
}

impl ModelSource {
    /// The word of the source as the model file writes it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Provided => "provided",
            Self::Fitted => "fitted",
            Self::Simulation => "simulation",
        }
    }
}

/// Whether a model has been validated against independent evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ModelValidation {
    /// Not validated.
    Unvalidated,
    /// Validated against the evidence listed with the model.
    Validated,
}

impl ModelValidation {
    /// The word of the validation state as the model file writes it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Unvalidated => "unvalidated",
            Self::Validated => "validated",
        }
    }
}

/// The identity of a model: what it is, which channels it has, its delay and its operating point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelIdentity {
    /// The revision, a non-blank text.
    pub revision: String,
    /// Where the model came from.
    pub source: ModelSource,
    /// Whether the model has been validated.
    pub validation: ModelValidation,
    /// The evidence of the validation; required when the model is validated.
    pub evidence: Vec<String>,
    /// The inputs, in order.
    pub inputs: Vec<ModelChannel>,
    /// The outputs, in order.
    pub outputs: Vec<ModelChannel>,
    /// The pure input delay in seconds, zero or more.
    pub delay_seconds: f64,
    /// The operating point the deviations are measured from, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_point: Option<OperatingPoint>,
}

/// A general model: a transfer function with one input and one output, or a state-space model whose
/// states are listed, continuous or discrete. The JSON `kind` is the word of the variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum GeneralModel {
    /// A transfer function `N(s)/D(s)` (or in `z`).
    TransferFunction {
        /// The identity of the model.
        #[serde(flatten)]
        identity: ModelIdentity,
        /// The numerator and denominator.
        #[serde(flatten)]
        transfer: TransferFunction,
    },
    /// A continuous-time state-space model.
    ContinuousStateSpace {
        /// The identity of the model.
        #[serde(flatten)]
        identity: ModelIdentity,
        /// The states, in the order of the matrices.
        states: Vec<ModelChannel>,
        /// The matrices `A`, `B`, `C`, `D`.
        #[serde(flatten)]
        system: StateSpace,
    },
    /// A discrete-time state-space model with a sample time.
    DiscreteStateSpace {
        /// The identity of the model.
        #[serde(flatten)]
        identity: ModelIdentity,
        /// The states, in the order of the matrices.
        states: Vec<ModelChannel>,
        /// The matrices `A`, `B`, `C`, `D`.
        #[serde(flatten)]
        system: StateSpace,
        /// The sample period in seconds: finite and positive.
        sample_time_seconds: f64,
    },
}

impl GeneralModel {
    /// The identity of the model, whatever its kind.
    #[must_use]
    pub fn identity(&self) -> &ModelIdentity {
        match self {
            Self::TransferFunction { identity, .. }
            | Self::ContinuousStateSpace { identity, .. }
            | Self::DiscreteStateSpace { identity, .. } => identity,
        }
    }

    /// The timing of the model: discrete for a discrete state-space model, continuous otherwise.
    #[must_use]
    pub fn timing(&self) -> Timing {
        match self {
            Self::DiscreteStateSpace {
                sample_time_seconds,
                ..
            } => Timing::Discrete {
                sample_time_seconds: *sample_time_seconds,
            },
            Self::TransferFunction { .. } | Self::ContinuousStateSpace { .. } => Timing::Continuous,
        }
    }

    /// A copy of the model that starts from `point`, and is the same model in every other respect, its revision included. Only the
    /// Manual-to-Auto entry case uses it: that case starts where its plant starts (specification 35, B5.3), so the model that Validate
    /// qualifies keeps its own operating point.
    #[must_use]
    pub fn with_operating_point(&self, point: OperatingPoint) -> Self {
        let mut copy = self.clone();
        match &mut copy {
            Self::TransferFunction { identity, .. }
            | Self::ContinuousStateSpace { identity, .. }
            | Self::DiscreteStateSpace { identity, .. } => identity.operating_point = Some(point),
        }
        copy
    }
}

/// Decodes an imported model. Returns the model only when every rule of the model contract holds:
/// the revision, the source and validation words, the evidence of a validated model, the channels
/// (1 to 32 each, unique identifiers, text identifiers and units, finite ranges with a minimum below
/// the maximum), a finite non-negative delay, an operating point inside the channel ranges, and for
/// each kind its shape and sample time. A transfer function has one input, one output and polynomials
/// of 1 to 17 finite coefficients. The identity and dynamics are taken from the checked value.
#[must_use]
pub fn decode_general_model(value: &Value) -> Option<GeneralModel> {
    let object = value.as_object()?;
    object
        .get("revision")
        .and_then(Value::as_str)
        .filter(|text| js_text(text))?;
    let source = object.get("source").and_then(Value::as_str)?;
    if !matches!(source, "provided" | "fitted" | "simulation") {
        return None;
    }
    let validation = object.get("validation").and_then(Value::as_str)?;
    if !matches!(validation, "unvalidated" | "validated") {
        return None;
    }
    let evidence = object.get("evidence").and_then(Value::as_array)?;
    if !evidence
        .iter()
        .all(|entry| entry.as_str().is_some_and(js_text))
    {
        return None;
    }
    if validation == "validated" && evidence.is_empty() {
        return None;
    }
    let inputs = decode_channels(object.get("inputs"))?;
    let outputs = decode_channels(object.get("outputs"))?;
    let delay = object.get("delaySeconds").and_then(Value::as_f64)?;
    if !delay.is_finite() || delay < 0.0 {
        return None;
    }
    if let Some(point) = object.get("operatingPoint") {
        check_operating_point(point, &inputs, &outputs)?;
    }
    match object.get("kind").and_then(Value::as_str)? {
        "transfer-function" => check_transfer(object, inputs.len(), outputs.len())?,
        "continuous-state-space" => check_state_space(object, inputs.len(), outputs.len(), false)?,
        "discrete-state-space" => check_state_space(object, inputs.len(), outputs.len(), true)?,
        _ => return None,
    }
    crate::json::decode_value(value.clone()).ok()
}

/// A transfer function: one input, one output, and polynomials the library accepts.
fn check_transfer(object: &Map<String, Value>, inputs: usize, outputs: usize) -> Option<()> {
    if inputs != 1 || outputs != 1 {
        return None;
    }
    let numerator = polynomial(object.get("numerator"))?;
    let denominator = polynomial(object.get("denominator"))?;
    TransferFunction::new(numerator, denominator)
        .ok()
        .map(|_| ())
}

/// A state-space model: its states, the matrix shapes, and its sample time when discrete. A
/// continuous model must not carry a sample time.
fn check_state_space(
    object: &Map<String, Value>,
    inputs: usize,
    outputs: usize,
    discrete: bool,
) -> Option<()> {
    let states = decode_channels(object.get("states"))?.len();
    let a = matrix(object.get("a"), states, states)?;
    let b = matrix(object.get("b"), states, inputs)?;
    let c = matrix(object.get("c"), outputs, states)?;
    let d = matrix(object.get("d"), outputs, inputs)?;
    if discrete {
        let sample_time = object.get("sampleTimeSeconds").and_then(Value::as_f64)?;
        Timing::discrete(sample_time).ok()?;
    } else if object.contains_key("sampleTimeSeconds") {
        return None;
    }
    StateSpace::new(a, b, c, d).ok().map(|_| ())
}

/// The operating point: one coordinate per channel, each finite and inside its channel's range.
fn check_operating_point(
    value: &Value,
    inputs: &[ModelChannel],
    outputs: &[ModelChannel],
) -> Option<()> {
    let object = value.as_object()?;
    within_ranges(object.get("inputs"), inputs)?;
    within_ranges(object.get("outputs"), outputs)?;
    Some(())
}

fn within_ranges(value: Option<&Value>, channels: &[ModelChannel]) -> Option<Vec<f64>> {
    let coordinates = numbers(value?)?;
    let inside = coordinates.len() == channels.len()
        && coordinates.iter().zip(channels).all(|(value, channel)| {
            *value >= channel.operating_range.minimum && *value <= channel.operating_range.maximum
        });
    inside.then_some(coordinates)
}

/// A list of 1 to 32 channels with unique identifiers.
fn decode_channels(value: Option<&Value>) -> Option<Vec<ModelChannel>> {
    let entries = value?.as_array()?;
    if entries.is_empty() || entries.len() > MAXIMUM_CHANNELS {
        return None;
    }
    let channels = entries
        .iter()
        .map(decode_channel)
        .collect::<Option<Vec<_>>>()?;
    let unique = {
        let mut identifiers = HashSet::new();
        channels
            .iter()
            .all(|channel| identifiers.insert(channel.id.as_str()))
    };
    unique.then_some(channels)
}

fn decode_channel(value: &Value) -> Option<ModelChannel> {
    let object = value.as_object()?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|text| js_text(text))?;
    let unit = object
        .get("unit")
        .and_then(Value::as_str)
        .filter(|text| js_text(text))?;
    let range = object.get("operatingRange")?.as_object()?;
    let minimum = finite(range.get("minimum")?)?;
    let maximum = finite(range.get("maximum")?)?;
    (minimum < maximum).then(|| ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange { minimum, maximum },
    })
}

/// A polynomial of 1 to 17 finite coefficients.
fn polynomial(value: Option<&Value>) -> Option<Vec<f64>> {
    let coefficients = numbers(value?)?;
    (1..=MAXIMUM_POLYNOMIAL_LENGTH)
        .contains(&coefficients.len())
        .then_some(coefficients)
}

/// A matrix with `rows` rows of `columns` finite numbers.
fn matrix(value: Option<&Value>, rows: usize, columns: usize) -> Option<DenseMatrix> {
    let entries = value?.as_array()?;
    if entries.len() != rows {
        return None;
    }
    entries
        .iter()
        .map(|row| numbers(row).filter(|values| values.len() == columns))
        .collect()
}

/// A JSON array of finite numbers.
pub(crate) fn numbers(value: &Value) -> Option<Vec<f64>> {
    value.as_array()?.iter().map(finite).collect()
}

/// A finite JSON number.
pub(crate) fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

/// The characters that JavaScript's `String.prototype.trim` removes: the ASCII white space and line
/// terminators, the no-break space, the byte-order mark, and the Unicode space separators.
fn is_js_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// A text that is not blank in JavaScript's sense (the `TypeScript`'s `text` check).
pub(crate) fn js_text(text: &str) -> bool {
    !text.trim_matches(is_js_whitespace).is_empty()
}
