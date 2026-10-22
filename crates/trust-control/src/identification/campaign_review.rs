//! The review of an identification campaign (specification 35, §0.1a `controlStudio/identify` with `action: "review"`, B3.2 to
//! B3.4, B9.3 and C12): the campaign Identify would run, answered before the engineer starts it. The review is given by the loop,
//! the settings the engineer asks for and, when a runtime is connected, one live sample of the loop. It starts nothing and writes
//! nothing. It answers an identity, the digest of what the campaign runs, which a start names (E11.8), and on a physical target
//! the approval block that the engineer approves (A2.5).
//!
//! The campaign's own preconditions and rules decide the review (`configured_acquisition`, `require_campaign_limits`,
//! `identification_method`), so the review refuses what the campaign refuses, with the same next action. What the campaign
//! measures before it steps (the steady baseline's noise, B3.3) is not known yet: the amplitude is then shown as the engineer
//! asked for it, and zero means Identify sizes it after the baseline.
//!
//! A project test (C12) is reviewed and started without the engineer's settings: the project owns its experiment, so it runs the
//! loop's declared experiment ([`campaign_settings`]). Its review binds the experiment settings the runtime holds when it is made
//! ([`bound_to_settings`]), so a setting changed before the start gives another identity and the start is refused.

#![allow(clippy::module_name_repetitions)] // `campaign_review` names the review it gives.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;

use crate::commissioning::application_records::review_identity;
use crate::experiment::plan::project_stop_conditions;
use crate::experiment::{ControlExperimentKind, ExperimentSample};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::{
    configured_acquisition, declared_settings, AcquisitionMethod, AcquisitionSettings,
};
use crate::identification::campaign::{
    campaign_maximum_ms, ms_as_f64, plant_input_range, require_campaign_limits,
};
use crate::identification::connected_run::{
    campaign_hold_ms, campaign_method, own_steps_check, ACQUISITION_SETTINGS_TEXT,
};
use crate::identification::policy::{identification_method, project_scripted};
use crate::limits::RESTORATION_CONFIRMATION_EXECUTIONS;
use crate::loop_definition::{
    Activation, AuxiliaryRole, Direction, Execution, Experiment, LoopDefinition, LoopSignal, Range,
};
use crate::recording::EvidenceSource;

/// The review of the campaign that Identify would run (B3.2 to B3.4, C12).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyReview {
    /// The identity of this review (§0.1a, E11.8): `sha256:` followed by the digest of what the campaign runs. A start names it,
    /// and the engine refuses a start whose identity no longer matches.
    pub identity: String,
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
    /// The longest the campaign runs, in s: the baseline wait and the maximum duration of the step (A7, B3.4 step 2). A project step
    /// test whose loop declares a hold parameter runs the recording of the hold the runtime applies: its baseline, that hold and one
    /// restoration sample (C12).
    pub maximum_duration_seconds: f64,
    /// The amplitude the campaign starts from. Zero means Identify sizes the step from the measured noise after the baseline (B3.3).
    pub amplitude: f64,
    /// The project's scenario, for a project-scripted experiment (C12); absent for an experiment Control Studio runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_scripted: Option<ProjectScriptedReview>,
    /// What one physical approval covers (A2.5, B3.4 step 1, B9.3): present only on a physical target, where the start needs the
    /// engineer's approval. A simulated target shows no block; pressing the action confirms the start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<CampaignApproval>,
}

/// What one physical approval of an identification campaign covers (A2.5, B3.4 step 1, B9.3): the loop, each signal the campaign
/// writes with its limits, the amplitude, the maximum duration, the stop conditions and the restoration. It shows limits, not
/// measured values: the steady baseline sets the operating point (B3.4 step 2).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CampaignApproval {
    /// The loop the campaign works on.
    pub loop_id: String,
    /// Each signal the campaign writes, with the limits it keeps the signal in.
    pub signals: Vec<ApprovalSignal>,
    /// The step's amplitude; zero when Identify sizes the step after the steady baseline (B3.3).
    pub amplitude: f64,
    /// The longest the campaign runs, in s (A7, B3.4 step 2).
    pub duration_seconds: f64,
    /// What ends the campaign before its end, and what the runtime does then (B3.4 step 6, B3.9).
    pub stop_conditions: Vec<String>,
    /// What the restoration returns, and when it is confirmed (B3.4 step 5, A7).
    pub restoration: String,
    /// The sentence the page shows with the block: the target is physical, and nothing is written before the engineer approves the
    /// reviewed campaign (A2.5). Always present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// One signal an identification campaign writes, with the limits it keeps it in (A2.5, B3.4). A limit the loop does not declare
/// is not a number, and the page shows it as not declared (B9.3).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalSignal {
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's unit; empty means dimensionless or not applicable.
    pub unit: String,
    /// The smallest value the signal may take; not a number when no limit is declared.
    pub minimum: f64,
    /// The largest value the signal may take; not a number when no limit is declared.
    pub maximum: f64,
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
    /// The stop conditions the runtime applies to the scenario (E5.5, C12): the list the test's plan names.
    pub stop_conditions: Vec<String>,
    /// The test signal of the plan, in words (C12): the activation and the value the start writes to it, `P1.ExperimentEnabled · TRUE
    /// while the test runs`, or `Observation only: no signal is written` when the project declares no activation. Always present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_signal: Option<String>,
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

/// The stop conditions of an identification campaign that Control Studio runs, in the words its approval shows (B3.4 step 6, B3.9).
const CAMPAIGN_STOP_CONDITIONS: [&str; 2] = [
    "The runtime stops the campaign when the plant input it writes leaves its limits (B3.4 step 6).",
    "Abort, a failure, a forced signal, a mode left, a reload or a restart stops it at once, and the runtime restores what it wrote (B3.9).",
];

/// The restoration of an identification campaign, in the words its approval shows (B3.4 step 5, A6.2).
const CAMPAIGN_RESTORATION: &str = "The runtime restores the written signal to its value from before the campaign and confirms the restoration (B3.4 step 5). A Manual campaign returns the loop to its starting mode (A6.2).";

/// The restoration of a campaign that writes nothing, in the words its approval shows.
const NOTHING_WRITTEN: &str = "Nothing is written, so nothing is restored.";

/// The test signal of a project test whose project declares no activation (C12).
pub const OBSERVATION_ONLY: &str = "Observation only: no signal is written";

/// The sentence of the approval block of a project test (A2.5, C12).
const APPROVE_TEST: &str =
    "Physical target. Approve the reviewed test before it starts; nothing is written before your approval.";

/// The sentence of the approval block of any other campaign (A2.5).
const APPROVE_CAMPAIGN: &str =
    "Physical target. Approve the reviewed campaign before it starts; nothing is written before your approval.";

/// The settings that a review and a start of `controlStudio/identify` run with (§0.1a, B3.2, C12): the engineer's, when the request
/// carries them. The project test of a project-scripted loop carries none: the project owns its experiment, so the test runs the loop's
/// declared experiment with the Automatic method (`declared_settings`), and a loop that declares no duration is refused under Limits by
/// the campaign's own check (B3.4 step 1). Any other loop needs the engineer's method and limits.
///
/// # Errors
/// The request carries no settings, and the loop has no project test (B3.2), with Limits as the next action.
pub fn campaign_settings(
    loop_definition: &LoopDefinition,
    settings: Option<AcquisitionSettings>,
) -> Result<AcquisitionSettings, Failure> {
    if let Some(settings) = settings {
        return Ok(settings);
    }
    if project_scripted(loop_definition).is_some() {
        return Ok(declared_settings(
            loop_definition,
            AcquisitionMethod::Automatic,
        ));
    }
    Err(Failure::new(
        ACQUISITION_SETTINGS_TEXT,
        Cause::Settings,
        NextAction::OpenLimits,
    ))
}

/// The review of a project test bound to the values of its experiment settings that the runtime holds when the review is made (C12,
/// Parameter purpose: the settings are frozen through review, baseline and execution). The identity is the digest of the review's own
/// identity and those values, so a setting that changes between the review and the start gives another identity, and the start is
/// refused as a changed review is (E11.8). A review without settings, of a loop whose parameter tool writes none, is returned as it is.
#[must_use]
pub fn bound_to_settings(
    mut review: IdentifyReview,
    settings: Option<&BTreeMap<String, f64>>,
) -> IdentifyReview {
    if let Some(values) = settings {
        review.identity = review_identity(&json!({
            "review": review.identity,
            "experimentSettings": values,
        }));
    }
    review
}

/// The review of the campaign that `settings` ask for on `loop_definition`, with the loop's live `sample` when a runtime
/// answered one (B3.3). The campaign's checks run first, in its order, so the first refusal is the one the campaign would give.
/// A live sample on a physical target gives the review its approval block (A2.5). `hold_seconds` is the value the runtime applies to
/// the loop's declared hold parameter, which the caller reads for a project step test (C12, Hold parameter): the review's duration
/// is the recording of that hold. A loop that declares no hold parameter ignores it.
///
/// # Errors
/// The settings cannot configure the loop (B3.2); a limit the campaign needs is missing or not ordered (B3.4 step 1, next action
/// Limits); the maximum duration is outside 1 s to 24 h (A7, next action Limits); the method cannot be chosen from the loop's
/// mode and policy (B3.3), with the next action the policy names; or the loop's hold parameter has no applied value within its
/// bounds, or one whose recording is longer than the maximum duration (`campaign_hold_ms`).
pub fn campaign_review(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
    sample: Option<&ExperimentSample>,
    hold_seconds: Option<f64>,
) -> Result<IdentifyReview, Failure> {
    let configured =
        configured_acquisition(loop_definition, Some(settings)).map_err(|error| error.failure())?;
    require_campaign_limits(&configured)?;
    let loop_method =
        identification_method(&configured, sample).map_err(|error| error.failure())?;
    own_steps_check(&configured, loop_method)?;
    let method = campaign_method(&configured, loop_method);
    let maximum_ms = campaign_maximum_ms(&configured)?;
    let input_range = plant_input_range(&configured)?;
    // A project step test records its baseline, the hold the runtime applies and one restoration sample (C12): that recording is the
    // duration the review shows, and a hold that cannot be recorded is refused here, before the engineer starts the test.
    let hold_ms = campaign_hold_ms(&configured, hold_seconds)?;
    let experiment = configured.experiment.as_ref();
    let maximum_duration_seconds = hold_ms.map_or_else(
        || ms_as_f64(maximum_ms) / 1_000.0,
        |recording_ms| recording_ms / 1_000.0,
    );
    let execution = experiment.map_or(Execution::ControlStudio, |experiment| experiment.execution);
    let amplitude = experiment.map_or(0.0, |experiment| experiment.amplitude);
    let output_range = configured.signals.process_variable.range;
    let scripted = project_scripted(&configured)
        .map(|experiment| project_scripted_review(experiment, maximum_duration_seconds));
    let approval = sample
        .is_some_and(|sample| sample.source == EvidenceSource::PhysicalPlant)
        .then(|| {
            campaign_approval(
                loop_definition,
                method.kind,
                input_range,
                amplitude,
                maximum_duration_seconds,
                scripted.as_ref(),
            )
        });
    // What the confirmation runs (E11.8): the loop and its revision, the settings, and everything the review derives from them
    // and from the target's kind. The running values of the process are not part of it, because they move with every scan.
    let identity = review_identity(&json!({
        "loopId": loop_definition.id,
        "loopRevision": loop_definition.revision,
        "settings": settings,
        "kind": method.kind,
        "automatic": method.automatic,
        "manual": method.manual,
        "execution": execution,
        "inputRange": input_range,
        "outputRange": output_range,
        "maximumDurationSeconds": maximum_duration_seconds,
        "amplitude": amplitude,
        "projectScripted": scripted,
        "approval": approval,
    }));
    Ok(IdentifyReview {
        identity,
        kind: method.kind,
        automatic: method.automatic,
        manual: method.manual,
        execution,
        input_range,
        output_range,
        maximum_duration_seconds,
        amplitude,
        project_scripted: scripted,
        approval,
    })
}

/// What a project-scripted experiment writes, asserts and stops on, as the loop declares it (C12, E5.5).
fn project_scripted_review(
    experiment: &Experiment,
    duration_seconds: f64,
) -> ProjectScriptedReview {
    ProjectScriptedReview {
        baseline_seconds: experiment.baseline_seconds,
        activation: experiment.activation.as_ref().map(activation_review),
        abort: experiment.abort.as_ref().map(abort_review),
        restoration: experiment.activation.as_ref().map(|_| RestorationReview {
            activation_restored_to: false,
            confirmed_after_executions: RESTORATION_CONFIRMATION_EXECUTIONS,
        }),
        stop_conditions: project_stop_conditions(
            duration_seconds,
            experiment.abort.as_ref().and_then(|abort| {
                abort
                    .asserted_value
                    .map(|asserted| (abort.symbol.as_str(), asserted))
            }),
        ),
        test_signal: Some(experiment.activation.as_ref().map_or_else(
            || OBSERVATION_ONLY.to_owned(),
            |activation| format!("{} · TRUE while the test runs", activation.symbol),
        )),
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

/// The approval block of a campaign of `kind` on a physical target (A2.5, B3.4 step 1, B9.3). The signals are those the campaign
/// writes, and the restoration and the stop conditions are the ones its kind names.
fn campaign_approval(
    loop_definition: &LoopDefinition,
    kind: ControlExperimentKind,
    input_range: Range,
    amplitude: f64,
    duration_seconds: f64,
    project_scripted: Option<&ProjectScriptedReview>,
) -> CampaignApproval {
    let signals = written_signals(loop_definition, kind, input_range);
    let restoration = match (signals.is_empty(), project_scripted.and_then(|scripted| scripted.restoration.as_ref())) {
        (true, _) => NOTHING_WRITTEN.to_owned(),
        (false, Some(restoration)) => format!(
            "The activation returns to {} however the test ends, and its restoration is confirmed after {} executions of the loop's task (C12, A7).",
            if restoration.activation_restored_to { "TRUE" } else { "FALSE" },
            restoration.confirmed_after_executions,
        ),
        (false, None) => CAMPAIGN_RESTORATION.to_owned(),
    };
    let stop_conditions = match project_scripted {
        Some(scripted) => scripted.stop_conditions.clone(),
        None => CAMPAIGN_STOP_CONDITIONS
            .iter()
            .map(|text| (*text).to_owned())
            .collect(),
    };
    let message = if project_scripted.is_some() {
        APPROVE_TEST
    } else {
        APPROVE_CAMPAIGN
    };
    CampaignApproval {
        loop_id: loop_definition.id.clone(),
        signals,
        amplitude,
        duration_seconds,
        stop_conditions,
        restoration,
        message: Some(message.to_owned()),
    }
}

/// The signals a campaign of `kind` writes, each with the limits it keeps it in (A2.5, B3.4): the activation of a project-scripted
/// test (its limits are not declared), the setpoint of a setpoint step (the permitted output range it is capped by, B3.3), the
/// writable disturbance of a load step, or the plant input of every other step (the plant input range, B3.4 step 1). An
/// observation writes nothing.
fn written_signals(
    loop_definition: &LoopDefinition,
    kind: ControlExperimentKind,
    input_range: Range,
) -> Vec<ApprovalSignal> {
    let output_range = loop_definition.signals.process_variable.range;
    match kind {
        ControlExperimentKind::Observation => Vec::new(),
        ControlExperimentKind::ProjectSequence => loop_definition
            .experiment
            .as_ref()
            .and_then(|experiment| experiment.activation.as_ref())
            .map(|activation| ApprovalSignal {
                symbol: activation.symbol.clone(),
                unit: String::new(),
                minimum: f64::NAN,
                maximum: f64::NAN,
            })
            .into_iter()
            .collect(),
        ControlExperimentKind::SetpointStep => loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|setpoint| approval_signal(setpoint, output_range))
            .into_iter()
            .collect(),
        ControlExperimentKind::LoadStep => loop_definition
            .signals
            .additional
            .iter()
            .flatten()
            .find(|signal| {
                signal.role == AuxiliaryRole::Disturbance && signal.direction == Direction::Write
            })
            .map(|signal| ApprovalSignal {
                symbol: signal.symbol.clone(),
                unit: signal.unit.clone(),
                minimum: signal.range.minimum,
                maximum: signal.range.maximum,
            })
            .into_iter()
            .collect(),
        _ => vec![approval_signal(
            &loop_definition.signals.manipulated_variable,
            input_range,
        )],
    }
}

/// A loop signal with the limits given for it.
fn approval_signal(signal: &LoopSignal, range: Range) -> ApprovalSignal {
    ApprovalSignal {
        symbol: signal.symbol.clone(),
        unit: signal.unit.clone(),
        minimum: range.minimum,
        maximum: range.maximum,
    }
}

#[cfg(test)]
#[path = "campaign_review_tests.rs"]
mod tests;
