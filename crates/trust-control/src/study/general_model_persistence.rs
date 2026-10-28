//! The supplied model's section of a study (`generalModelPersistence.ts`; specification 35, A8.1, B3.7 and B10.2;
//! numerical annex N4.1 to N4.6): the model the engineer supplied (or took from the simulated plant), and the analysis of
//! it that the study saved. A supplied model enters the study as a provisional model unless it carries evidence of
//! independent validation (B3.7), which its own validation state says; a model that a recording produced is kept in the
//! Identify section with the recordings it came from.
//!
//! The analysis is the model's frequency response on a logarithmic grid, its poles, and the optional gains and structure
//! the model allows, and, when the engineer asked for it, the open-loop time response. The checks are those of
//! `isSavedGeneralModelAnalysis`: an analysis is kept only when it was made for this very model, every channel has one
//! point per grid frequency, and every number is finite. Two rules follow the numerical annex where the `TypeScript` check
//! did not: the stability flag follows the poles (N4.2), and a discrete model's grid stays below its Nyquist frequency
//! (N4.3).
//!
//! The result types are those of `generalModel.ts` (`GeneralModelFrequencyResult`, the qualified variant, which is the
//! only one that is stored); the frequency analysis that makes them moves with stage V2.
//!
//! Not kept here, each exists only for a feature that is parked with stage V7, and a saved analysis that carries one is
//! refused rather than read without it: the pole-zero cancellation view (`poleView`), the feedback analysis with a
//! controller (`feedback`) and the periodic frequency estimate (`periodic`).

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::model::Timing;

use crate::failure::{Cause, Failure, NextAction};
use crate::model::{
    is_general_state_structure, is_model_time_response, GeneralModel, GeneralStateStructure,
    ModelChannel, ModelSource, ModelTimeResponse,
};
use crate::study::{is_valid_model, StudySection};

/// The fewest points of a frequency grid (N4.3).
const MINIMUM_GRID_POINTS: u32 = 2;

/// The most points of a frequency grid (N4.3).
const MAXIMUM_GRID_POINTS: u32 = 512;

/// The most channel-frequency points of one analysis: channels times grid points (N4.3).
const MAXIMUM_CHANNEL_FREQUENCY_POINTS: u64 = 16_384;

/// How far a stored frequency may lie from the grid's, as a share of the larger of the frequency and 1 (the logarithmic
/// spacing is computed with a power, which rounds).
const FREQUENCY_TOLERANCE: f64 = 1e-10;

/// The frequencies an analysis was made at (`ModelFrequencyGrid`): `points` frequencies from `minimum_frequency` to
/// `maximum_frequency` in rad/s, spaced evenly on a logarithmic scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelFrequencyGrid {
    /// The lowest frequency in rad/s: positive.
    pub minimum_frequency: f64,
    /// The highest frequency in rad/s: above the lowest.
    pub maximum_frequency: f64,
    /// The number of frequencies: 2 to 512.
    pub points: u32,
}

impl ModelFrequencyGrid {
    /// Whether the grid is valid for a model with `channels` channel pairs (`validModelGrid`, N4.3): finite, a positive
    /// increasing range, 2 to 512 points, and at most 16,384 channel-frequency points in all.
    #[must_use]
    pub fn is_valid(&self, channels: usize) -> bool {
        self.minimum_frequency.is_finite()
            && self.maximum_frequency.is_finite()
            && self.minimum_frequency > 0.0
            && self.maximum_frequency > self.minimum_frequency
            && (MINIMUM_GRID_POINTS..=MAXIMUM_GRID_POINTS).contains(&self.points)
            && u64::try_from(channels)
                .ok()
                .and_then(|channels| u64::from(self.points).checked_mul(channels))
                .is_some_and(|total| total <= MAXIMUM_CHANNEL_FREQUENCY_POINTS)
    }

    /// The frequency of point `index` (`frequencyAt`): `minimum · (maximum / minimum)^(index / (points − 1))`, and exactly
    /// `maximum` for the last point.
    #[must_use]
    pub fn frequency_at(&self, index: u32) -> f64 {
        let last = self.points.saturating_sub(1);
        if index == last {
            self.maximum_frequency
        } else {
            self.minimum_frequency
                * (self.maximum_frequency / self.minimum_frequency)
                    .powf(f64::from(index) / f64::from(last))
        }
    }
}

/// One point of a channel's frequency response (`GeneralFrequencyPoint`): the complex value, and its magnitude and phase.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrequencyPoint {
    /// The angular frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The real part of the response.
    pub real: f64,
    /// The imaginary part of the response.
    pub imaginary: f64,
    /// The magnitude of the response.
    pub magnitude: f64,
    /// The phase of the response in radians, continuous in frequency.
    pub phase_radians: f64,
}

impl FrequencyPoint {
    /// Whether every number of the point is finite (N4.3: a point is never nonfinite).
    fn is_finite(&self) -> bool {
        [
            self.frequency_rad_per_second,
            self.real,
            self.imaginary,
            self.magnitude,
            self.phase_radians,
        ]
        .into_iter()
        .all(f64::is_finite)
    }
}

/// The response of one input-output pair on the grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelFrequencyResponse {
    /// The input of the pair: one of the model's inputs.
    pub input: ModelChannel,
    /// The output of the pair: one of the model's outputs.
    pub output: ModelChannel,
    /// One point per grid frequency.
    pub points: Vec<FrequencyPoint>,
}

/// A pole of the model: a root of its characteristic polynomial, or an eigenvalue of its state matrix.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPole {
    /// The real part.
    pub real: f64,
    /// The imaginary part.
    pub imaginary: f64,
}

/// Whether an analysis was available, and why not when it was not (`capabilities`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    /// Whether the analysis is available for the model.
    pub available: bool,
    /// Why it is not, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The analyses a model allows (`capabilities`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrequencyCapabilities {
    /// The relative gain array.
    pub relative_gain: Capability,
    /// The step response.
    pub step_response: Capability,
    /// The conversion to a transfer function.
    pub transfer_conversion: Capability,
}

/// The qualified result of a model's frequency analysis (`GeneralModelFrequencyResult`, the variant `qualified`). From
/// `generalModel.ts`; moves with stage V2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneralModelFrequencyResult {
    /// The model the analysis was made for.
    pub model: GeneralModel,
    /// One response per input-output pair, the inputs running fastest: pair `i` is input `i mod n` of output `i div n`.
    pub channels: Vec<ChannelFrequencyResponse>,
    /// The model's poles; a pure delay is not among them.
    pub poles: Vec<ModelPole>,
    /// Whether every pole is stable (N4.2): continuous, a negative real part; discrete, a magnitude below one.
    pub stable: bool,
    /// What the poles do and do not say.
    pub pole_scope: String,
    /// The relative gain array, outputs by inputs, when the model allows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_gain_matrix: Option<DenseMatrix>,
    /// The controllability and observability structure of a state-space model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_structure: Option<GeneralStateStructure>,
    /// The analyses the model allows.
    pub capabilities: FrequencyCapabilities,
}

impl GeneralModelFrequencyResult {
    /// The result is for `model`, with a response for every pair on `grid`, poles that agree with the stability flag, and
    /// the optional parts in the shape of the model.
    fn check(&self, model: &GeneralModel, grid: &ModelFrequencyGrid) -> Result<(), Failure> {
        if self.model != *model {
            return Err(refuse("its result was made for another model"));
        }
        self.check_channels(model, grid)?;
        self.check_poles(model)?;
        let identity = model.identity();
        if let Some(matrix) = &self.relative_gain_matrix {
            let shaped = matrix.len() == identity.outputs.len()
                && matrix.iter().all(|row| {
                    row.len() == identity.inputs.len() && row.iter().copied().all(f64::is_finite)
                });
            if !shaped {
                return Err(refuse(
                    "the relative gain array is not outputs by inputs finite numbers",
                ));
            }
        }
        if let Some(structure) = &self.state_structure {
            let fits = !matches!(model, GeneralModel::TransferFunction { .. })
                && serde_json::to_value(structure)
                    .is_ok_and(|value| is_general_state_structure(&value, model));
            if !fits {
                return Err(refuse(
                    "the state structure does not belong to the state-space model",
                ));
            }
        }
        Ok(())
    }

    /// Every pair of the model has its response: the model's input and output in the order of the pairs, and one finite
    /// point at each frequency of the grid.
    fn check_channels(
        &self,
        model: &GeneralModel,
        grid: &ModelFrequencyGrid,
    ) -> Result<(), Failure> {
        let identity = model.identity();
        let input_count = identity.inputs.len();
        if self.channels.len() != input_count * identity.outputs.len() {
            return Err(refuse(
                "its result has not one response per input-output pair",
            ));
        }
        for (index, channel) in self.channels.iter().enumerate() {
            let pair = (
                identity.inputs.get(index % input_count),
                identity.outputs.get(index / input_count),
            );
            if pair != (Some(&channel.input), Some(&channel.output)) {
                return Err(refuse("a response is not for its input-output pair"));
            }
            if u32::try_from(channel.points.len()).ok() != Some(grid.points) {
                return Err(refuse("a response has not one point per grid frequency"));
            }
            for (point, position) in channel.points.iter().zip(0_u32..) {
                let expected = grid.frequency_at(position);
                let off_grid = (point.frequency_rad_per_second - expected).abs()
                    > FREQUENCY_TOLERANCE * expected.max(1.0);
                if !point.is_finite() || off_grid {
                    return Err(refuse(
                        "a response point is not finite or not at its grid frequency",
                    ));
                }
            }
        }
        Ok(())
    }

    /// The poles are finite, and the stability flag follows them (N4.2).
    fn check_poles(&self, model: &GeneralModel) -> Result<(), Failure> {
        if !self
            .poles
            .iter()
            .all(|pole| pole.real.is_finite() && pole.imaginary.is_finite())
        {
            return Err(refuse("a pole is not finite"));
        }
        let discrete = matches!(model, GeneralModel::DiscreteStateSpace { .. });
        let stable = self.poles.iter().all(|pole| {
            if discrete {
                pole.real.hypot(pole.imaginary) < 1.0
            } else {
                pole.real < 0.0
            }
        });
        if stable != self.stable {
            return Err(refuse(
                "the stability flag does not follow the poles (N4.2)",
            ));
        }
        Ok(())
    }
}

/// The analysis of a supplied model that the study saved: the grid, the frequency result on it, and optionally the
/// open-loop time response (`SavedGeneralModelAnalysis`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuppliedModelAnalysis {
    /// The frequencies the result was made at.
    pub grid: ModelFrequencyGrid,
    /// The frequency result.
    pub result: GeneralModelFrequencyResult,
    /// The open-loop time response of the model, when the engineer asked for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<ModelTimeResponse>,
}

impl SuppliedModelAnalysis {
    /// The analysis is for `model` (`isSavedGeneralModelAnalysis`).
    fn check(&self, model: &GeneralModel) -> Result<(), Failure> {
        let identity = model.identity();
        if !self
            .grid
            .is_valid(identity.inputs.len() * identity.outputs.len())
        {
            return Err(refuse(
                "the frequency grid needs 2 to 512 points, a positive increasing range and at most 16384 channel-frequency points (N4.3)",
            ));
        }
        if let GeneralModel::DiscreteStateSpace {
            sample_time_seconds,
            ..
        } = model
            && self.grid.maximum_frequency >= PI / *sample_time_seconds
        {
            return Err(refuse(
                "the frequencies of a discrete model stay below its Nyquist frequency (N4.3)",
            ));
        }
        self.result.check(model, &self.grid)?;
        if let Some(response) = &self.response {
            let computed = serde_json::to_value(response)
                .is_ok_and(|value| is_model_time_response(&value, model));
            if !computed {
                return Err(refuse("its time response was not computed for the model"));
            }
        }
        Ok(())
    }
}

/// The model the engineer supplied, with its saved analysis, kept in the study under the key `suppliedModel`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuppliedModelSection {
    /// The model. It was not fitted from a recording; it is provisional unless it carries evidence of independent
    /// validation (B3.7).
    pub model: GeneralModel,
    /// The analysis of the model, when it was calculated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<SuppliedModelAnalysis>,
}

impl SuppliedModelSection {
    /// The section of a supplied `model` and its `analysis`, if the model was analysed.
    #[must_use]
    pub fn new(model: GeneralModel, analysis: Option<SuppliedModelAnalysis>) -> Self {
        Self { model, analysis }
    }
}

impl StudySection for SuppliedModelSection {
    const KEY: &'static str = "suppliedModel";

    fn check(&self) -> Result<(), Failure> {
        if !is_valid_model(&self.model) {
            return Err(refuse(
                "the model breaks the model contract (revision, evidence, channels, delay, operating point or coefficients)",
            ));
        }
        if self.model.identity().source == ModelSource::Fitted {
            return Err(refuse(
                "a model fitted from a recording is kept with the identification that produced it (B3.1)",
            ));
        }
        self.analysis
            .as_ref()
            .map_or(Ok(()), |analysis| analysis.check(&self.model))
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

/// What the page shows of a supplied model (B3.7, B9.4): its type in plain words and its equation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppliedModelWords {
    /// The model type, for example "Transfer function, continuous time".
    pub model_type: String,
    /// The equation with its coefficients, for example "G(s) = 2 / (5s + 1), delay 1 s".
    pub equation: String,
}

/// The words of a supplied model, read from its coefficients (highest power first), its delay and its time base. A transfer
/// function is written in `s` when it is continuous and in `z` when it is discrete. A state-space model is named by its
/// number of states, because its matrices are the model's own.
#[must_use]
pub fn supplied_model_words(model: &GeneralModel) -> SuppliedModelWords {
    let delay = model.identity().delay_seconds;
    let delay_text = if delay > 0.0 {
        format!(", delay {delay} s")
    } else {
        String::new()
    };
    let (model_type, equation) = match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            let variable = match model.timing() {
                Timing::Continuous => "s",
                Timing::Discrete { .. } => "z",
            };
            (
                format!("Transfer function, {}", timing_words(model)),
                format!(
                    "G({variable}) = {} / {}{delay_text}",
                    bracketed(&polynomial_text(&transfer.numerator, variable)),
                    bracketed(&polynomial_text(&transfer.denominator, variable)),
                ),
            )
        }
        GeneralModel::ContinuousStateSpace { states, .. } => (
            format!("State-space model, {}", timing_words(model)),
            format!(
                "dx/dt = A x + B u, y = C x + D u, with {} states{delay_text}",
                states.len()
            ),
        ),
        GeneralModel::DiscreteStateSpace { states, .. } => (
            format!("State-space model, {}", timing_words(model)),
            format!(
                "x(k+1) = A x(k) + B u(k), y(k) = C x(k) + D u(k), with {} states{delay_text}",
                states.len()
            ),
        ),
    };
    SuppliedModelWords {
        model_type,
        equation,
    }
}

/// The time base of a model in words: continuous time, or discrete time with its sample time.
fn timing_words(model: &GeneralModel) -> String {
    match model.timing() {
        Timing::Continuous => "continuous time".to_string(),
        Timing::Discrete {
            sample_time_seconds,
        } => format!("discrete time, sample time {sample_time_seconds} s"),
    }
}

/// A polynomial in `variable` with coefficients from the highest power down, such as `5s + 1`. Zero terms are left out, and a
/// coefficient of one is not written.
fn polynomial_text(coefficients: &[f64], variable: &str) -> String {
    let last = coefficients.len().saturating_sub(1);
    let mut text = String::new();
    for (index, &coefficient) in coefficients.iter().enumerate() {
        let magnitude = coefficient.abs();
        if magnitude <= 0.0 {
            continue;
        }
        let power = last - index;
        let body = match (power, is_one(magnitude)) {
            (0, _) => format!("{magnitude}"),
            (1, true) => variable.to_string(),
            (1, false) => format!("{magnitude}{variable}"),
            (_, true) => format!("{variable}^{power}"),
            (_, false) => format!("{magnitude}{variable}^{power}"),
        };
        if !text.is_empty() {
            text.push_str(if coefficient < 0.0 { " - " } else { " + " });
        } else if coefficient < 0.0 {
            text.push('-');
        }
        text.push_str(&body);
    }
    if text.is_empty() {
        "0".to_string()
    } else {
        text
    }
}

/// The text in brackets when it has more than one term.
fn bracketed(text: &str) -> String {
    if text.contains(' ') {
        format!("({text})")
    } else {
        text.to_string()
    }
}

fn is_one(value: f64) -> bool {
    (value - 1.0).abs() < f64::EPSILON
}

fn refuse(reason: &str) -> Failure {
    Failure::new(
        &format!("The supplied model of the study is not valid: {reason}."),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "general_model_persistence_tests.rs"]
mod tests;
