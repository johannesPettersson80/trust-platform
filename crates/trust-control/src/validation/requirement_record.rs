//! The wire types of a validation requirement (specification 35, B5.4; numerics N8.3): the window of a requirement, the
//! calculation of each kind with its limits, the reference of an error, and the verdict on one requirement. They are stored
//! and sent as `camelCase` JSON, and their enums use the `TypeScript` words. The evaluation of a run against them is in
//! `run_validation`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::identification::independent_validation::RequirementStatus;

/// A requirement's window, in seconds of the run: it starts at or after 0 and ends after it starts (N8.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Window {
    /// The start of the window, in seconds.
    pub start: f64,
    /// The end of the window, in seconds.
    pub end: f64,
}

/// One requirement of a run: its identifier, the channel it measures, its window and its calculation (`RunRequirement`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunRequirement {
    /// The identifier, unique within one evaluation.
    pub id: String,
    /// The channel it measures, by symbol.
    pub channel: String,
    /// The window it measures, in seconds of the run.
    pub window: Window,
    /// The calculation and its limits, tagged by `kind`.
    #[serde(flatten)]
    pub check: RunCheck,
}

/// The calculation of a requirement and its limits, tagged by `kind` with the `TypeScript` words (`RunRequirement`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RunCheck {
    /// The error between the channel and a reference, measured by one statistic (N8.3).
    Error {
        /// The reference: a constant, or another channel of the run with the same unit.
        reference: ErrorReference,
        /// The statistic of the error that is compared with the maximum.
        statistic: ErrorStatistic,
        /// The largest accepted value of the statistic.
        maximum: f64,
    },
    /// The overshoot beyond a step from `initial` to `target`, in the step's direction (N8.3).
    Overshoot {
        /// The value before the step.
        initial: f64,
        /// The value the step aims at.
        target: f64,
        /// The largest accepted overshoot.
        maximum: f64,
    },
    /// The settling into a band around `target`, reached by the deadline and held for `hold_seconds` (N8.3).
    #[serde(rename_all = "camelCase")]
    Settling {
        /// The target of the band.
        target: f64,
        /// The half-width of the band.
        band: f64,
        /// The time that the response must stay in the band, in seconds.
        hold_seconds: f64,
        /// The latest arrival, in seconds after the window starts.
        deadline_seconds: f64,
    },
    /// The channel stays within its bounds for the whole window (N8.3).
    Bounds {
        /// The smallest accepted value.
        minimum: f64,
        /// The largest accepted value.
        maximum: f64,
    },
    /// The state stays within its bounds for the whole window, measured like `Bounds`.
    StateBounds {
        /// The smallest accepted value.
        minimum: f64,
        /// The largest accepted value.
        maximum: f64,
    },
    /// The channel stays on one of the allowed levels, within the tolerance.
    Levels {
        /// The allowed levels.
        allowed: Vec<f64>,
        /// The largest accepted distance from a level.
        tolerance: f64,
    },
    /// Each completed stay in `state` lasts at least `minimum_seconds`.
    #[serde(rename_all = "camelCase")]
    Dwell {
        /// The level that separates the two states: a value at or above it is high.
        threshold: f64,
        /// The state whose stays are measured.
        state: DwellState,
        /// The shortest accepted stay, in seconds.
        minimum_seconds: f64,
    },
    /// The adjacent-sample rate stays at or below `maximum` per second.
    Rate {
        /// The largest accepted rate, per second.
        maximum: f64,
        /// The direction that counts; absent when either direction counts.
        direction: Option<Movement>,
    },
    /// The time in saturation stays at or below `maximum_seconds` (N8.3).
    #[serde(rename_all = "camelCase")]
    Saturation {
        /// The value at or below which the signal is saturated.
        minimum: f64,
        /// The value at or above which the signal is saturated.
        maximum: f64,
        /// The longest accepted saturation, in seconds.
        maximum_seconds: f64,
    },
    /// The number of changes that strictly exceed `threshold` stays at or below `maximum` (N8.3).
    Switching {
        /// The smallest change that counts as a switch.
        threshold: f64,
        /// The largest accepted number of switches.
        maximum: u64,
    },
    /// The total movement of the channel, compared with `maximum` when there is one.
    Activity {
        /// The largest accepted total movement; absent when the movement is only reported.
        maximum: Option<f64>,
    },
}

/// The reference of an error requirement: a constant, or a reference channel of the run (N8.3, `r_k`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ErrorReference {
    /// A constant reference.
    Constant(f64),
    /// A reference channel of the run, sampled with the measured channel.
    Channel {
        /// The reference channel's symbol.
        channel: String,
    },
}

/// The statistic of an error requirement (N8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ErrorStatistic {
    /// The largest absolute error.
    Maximum,
    /// The mean absolute error.
    Mean,
    /// The root mean square error.
    Rms,
    /// The signed mean error (N8.3 "Mean error"). It is reported with its sign, and it passes when its magnitude is at most
    /// the maximum, so a bias fails in either direction: a one-sided test would pass a large negative bias. The `TypeScript`
    /// has no such statistic.
    MeanError,
}

/// The state whose stays a dwell requirement measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DwellState {
    /// The value is at or above the threshold.
    High,
    /// The value is below the threshold.
    Low,
}

/// The direction of a rate requirement's movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Movement {
    /// A rise.
    Up,
    /// A fall.
    Down,
}

/// The verdict on one requirement: its status, its measured value and limit when it has them, its unit and the sentence
/// that says what was found (B5.4, A11.1). `kind` is the wire kind, such as `state-bounds`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunRequirementResult {
    /// The requirement's identifier.
    pub id: String,
    /// The requirement's wire kind.
    pub kind: String,
    /// Pass, Fail or Inconclusive.
    pub status: RequirementStatus,
    /// The measured value, when one was calculated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The limit that the value is compared with, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// The unit of the value and of the limit.
    pub unit: String,
    /// The sentence that says what was found.
    pub reason: String,
}
