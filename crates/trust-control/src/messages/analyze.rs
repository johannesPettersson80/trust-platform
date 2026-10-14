//! The wire types of `controlStudio/analyze` (specification 35, §0.1a and B9.9; draft contract §2): the request, the tool list,
//! the envelope of a calculation and the answer of each tool. Requests are plain structs, so that they decode with
//! `crate::json` whatever `serde_json`'s features are; answers are only written. `messages.rs` re-exports them.
//!
//! The tools of B2 (`rootLocus`, `routh`, `feedback` and `measuredFrequency`) have their answer types in `analyze_declared.rs`.

// The wire names of this module are the analysis names: `AnalyzeAnswer` and `AnalyzeRequest` repeat the module's name, as the request names do.
#![allow(clippy::module_name_repetitions)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::linalg::ComplexValue;
use trust_control_math::simulate::TimeResponsePoint;

use crate::analysis::general_feedback::ChannelPair;
use crate::analysis::lead_lag::{LeadLagMode, LeadLagResult};
use crate::failure::Failure;
use crate::model::{GeneralStateStructure, ModelTimeSettings, ResponseConnection};

use super::analyze_declared::{
    FeedbackAnswer, MeasuredFrequencyAnswer, RootLocusAnswer, RouthAnswer,
};

/// One analysis tool of the Tune page (B9.9). The wire names are the camelCase words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisTool {
    /// The declared root locus (an operation).
    RootLocus,
    /// The declared Routh–Hurwitz analysis (an operation).
    Routh,
    /// The pole-zero map (a quick answer).
    PoleZero,
    /// The Bode frequency response (an operation).
    Frequency,
    /// The measured frequency response of a recording (an operation).
    MeasuredFrequency,
    /// The Nyquist and loop response with a controller (an operation).
    Feedback,
    /// The lead/lag design (a quick answer).
    LeadLag,
    /// The initial-state response (an operation).
    InitialState,
    /// The controllability and observability structure (a quick answer).
    StateStructure,
    /// The relative gain array (a quick answer).
    RelativeGain,
}

/// Which pole-zero view is shown (B9.9, `poleZero`): the unreduced model, or the input/output view with its exact common factors
/// removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PoleZeroView {
    /// The model as declared, with every internal mode.
    Unreduced,
    /// The input/output view: exact common factors removed. Needs one input and one output.
    Minimal,
}

/// A frequency grid (N4.3): the lowest and the highest frequency in rad/s, and the number of logarithmically spaced points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrequencyGrid {
    /// The lowest frequency in rad/s; positive.
    pub minimum_frequency: f64,
    /// The highest frequency in rad/s; above the lowest.
    pub maximum_frequency: f64,
    /// The number of frequencies: 2 to 512.
    pub points: u32,
}

/// The inputs of every tool, in one flat struct. Each tool reads only its own fields (draft §2.3); the envelope's `inputs` lists
/// the fields the tool read, with the defaults filled in. Every field is optional in a request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisInputs {
    /// `poleZero`: the view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PoleZeroView>,
    /// `frequency` and `feedback`: the grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<FrequencyGrid>,
    /// `feedback`: the plant's input and output in the loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_channel: Option<ChannelPair>,
    /// `feedback`: the controller's input and output in the loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller_channel: Option<ChannelPair>,
    /// `rootLocus` and `routh`: the selected value of the declared parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_value: Option<f64>,
    /// `measuredFrequency`: the recording, by its identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// `measuredFrequency`: the revision of the recording the request was made for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_revision: Option<String>,
    /// `measuredFrequency`: the tone frequencies in rad/s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequencies: Option<Vec<f64>>,
    /// `measuredFrequency`: the number of segments, at least 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_count: Option<u32>,
    /// `measuredFrequency`: the minimum coherence, 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_coherence: Option<f64>,
    /// `measuredFrequency`: the startup samples left out of the estimate, at least 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_samples: Option<u32>,
    /// `leadLag`: the compensator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<LeadLagMode>,
    /// `leadLag`: the crossover in rad/s, above 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossover_rad_per_second: Option<f64>,
    /// `leadLag`: the phase in degrees for a lead, or the low-frequency gain ratio for a lag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// `initialState`: the output step in seconds, above 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_seconds: Option<f64>,
    /// `initialState`: the duration in seconds, a whole number of steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// `initialState`: the initial state, one value per state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<Vec<f64>>,
    /// `initialState`: the held input, one value per input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<f64>>,
    /// `initialState`: open loop, or through the feedback with a controller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<ResponseConnection>,
}

/// The arguments of `controlStudio/analyze` (§0.1a). `tool` absent asks for the tools that apply; `model` and `controller` are
/// general models (N4.1) as JSON, at most 256,000 bytes; without them the study's model is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzeRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code; a loop of another runtime is refused.
    pub runtime_id: String,
    /// The loop to analyse.
    pub loop_id: String,
    /// The tool to calculate; absent asks for the tools that apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<AnalysisTool>,
    /// The tool's inputs; every field is optional.
    #[serde(default)]
    pub inputs: AnalysisInputs,
    /// The model to analyse, as a general model (N4.1); the study's model when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<Value>,
    /// The controller for `feedback` (and for `initialState` through the feedback), as a general model; the loop's declared law
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<Value>,
}

/// The page the engineer views with an analysis tool (§0.1a, `controlStudio/page`; A8.1): the tool shown and its inputs, so that
/// reopening shows them again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisPage {
    /// The tool shown.
    pub tool: AnalysisTool,
    /// Its inputs, as the engineer entered them.
    #[serde(default)]
    pub inputs: AnalysisInputs,
}

/// One tool that applies to the model and the loop, as the page offers it (B9.9).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolOffer {
    /// The tool.
    pub tool: AnalysisTool,
    /// The name of the tool in the page.
    pub label: String,
    /// The text of the button that calculates it.
    pub action: String,
    /// The text of the button that aborts it; given for operations only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abort_action: Option<String>,
    /// Whether the tool is an operation (its answer follows through `controlStudio/update`).
    pub operation: bool,
    /// The inputs the tool takes when the request gives none.
    pub defaults: AnalysisInputs,
}

/// A model named by its revision (N4.1): the envelope names the model and the controller it used, and does not echo them.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelRevision {
    /// The model's revision.
    pub revision: String,
}

/// The envelope of one calculation (§0.1a): the tool, the inputs the engine read with their defaults, the defaults, and either
/// the result or the failure of the library's refusal (A11).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisEnvelope {
    /// The tool calculated.
    pub tool: AnalysisTool,
    /// The inputs the engine read, with the defaults filled in; only the fields of this tool.
    pub inputs: AnalysisInputs,
    /// The defaults of this tool for this model.
    pub defaults: AnalysisInputs,
    /// The model analysed, by its revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRevision>,
    /// The controller used, by its revision, for `feedback` and for `initialState` through the feedback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<ModelRevision>,
    /// The tool's answer, when the calculation qualified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AnalysisResult>,
    /// The library's refusal, when the calculation did not qualify.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

/// The answer of one tool. Each variant holds that tool's answer, with `summary` (sentences for the result area, warnings first)
/// and `details` (sentences for the details disclosure).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisResult {
    /// `rootLocus` (B2).
    RootLocus(RootLocusAnswer),
    /// `routh` (B2).
    Routh(RouthAnswer),
    /// `poleZero`.
    PoleZero(PoleZeroAnswer),
    /// `frequency`.
    Frequency(FrequencyAnswer),
    /// `measuredFrequency` (B2).
    MeasuredFrequency(MeasuredFrequencyAnswer),
    /// `feedback` (B2).
    Feedback(FeedbackAnswer),
    /// `leadLag`.
    LeadLag(LeadLagAnswer),
    /// `initialState`.
    InitialState(InitialStateAnswer),
    /// `stateStructure`.
    StateStructure(StateStructureAnswer),
    /// `relativeGain`.
    RelativeGain(RelativeGainAnswer),
}

/// The answer of `controlStudio/analyze` (its JSON-RPC `result`): the tools that apply, a calculation's envelope, or the started
/// operation. It is written untagged, so each form has the shape the specification names; it is never read back.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum AnalyzeAnswer {
    /// The tools that apply, when the request named no tool.
    Tools {
        /// The offers, in the order the page lists them.
        tools: Vec<ToolOffer>,
    },
    /// A calculation that was answered at once, qualified or refused.
    Calculation(Box<AnalysisEnvelope>),
    /// An operation that was started; its envelope follows as the final update.
    Started {
        /// The operation's identifier.
        #[serde(rename = "operationId")]
        operation_id: String,
    },
}

/// The domain of the poles: `s` for a continuous model, `z` for a discrete one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PoleDomain {
    /// Continuous time: the Laplace variable `s`.
    S,
    /// Discrete time: the variable `z`.
    Z,
}

/// The numerator and the denominator of a transfer function, from the highest power down.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransferPolynomials {
    /// The numerator coefficients.
    pub numerator: Vec<f64>,
    /// The denominator coefficients.
    pub denominator: Vec<f64>,
}

/// The answer of `poleZero` (B9.9, N7.14's sibling rule): the poles and zeros of the view, and for a single-input, single-output
/// model the polynomials of both views and the cancelled factor.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PoleZeroAnswer {
    /// The domain of the poles.
    pub domain: PoleDomain,
    /// The view shown.
    pub view: PoleZeroView,
    /// The poles of the view.
    pub poles: Vec<ComplexValue>,
    /// The zeros of the view; empty for a state-space model with more than one input or output.
    pub zeros: Vec<ComplexValue>,
    /// Whether a cancelled pole lies in the unstable region (at or past the boundary).
    pub hidden_unstable_mode: bool,
    /// The unreduced transfer polynomials, for a single-input, single-output model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreduced: Option<TransferPolynomials>,
    /// The minimal input/output transfer polynomials, for a single-input, single-output model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal: Option<TransferPolynomials>,
    /// The monic common factor that the minimal view removed, for a single-input, single-output model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_factor: Option<Vec<f64>>,
    /// The sentences for the result area, warnings first.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}

/// A channel of a model: its identifier and its unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRef {
    /// The identifier of the channel.
    pub id: String,
    /// The unit of the channel's values.
    pub unit: String,
}

/// One point of a channel's frequency response (N4.3): the complex value, its magnitude in dB and its phase in radians and degrees.
/// The phase is the principal value in `(−180°, 180°]` (N4.3: plotted phase is wrapped).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyPoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The real part.
    pub real: f64,
    /// The imaginary part.
    pub imaginary: f64,
    /// The magnitude.
    pub magnitude: f64,
    /// The magnitude in dB, `20·log10` of the magnitude, floored at the smallest positive number.
    pub magnitude_db: f64,
    /// The phase in radians, principal value.
    pub phase_radians: f64,
    /// The phase in degrees, principal value.
    pub phase_degrees: f64,
}

/// The response of one input-output pair on the grid.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyChannel {
    /// The input of the pair.
    pub input: ChannelRef,
    /// The output of the pair.
    pub output: ChannelRef,
    /// One point per grid frequency.
    pub points: Vec<FrequencyPoint>,
}

/// The answer of `frequency` (B9.9; N4.2, N4.3): the grid as used, the poles and zeros, the stability of the poles and the response
/// of every input-output pair, inputs running fastest.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyAnswer {
    /// The grid the response was calculated on, as used.
    pub grid: FrequencyGrid,
    /// The poles of the model: its transfer denominator's roots, or the eigenvalues of its state matrix.
    pub poles: Vec<ComplexValue>,
    /// The zeros of a transfer function; empty for a state-space model.
    pub zeros: Vec<ComplexValue>,
    /// Whether every pole is stable (N4.2).
    pub stable: bool,
    /// What the poles do and do not say (N4.2: a pure delay is not among them).
    pub pole_scope: String,
    /// The response of every input-output pair.
    pub channels: Vec<FrequencyChannel>,
    /// The sentences for the result area, warnings first.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}

/// The answer of `leadLag` (B9.9): the compensator's figures (`design`), with its formula as the first sentence of `summary`.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LeadLagAnswer {
    /// The designed compensator.
    pub design: LeadLagResult,
    /// The sentences for the result area, warnings first.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}

/// The answer of `initialState` (B9.9, N4.0): the response's settings, its state names, its transform and its scope, and its samples.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitialStateAnswer {
    /// The settings the response was calculated with, with their connection.
    pub settings: ModelTimeSettings,
    /// The name of each state, in order.
    pub state_coordinates: Vec<String>,
    /// The transform equation of the response.
    pub transform: String,
    /// The scope sentence of the response.
    pub scope: String,
    /// The samples at `0, T, …, duration`.
    pub points: Vec<TimeResponsePoint>,
    /// The model's outputs, in the order of each point's values, with their units: the page names the output it draws.
    pub outputs: Vec<ChannelRef>,
    /// One sentence per output, and the scope.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}

/// A named matrix of a state structure, its rows in the declared order.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NamedMatrix {
    /// The name of the matrix, as the page writes it (`A`, `B`, `C`, `D`, the controllability and observability matrices).
    pub name: String,
    /// The rows of the matrix.
    pub rows: Vec<Vec<f64>>,
}

/// The answer of `stateStructure` (B9.9): the controllability and observability structure, the state order and the matrices.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateStructureAnswer {
    /// The structure, qualified with its ranks and matrices, or unavailable with its reason.
    pub structure: GeneralStateStructure,
    /// The order of the states, inputs and outputs, as one sentence.
    pub order: String,
    /// The model's matrices and the structure matrices; empty when the structure is unavailable.
    pub matrices: Vec<NamedMatrix>,
    /// The sentences for the result area.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}

/// The status of the relative gain array (N4.5).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum RelativeGainStatus {
    /// The array qualified: the matrix, outputs by inputs.
    Qualified {
        /// The relative gain array, outputs by inputs.
        matrix: Vec<Vec<f64>>,
    },
    /// The array is not available, for the reason given.
    Unavailable {
        /// The sentence that says why.
        reason: String,
    },
}

/// The answer of `relativeGain` (B9.9, N4.5): the array with its channel names and units.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelativeGainAnswer {
    /// The inputs, in the model's order.
    pub input_channels: Vec<ChannelRef>,
    /// The outputs, in the model's order.
    pub output_channels: Vec<ChannelRef>,
    /// The array, or why it is not available.
    pub status: RelativeGainStatus,
    /// The sentences for the result area.
    pub summary: Vec<String>,
    /// The sentences for the details disclosure.
    pub details: Vec<String>,
}
