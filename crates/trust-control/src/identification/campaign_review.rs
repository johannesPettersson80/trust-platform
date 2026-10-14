//! The review of an identification campaign (specification 35, §0.1a `controlStudio/identify` with `action: "review"`, B3.2 to
//! B3.4 and C12): the campaign Identify would run, answered before the engineer starts it. The review is given by the loop, the
//! settings the engineer asks for and, when a runtime is connected, one live sample of the loop. It starts nothing and writes
//! nothing.
//!
//! The campaign's own preconditions and rules decide the review (`configured_acquisition`, `require_campaign_limits`,
//! `identification_method`), so the review refuses what the campaign refuses, with the same next action. What the campaign
//! measures before it steps (the steady baseline's noise, B3.3) is not known yet: the amplitude is then shown as the engineer
//! asked for it, and zero means Identify sizes it after the baseline.

#![allow(clippy::module_name_repetitions)] // `campaign_review` names the review it gives.

use schemars::JsonSchema;
use serde::Serialize;

use crate::experiment::{ControlExperimentKind, ExperimentSample};
use crate::failure::Failure;
use crate::identification::acquisition_settings::{configured_acquisition, AcquisitionSettings};
use crate::identification::campaign::{
    campaign_maximum_ms, ms_as_f64, plant_input_range, require_campaign_limits,
};
use crate::identification::policy::{identification_method, project_scripted};
use crate::limits::RESTORATION_CONFIRMATION_EXECUTIONS;
use crate::loop_definition::{Activation, Execution, Experiment, LoopDefinition, Range};

/// The review of the campaign that Identify would run (B3.2 to B3.4, C12).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyReview {
    /// The experiment kind the runtime runs (B3.3): Identify (Automatic) chooses it from the loop's mode and policy.
    pub kind: ControlExperimentKind,
    /// Whether Identify chose the method from the loop's mode and policy (B3.3); false for a method the engineer or the loop names.
    pub automatic: bool,
    /// Whether the campaign writes the plant input in Manual, so the loop changes to Manual and back (A6.2).
    pub manual: bool,
    /// Who runs the experiment: Control Studio, or the project's own scenario (C12).
    pub execution: Execution,
    /// The plant input range the campaign may write (B3.4 step 1).
    pub input_range: Range,
    /// The permitted range of the measured output (B3.4 step 1).
    pub output_range: Range,
    /// The longest the campaign runs, in s: the baseline wait and the maximum duration of the step (A7, B3.4 step 2).
    pub maximum_duration_seconds: f64,
    /// The amplitude the campaign starts from. Zero means Identify sizes the step from the measured noise after the baseline (B3.3).
    pub amplitude: f64,
    /// The project's scenario, for a project-scripted experiment (C12); absent for an experiment Control Studio runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_scripted: Option<ProjectScriptedReview>,
}

/// What a project-scripted experiment writes and asserts (C12): its baseline, its activation and its abort.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScriptedReview {
    /// The steady baseline before the activation, in s (C12); absent when the project declares none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_seconds: Option<f64>,
    /// The activation: the authorized start writes it TRUE, and the restoration writes it FALSE however the experiment ends
    /// (C12). Absent when the scenario is observation only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation: Option<ActivationReview>,
    /// The abort: a failed termination asserts it, and a normal completion does not (C12). Absent without an abort declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abort: Option<AbortReview>,
    /// The restoration of the activation after the experiment ends (C12, A7); absent when the scenario has no activation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restoration: Option<RestorationReview>,
}

/// How the activation of a project-scripted experiment is restored (C12, A7).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestorationReview {
    /// The value the activation is restored to, however the experiment ends: normal completion, abort, timeout or failure (C12).
    pub activation_restored_to: bool,
    /// The restoration is confirmed by this execution of the loop's task after the restoring write (A7).
    pub confirmed_after_executions: u64,
}

/// The activation variable of a project-scripted experiment, which the authorized start writes TRUE (C12).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivationReview {
    /// The variable's path.
    pub symbol: String,
    /// The variable's address.
    pub address: String,
}

/// The abort variable of a project-scripted experiment, asserted by a failed termination (C12).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AbortReview {
    /// The variable's path.
    pub symbol: String,
    /// The variable's address.
    pub address: String,
    /// The value that asserts an abort, as the declaration names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asserted_value: Option<bool>,
}

/// The review of the campaign that `settings` ask for on `loop_definition`, with the loop's live `sample` when a runtime
/// answered one (B3.3). The campaign's checks run first, in its order, so the first refusal is the one the campaign would give.
///
/// # Errors
/// The settings cannot configure the loop (B3.2); a limit the campaign needs is missing or not ordered (B3.4 step 1, next action
/// Limits); or the method cannot be chosen from the loop's mode and policy (B3.3), with the next action the policy names.
pub fn campaign_review(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
    sample: Option<&ExperimentSample>,
) -> Result<IdentifyReview, Failure> {
    let configured =
        configured_acquisition(loop_definition, Some(settings)).map_err(|error| error.failure())?;
    require_campaign_limits(&configured)?;
    let method = identification_method(&configured, sample).map_err(|error| error.failure())?;
    let maximum_ms = campaign_maximum_ms(&configured)?;
    let input_range = plant_input_range(&configured)?;
    let experiment = configured.experiment.as_ref();
    Ok(IdentifyReview {
        kind: method.kind,
        automatic: method.automatic,
        manual: method.manual,
        execution: experiment.map_or(Execution::ControlStudio, |experiment| experiment.execution),
        input_range,
        output_range: configured.signals.process_variable.range,
        maximum_duration_seconds: ms_as_f64(maximum_ms) / 1_000.0,
        amplitude: experiment.map_or(0.0, |experiment| experiment.amplitude),
        project_scripted: project_scripted(&configured).map(project_scripted_review),
    })
}

/// What a project-scripted experiment writes and asserts, as the loop declares it (C12).
fn project_scripted_review(experiment: &Experiment) -> ProjectScriptedReview {
    ProjectScriptedReview {
        baseline_seconds: experiment.baseline_seconds,
        activation: experiment.activation.as_ref().map(activation_review),
        abort: experiment.abort.as_ref().map(abort_review),
        restoration: experiment.activation.as_ref().map(|_| RestorationReview {
            activation_restored_to: false,
            confirmed_after_executions: RESTORATION_CONFIRMATION_EXECUTIONS,
        }),
    }
}

fn activation_review(activation: &Activation) -> ActivationReview {
    ActivationReview {
        symbol: activation.symbol.clone(),
        address: activation.address.clone(),
    }
}

fn abort_review(abort: &Activation) -> AbortReview {
    AbortReview {
        symbol: abort.symbol.clone(),
        address: abort.address.clone(),
        asserted_value: abort.asserted_value,
    }
}

#[cfg(test)]
#[path = "campaign_review_tests.rs"]
mod tests;
