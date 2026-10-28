//! The loop's bindings: its signals, limits, sample timing, parameter record and identification
//! experiment (specification 35, §17.1, A5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The controller's parameter record, as `control.parameters.*` writes it (protocol annex, E7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParameterBinding {
    /// The loop the binding belongs to.
    pub loop_id: String,
    /// The task that owns the record.
    pub task_name: String,
    /// The record's numeric fields.
    pub parameters: Vec<Parameter>,
    /// The mode variable, and its Manual value.
    pub mode: ModeBinding,
}

/// One numeric field of the parameter record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    /// The field's path.
    pub symbol: String,
    /// `REAL` or `LREAL`.
    pub iec_type: RealType,
    /// The field's unit.
    pub unit: String,
    /// What the field means, for example `proportional gain`.
    pub convention: String,
    /// The smallest value Tune may choose.
    pub minimum: f64,
    /// The largest value Tune may choose.
    pub maximum: f64,
}

/// The mode variable of a parameter binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModeBinding {
    /// The mode variable's path.
    pub symbol: String,
    /// The value that means Manual.
    pub manual_value: bool,
}

/// A real IEC type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum RealType {
    /// `REAL`.
    Real,
    /// `LREAL`.
    Lreal,
}

/// What the bound parameters are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ParameterPurpose {
    /// The controller's settings.
    Controller,
    /// The settings of the project's own experiment.
    Experiment,
}

/// The loop's topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Topology {
    /// The controller acts on its measurement.
    ClosedLoop,
    /// An input/output binding without a setpoint.
    OpenLoop,
}

/// The loop's signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Signals {
    /// The setpoint; absent in an open loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<LoopSignal>,
    /// The measurement.
    pub process_variable: LoopSignal,
    /// The controller's output.
    pub manipulated_variable: LoopSignal,
    /// The actuator's measured position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actuator: Option<LoopSignal>,
    /// Further signals of the loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional: Option<Vec<AuxiliarySignal>>,
}

/// One signal of the loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoopSignal {
    /// The variable's path.
    pub symbol: String,
    /// The variable's address; absent for a program variable without one (§17.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The variable's IEC type.
    pub iec_type: String,
    /// Whether Control Studio reads or writes it.
    pub direction: Direction,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
    /// The signal's range.
    pub range: Range,
}

/// A further signal of the loop, in a channel and with a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuxiliarySignal {
    /// The signal's identifier in the loop.
    pub id: String,
    /// The channel it belongs to.
    pub channel: String,
    /// Its role in the channel.
    pub role: AuxiliaryRole,
    /// The variable's path.
    pub symbol: String,
    /// The variable's address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The variable's IEC type.
    pub iec_type: String,
    /// Whether Control Studio reads or writes it.
    pub direction: Direction,
    /// The signal's unit.
    pub unit: String,
    /// The signal's range.
    pub range: Range,
}

/// The role of a further signal.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum AuxiliaryRole {
    /// A setpoint.
    #[serde(rename = "SP")]
    Setpoint,
    /// A measurement.
    #[serde(rename = "PV")]
    ProcessVariable,
    /// An output.
    #[serde(rename = "MV")]
    ManipulatedVariable,
    /// An actuator position.
    #[serde(rename = "actuator")]
    Actuator,
    /// A disturbance.
    #[serde(rename = "disturbance")]
    Disturbance,
    /// A state.
    #[serde(rename = "state")]
    State,
    /// A quality flag.
    #[serde(rename = "quality")]
    Quality,
    /// A schedule variable.
    #[serde(rename = "schedule")]
    Schedule,
}

/// Whether Control Studio reads or writes a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Read only.
    Read,
    /// Written by Control Studio.
    Write,
}

/// A signal's range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Range {
    /// The smallest value.
    pub minimum: f64,
    /// The largest value.
    pub maximum: f64,
}

/// The output's limits. A loop file may omit an output limit or the expected settling time: the loop still loads, and
/// Identify refuses to start until the missing limit is set, naming it (B1.4, B3.4 step 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    /// The smallest output, when the file sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_minimum: Option<f64>,
    /// The largest output, when the file sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_maximum: Option<f64>,
    /// The largest rise of the output, per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_up_per_s: Option<f64>,
    /// The largest fall of the output, per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_down_per_s: Option<f64>,
    /// The expected settling time, in s, when the loop declares one. The baseline window and the record period follow it
    /// (B3.4 step 2, A4.2); when present it is positive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_settling_seconds: Option<f64>,
}

/// How often the loop's values are observed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SampleTiming {
    /// The nominal period, in ms.
    pub nominal_period_ms: f64,
    /// The largest jitter, in ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_jitter_ms: Option<f64>,
}

/// The loop's identification experiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Experiment {
    /// Who runs the experiment.
    pub execution: Execution,
    /// The experiment's kind.
    pub kind: ExperimentKind,
    /// The step's or relay's amplitude; 0 for an observation.
    pub amplitude: f64,
    /// A relay's first direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_direction: Option<RelayDirection>,
    /// The experiment's duration, in s, when the file sets it. Identify names it while it is missing (B3.4 step 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// A project experiment's baseline before its activation, in s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_seconds: Option<f64>,
    /// The experiment parameter, in s, whose applied value sets how long a project step is held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_parameter: Option<String>,
    /// The seed of a pseudo-random excitation, 1 to 2³² − 1.
    pub seed: u64,
    /// A project experiment's activation variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<Activation>,
    /// A project experiment's abort variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort: Option<Activation>,
}

/// Who runs the experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Execution {
    /// Control Studio applies the experiment.
    ControlStudio,
    /// The project runs its own experiment; Control Studio only observes.
    ProjectScripted,
}

/// The kind of an identification experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentKind {
    /// Observation only.
    Observation,
    /// A setpoint step.
    SetpointStep,
    /// A load step.
    LoadStep,
    /// A step of the output.
    MvStep,
    /// A pulse of the output.
    MvPulse,
    /// A relay.
    Relay,
    /// A pseudo-random binary sequence.
    Prbs,
    /// A multisine.
    Multisine,
}

/// A relay's first direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RelayDirection {
    /// Upwards first.
    Positive,
    /// Downwards first.
    Negative,
}

/// A BOOL variable Control Studio writes to activate or abort a project experiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Activation {
    /// The variable's path.
    pub symbol: String,
    /// The variable's address.
    pub address: String,
    /// Always `BOOL`.
    pub iec_type: String,
    /// Always written.
    pub direction: Direction,
    /// The value that asserts an abort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asserted_value: Option<bool>,
}
