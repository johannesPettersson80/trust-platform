//! The records of low-order step identification: the sample it reads, the model families it may fit,
//! the fit with its uncertainty intervals, and the rejections. This module holds the types the algorithms
//! exchange, with the JSON words of the Control Studio records. The algorithms are in the submodules:
//! `fit` (the least-squares grid fit of every family, N2.2), `simulate` (the free-running prediction under the
//! recorded input, N3.1), `response` (the held responses), `single_run` (the largest input change),
//! `step_response` (the bounded step method), `relay` (relay cycles and the PI candidate), `score` (the
//! independent check) and `gain` (the sign rule of a gain). Which fitted form is recommended is not decided here:
//! `trust-control` makes that choice.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod fit;
pub mod gain;
pub mod relay;
pub mod response;
pub mod score;
pub mod simulate;
pub mod single_run;
pub mod step_response;

/// A count as a float. The counts here are sample counts, far below 2^53.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

/// `⌈count · share⌉`, computed in floating point as the `TypeScript` computes `Math.ceil(count * share)`, so a share
/// of the samples rounds exactly as it did. The result is a sample count, far below the range of `usize`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn ceil_share(count: usize, share: f64) -> usize {
    (count as f64 * share).ceil() as usize
}

/// The quality of a sample: good, uncertain (usable with care), or bad (excluded from fits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SampleQuality {
    /// A usable sample.
    Good,
    /// A sample to use with care.
    Uncertain,
    /// A sample excluded from fits.
    Bad,
}

/// One sample on a time axis: its time, the input and the output.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeDomainSample {
    /// The time of the sample in seconds.
    pub time_seconds: f64,
    /// The input at the sample.
    pub input: f64,
    /// The output at the sample.
    pub output: f64,
    /// The quality of the sample.
    pub quality: SampleQuality,
}

/// A low-order model family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LowOrderModelKind {
    /// `G(s) = K`.
    StaticGain,
    /// `G(s) = K/s`.
    Integrating,
    /// `G(s) = K/(τs + 1)`.
    FirstOrder,
    /// `G(s) = K·e^{−Ls}/(τs + 1)`.
    FirstOrderDeadTime,
    /// `G(s) = K/(τ₁s + 1)(τ₂s + 1)`.
    SecondOrder,
    /// `G(s) = K·e^{−Ls}/(τ₁s + 1)(τ₂s + 1)`.
    SecondOrderDeadTime,
}

impl LowOrderModelKind {
    /// The rank of the family in the order N2.1 lists them, the simplest first: static gain 1, first order 2, first
    /// order with delay 3, integrating with delay 4, second order 5, second order with delay 6. The choice of N3.3
    /// takes the simplest acceptable family by this rank.
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::StaticGain => 1,
            Self::FirstOrder => 2,
            Self::FirstOrderDeadTime => 3,
            Self::Integrating => 4,
            Self::SecondOrder => 5,
            Self::SecondOrderDeadTime => 6,
        }
    }
}

/// A closed interval for one parameter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParameterInterval {
    /// The smallest value in the interval.
    pub minimum: f64,
    /// The largest value in the interval.
    pub maximum: f64,
}

/// How the uncertainty intervals of a fit were obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum UncertaintyMethod {
    /// From the resolution of the search grid and the residual of the validation data.
    #[serde(rename = "grid-resolution-and-validation-residual")]
    GridResolutionAndValidationResidual,
}

/// The uncertainty intervals of a fit. The time-constant and dead-time intervals are present when
/// the model has those parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LowOrderUncertainty {
    /// How the intervals were obtained.
    pub method: UncertaintyMethod,
    /// The interval of the gain.
    pub gain: ParameterInterval,
    /// The interval of the time constant, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the second time constant, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_time_constant_seconds: Option<ParameterInterval>,
    /// The interval of the dead time, when the model has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_time_seconds: Option<ParameterInterval>,
}

/// A fitted low-order model with its residuals, its normalised errors and its uncertainty. The
/// parameters that a family does not have are absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LowOrderModelFit {
    /// The model family.
    pub kind: LowOrderModelKind,
    /// The static gain `K`.
    pub gain: f64,
    /// The time constant `τ` in seconds, when the family has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_constant_seconds: Option<f64>,
    /// The second time constant in seconds, for the second-order families.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_time_constant_seconds: Option<f64>,
    /// The dead time `L` in seconds, for the families with dead time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_time_seconds: Option<f64>,
    /// The residuals on the estimation data.
    pub estimation_residuals: Vec<f64>,
    /// The residuals on the independent validation data.
    pub validation_residuals: Vec<f64>,
    /// The estimation error normalised by the output's variation.
    pub normalized_estimation_error: f64,
    /// The validation error normalised by the output's variation.
    pub normalized_validation_error: f64,
    /// The validation error over the final `max(3, ⌈0.1 · N⌉)` samples, normalised by the same span (N3.2).
    pub final_normalized_validation_error: f64,
    /// The sample interval of the estimation record in seconds: the median spacing of its samples (N3.3).
    pub sample_interval_seconds: f64,
    /// The uncertainty intervals of the parameters.
    pub uncertainty: LowOrderUncertainty,
    /// Whether the best point lies on an edge of its search grid that N2.4 keeps as a search-boundary finding: the longest
    /// delay or either end of a time-constant grid. A best delay of zero is one-sided and does not count (N2.4).
    #[serde(default)]
    pub on_search_boundary: bool,
}

/// The settings of a low-order identification: the families it may fit and the search settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LowOrderIdentificationOptions {
    /// The families that may be fitted.
    pub allowed_kinds: Vec<LowOrderModelKind>,
    /// The fewest samples a fit may use.
    pub minimum_samples: usize,
    /// The smallest span of the input that counts as excitation.
    pub minimum_input_span: f64,
}

/// Why a low-order identification is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LowOrderRejection {
    /// A sample is not finite.
    NonfiniteInput,
    /// Too few samples for the fit.
    InsufficientData,
    /// The input does not excite the plant enough.
    PoorExcitation,
    /// The recording has no step to identify from.
    MissingStep,
    /// The measured output does not change over the record, so no error can be normalised by its span (N3.2).
    FlatOutput,
}
