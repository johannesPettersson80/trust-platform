//! The rules of a connected identification run (`connectedIdentification.ts`; specification 35, A2.5, A4.2, B3.4 and B3.9;
//! runtime protocol annex E5.3, E11.4 and E11.6): the checks before anything is recorded, the hold of the step and its plan,
//! which start answers end the run and which are reconciled by the experiment's identity, what a hold's terminal status and an
//! explicit Cancel mean, and the sentences of the run with their causes and next actions (A11). The run's calls to the runtime
//! are in `trust-lsp` (`workflow::identify`); every decision they need is here.
//!
//! Several plant inputs (C7, C8): one campaign steps each input in turn, each in its own interval of one recording. The steps of
//! a loop's inputs are planned here ([`input_steps`], as `connectedIdentificationChannels.ts` builds them), and the hold of each
//! step shares the time left (`step_hold_ms`). The mode transfer of a method that needs the other mode moves with V4
//! (`identification::mode`).

use serde_json::Value;

use crate::experiment::number_text::js_round;
use crate::experiment::{
    build_control_experiment_plan, checked_experiment_start, classify_control_experiment_status,
    decode_control_experiment_recovery, ControlExperimentKind, ControlExperimentPlan,
    ControlExperimentPlanResult, ControlExperimentRecovery, ControlExperimentRecoveryScope,
    ControlExperimentRequestKind, ControlExperimentRuntimePhase, ControllerMode, ExperimentSample,
    PlanInput, PlanSource, RecordingRecoveryOwner, RestorationState,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::{
    decode_acquisition_settings, AcquisitionSettings,
};
use crate::identification::automatic::AutomaticIdentificationResult;
use crate::identification::campaign::baseline::BaselineSummary;
use crate::identification::campaign::{
    experiment_stop_failure, format_duration, ms_as_f64, plant_input_range,
    require_campaign_limits, restoration_confirmed, CANCELLED_TEXT,
};
use crate::identification::connected_record::{ConnectedRequest, RuntimeVerificationResult};
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};
use crate::identification::model_compatibility::{
    assert_qualified_model_result, CompatibilityRefusal,
};
use crate::identification::policy::{
    identification_amplitude, on_off_actuator, project_scripted, AmplitudeInput,
    IdentificationMethod,
};
use crate::identification::reconcile::ExperimentIdentity;
use crate::identification::recorded_model::RecordedFitError;
use crate::identification::texts::{compatibility_text, MISSING_NONZERO_STEP};
use crate::limits::MINIMUM_EXPERIMENT_DURATION_MS;
use crate::loop_definition::{Execution, LoopDefinition, RelayDirection};
use crate::model::{GeneralModel, ModelValidation};
use crate::recording::EvidenceSource;

// The steps of the loop's plant inputs (C7, C8) are in their own module; their rules are re-exported here.
#[path = "channel_steps.rs"]
mod channel_steps;

pub use channel_steps::{
    campaign_method, input_steps, loop_signal_of, manual_address, own_steps_check,
    several_channels, step_sample, InputStep,
};

/// The progress of a run that identifies the plant.
pub const IDENTIFYING_TEXT: &str = "Identifying — recording the connected plant…";
/// The progress of a run that records the independent model check.
pub const CHECKING_TEXT: &str = "Checking model — recording the connected plant…";
/// The progress of the fit.
pub const FITTING_TEXT: &str = "Fitting the measured response…";
/// The refusal of a physical target, before anything is recorded or written (A2.5): this path carries no approval.
pub const PHYSICAL_TARGET_TEXT: &str =
    "Live physical identification requires the reviewed physical experiment approval.";
/// The refusal of a start whose review no longer matches the campaign (§0.1a, E11.8).
pub const REVIEW_CHANGED_TEXT: &str = "The campaign changed since it was reviewed: its loop, its mode or its settings differ. Review it again before it starts.";
/// The refusal of a relay experiment whose first direction is downward.
pub const NEGATIVE_RELAY_TEXT: &str =
    "The connected runtime does not support negative-gain relay feedback.";
/// The failure of a hold whose experiment did not confirm its restoration (B3.4 step 5).
pub const UNRESTORED_TEXT: &str = "The experiment could not restore its original command.";
/// The start of the failure of a connected record whose evaluation does not pass (B3.9): the experiment stopped at a limit or
/// with missing samples. The finding follows it, and the recording is kept and is not qualified for a fit.
pub const EVALUATION_TEXT: &str = "The experiment stopped at a limit or with missing samples.";
/// The failure of an explicit Cancel that cannot confirm the restoration of its experiment.
pub const RECONNECT_TEXT: &str =
    "Reconnect the original runtime to confirm experiment restoration.";
/// The refusal of acquisition settings that do not decode (`identifyPlant` of `engineerActions/find.ts`).
pub const ACQUISITION_SETTINGS_TEXT: &str =
    "Review acquisition method, duration, amplitude and relay direction before finding a model.";
// From panelPersistence.ts (`storeExperimentRecovery`); moves with V6.
/// The refusal of an experiment's journal record that does not decode for its loop.
pub const EXPERIMENT_RECOVERY_TEXT: &str =
    "Test experiment recovery is invalid or outside the active loop scope.";
/// The sentence of the refusal of Identify plant, Open recording and Reset while the loop's journal holds an operation that is
/// unfinished (A8.4, A8.5; B3.9): it runs, or its outcome is unconfirmed. The operation is waited for, or resumed or cancelled,
/// and nothing is started beside it.
pub const UNFINISHED_TEXT: &str =
    "This loop has an unfinished operation. Wait for it to end, or resume or cancel it.";
/// The refusal of a live action, or a resume, that has no runtime connection (E11.1): it reaches the runtime, so the runtime must
/// be started first. The sentence is the one the `TypeScript` engine gives for the same refusal (`panelWorkflowSupport.ts`).
pub const RUNTIME_REQUIRED_TEXT: &str = "Start the runtime before running a live action.";
/// The operation `find.ts` names when it asks for the authority of a live write (`requireLiveWrite`).
pub const IDENTIFY_OPERATION: &str = "automatic identification campaign";
/// How many status reads confirm a restoration after an abort, one every
/// [`POLL_MILLISECONDS`](crate::identification::campaign::POLL_MILLISECONDS) (`confirmRestored`).
pub const RESTORATION_POLLS: usize = 50;

/// The refusal codes of a start that the runtime refused before it accepted an experiment (E0.4, E5.3;
/// `REFUSED_START_CODES`): no experiment exists, so nothing is reconciled.
pub const REFUSED_START_CODES: [&str; 13] = [
    "invalid_experiment",
    "controller_mode_blocked",
    "physical_approval_required",
    "invalid_address",
    "invalid_value",
    "outside_limits",
    "io_unavailable",
    "stale_scan",
    "forced_signal",
    "incompatible_binding",
    "restoration_mismatch",
    "experiment_state_unavailable",
    "experiment_active",
];

/// What a start request gave (E5.3, E0.4): the runtime's answer, its refusal with the code and text it carries, or no answer
/// that can be read (the connection failed, or the answer is not a control response).
#[derive(Debug, Clone, PartialEq)]
pub enum StartRead {
    /// The runtime answered.
    Answer(Value),
    /// The runtime refused the start.
    Refused {
        /// The refusal code of annex E0.4, when the runtime gave one.
        code: Option<String>,
        /// The runtime's text, which names the failed check.
        text: String,
    },
    /// No answer could be read.
    Unreadable(String),
}

/// What a start's answer means for the run (`dispatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    /// The runtime started the experiment that was sent (E5.3).
    Started,
    /// The runtime refused the start before it accepted an experiment: the run ends with the refusal, and nothing is
    /// reconciled.
    Refused(Failure),
    /// The outcome is unknown: the start is reconciled by its identity and never sent again (E11.4).
    Unknown,
}

/// What the identity of a start whose outcome was unknown means for the run (`reconcileStart`, E11.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartReconciliation {
    /// The runtime holds the experiment, running or ended: the run follows it to its end.
    Follow,
    /// The runtime holds no experiment of this identity, so none ran: the run ends, and nothing is left owned.
    Absent(Failure),
    /// The status cannot be read: the experiment is left to the runtime, never aborted here, and reopening reconciles it.
    Unresolved(Failure),
}

/// How a hold ended, as the runtime's terminal status says (B3.4 steps 5 and 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldEnd {
    /// The restoration is not confirmed: the experiment stays owned until it is.
    Unrestored(Failure),
    /// The experiment stopped before its hold ended: its recording is kept and is not qualified for a fit.
    Stopped(Failure),
    /// The experiment reached its planned end, and its restoration is confirmed.
    Completed,
}

/// What an explicit Cancel does with the experiment its identification journaled, as the experiment's identity shows it
/// (`cancel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelStep {
    /// The experiment runs: abort it, then confirm its restoration.
    Abort,
    /// The experiment has ended: confirm its restoration.
    Confirm,
    /// The runtime holds no experiment of this identity: nothing ran, so nothing is aborted or restored.
    Nothing,
    /// The status cannot be read, so the restoration cannot be confirmed.
    Unconfirmed(Failure),
}

/// Where the fit of a campaign's recording starts and where its steady baseline window ends (B3.4, N1.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitWindow {
    /// The start of the steady baseline window, in s: the fit starts here, and never before the recording's start.
    pub fit_from_seconds: f64,
    /// The end of the steady baseline window, in s: the operating point is taken over the window.
    pub baseline_end_seconds: f64,
}

/// The failure of a run cancelled between its steps (`checkCancelled`).
#[must_use]
pub fn identification_cancelled() -> Failure {
    Failure::new(CANCELLED_TEXT, Cause::Unclassified, NextAction::None)
}

/// The refusal of a run before anything is recorded or written (`acquire`) that carries no approval: a physical target, or a
/// relay whose first direction is downward, which the runtime does not support. `None` when the run may record. It is
/// [`approved_campaign_refusal`] without an approval.
#[must_use]
pub fn campaign_refusal(
    loop_definition: &LoopDefinition,
    method: IdentificationMethod,
    sample: Option<&ExperimentSample>,
) -> Option<Failure> {
    approved_campaign_refusal(loop_definition, method, sample, false)
}

/// The refusal of a run before anything is recorded or written (`acquire`), with the engineer's approval of the reviewed campaign
/// on a physical target (A2.5): a physical target is refused unless `physical_approved` is set, because one approval covers one
/// reviewed campaign and nothing else (A2.5, §0.1a). A relay whose first direction is downward is refused on either target.
#[must_use]
pub fn approved_campaign_refusal(
    loop_definition: &LoopDefinition,
    method: IdentificationMethod,
    sample: Option<&ExperimentSample>,
    physical_approved: bool,
) -> Option<Failure> {
    if !physical_approved
        && sample.is_some_and(|sample| sample.source == EvidenceSource::PhysicalPlant)
    {
        return Some(physical_target());
    }
    let downward = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.relay_direction)
        == Some(RelayDirection::Negative);
    (method.kind == ControlExperimentKind::Relay && downward)
        .then(|| Failure::new(NEGATIVE_RELAY_TEXT, Cause::Unclassified, NextAction::None))
}

// From runtimeTarget.ts (`requireLiveWrite`, without an approval: the approval path moves with V4).
/// The refusal of a live write on a physical target without the engineer's approval (A2.5): `None` on a simulated target,
/// where pressing the action is the confirmation. `operation` names the write.
#[must_use]
pub fn live_write_refusal(source: EvidenceSource, operation: &str) -> Option<Failure> {
    (source == EvidenceSource::PhysicalPlant).then(|| {
        Failure::new(
            &format!(
                "Physical approval is required for {operation} on a physical target. Review it on the page and approve it immediately before it starts."
            ),
            Cause::Approval,
            NextAction::Approve,
        )
    })
}

/// The acquisition settings an Identify request carries, decoded and checked (`identifyPlant`). A loop that declares no duration
/// gives its settings a duration of zero (`declared_settings`), which is no duration: when the settings give none either, the
/// refusal is the campaign's own, and it names the missing field (B3.4 step 1, B3.9).
///
/// # Errors
/// The settings do not decode or break a check of `decode_acquisition_settings`: the engineer reviews them in Limits. The loop
/// declares no duration and the settings give none: the campaign's refusal names the first missing limit (the plant input
/// limits, then `experiment.durationSeconds`), and Limits opens there.
pub fn checked_acquisition_settings(
    loop_definition: &LoopDefinition,
    payload: &Value,
) -> Result<AcquisitionSettings, Failure> {
    if let Some(settings) = decode_acquisition_settings(payload) {
        return Ok(settings);
    }
    let typed = crate::json::decode_value::<AcquisitionSettings>(payload.clone());
    let settings_give_no_duration = typed.is_ok_and(|settings| settings.duration_seconds <= 0.0);
    let loop_declares_no_duration = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.duration_seconds)
        .is_none();
    if settings_give_no_duration && loop_declares_no_duration {
        require_campaign_limits(loop_definition)?;
    }
    Err(Failure::new(
        ACQUISITION_SETTINGS_TEXT,
        Cause::Settings,
        NextAction::OpenLimits,
    ))
}

/// The expected settling time the campaign's recording sets its record period by, in whole ms (A4.2): the declared one, or a
/// tenth of the maximum duration, so a response ten times faster than the maximum still gets about 200 samples. `None` below
/// 1 ms, where the record period only fits the sample limit, as for no settling time at all.
#[must_use]
pub fn recording_settling_ms(maximum_ms: u64, declared_seconds: Option<f64>) -> Option<u64> {
    let seconds = declared_seconds.unwrap_or_else(|| ms_as_f64(maximum_ms) / 10_000.0);
    whole_milliseconds(seconds)
}

/// Whether the baseline's refusal offers a setpoint step: a loop with a setpoint that the sample reads in Auto (B3.4 step 2). A
/// project-scripted loop owns its experiment and writes no setpoint, so it is never offered one (C12).
#[must_use]
pub fn offers_setpoint_step(
    loop_definition: &LoopDefinition,
    sample: Option<&ExperimentSample>,
) -> bool {
    loop_definition.signals.setpoint.is_some()
        && project_scripted(loop_definition).is_none()
        && sample.is_some_and(|sample| sample.controller_mode == ControllerMode::Auto)
}

/// How long a step is held, in ms (B3.4 step 4): the maximum duration counted from the end of the baseline, within the recording,
/// shared by the `remaining` inputs still to step, less `restoration_ms` for the restoration ([`restoration_allowance_ms`]). The
/// step starts at `step_seconds` and the baseline ended at `baseline_end_seconds`, both in process time of the recording, so a
/// long wait for a steady start never shortens the step. A campaign of one input has `remaining` 1 (`connectedIdentificationChannels`).
#[must_use]
pub fn step_hold_ms(
    maximum_ms: u64,
    recording_ms: u64,
    restoration_ms: f64,
    baseline_end_seconds: f64,
    step_seconds: f64,
    remaining: u32,
) -> f64 {
    let after_baseline = ms_as_f64(maximum_ms) - (step_seconds - baseline_end_seconds) * 1_000.0;
    let within_recording = ms_as_f64(recording_ms) - step_seconds * 1_000.0;
    let shared = after_baseline.min(within_recording) / f64::from(remaining.max(1));
    shared.floor() - restoration_ms
}

/// The restoration allowance of a hold, in ms (B3.4 step 4): one baseline window, or, for a project-scripted experiment, which
/// records its declared duration, the one restoration sample, one execution of the loop's task (C12).
#[must_use]
pub fn restoration_allowance_ms(
    loop_definition: &LoopDefinition,
    baseline: &BaselineSummary,
) -> f64 {
    let project_scripted = loop_definition
        .experiment
        .as_ref()
        .is_some_and(|experiment| experiment.execution == Execution::ProjectScripted);
    if project_scripted {
        loop_definition.controller.task_period_ms
    } else {
        js_round(baseline.window_seconds * 1_000.0)
    }
}

/// The refusal of a hold shorter than the shortest experiment (A7), or `None`. A limit is the fix, so Limits opens (B3.9).
#[must_use]
pub fn hold_refusal(hold_ms: f64) -> Option<Failure> {
    (hold_ms < ms_as_f64(MINIMUM_EXPERIMENT_DURATION_MS)).then(|| {
        Failure::new(
            &format!(
                "The maximum experiment duration leaves no time to hold the step after the baseline and its restoration ({} free). Open Limits… for a longer maximum duration.",
                format_duration(hold_ms.max(0.0) / 1_000.0)
            ),
            Cause::Settings,
            NextAction::OpenLimits,
        )
    })
}

/// The seed of the step's excitation: the loop's seed (1 when it declares none), and the next one for the independent model
/// check. The plan keeps its low 32 bits, as the `TypeScript`'s `seed >>> 0` does.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // The low 32 bits are the seed, as `>>> 0` takes them.
pub fn step_seed(loop_definition: &LoopDefinition, validation: bool) -> u32 {
    let configured = loop_definition
        .experiment
        .as_ref()
        .map_or(1, |experiment| experiment.seed);
    configured.wrapping_add(u64::from(validation)) as u32
}

/// The reviewed plan of the step (`preparePlan`, B3.3 and B3.4 step 3): the method's amplitude, the hold as its duration, the
/// step's seed, and the steady baseline's mean and half range, which centre and size a relay's switching band. The plan keeps
/// the review's identifier; the run gives it a fresh one before it is journaled.
///
/// # Errors
/// The loop's plant input range is missing or not ordered (the refusal of [`plant_input_range`]); no synchronized sample is
/// available, or the step has no headroom (the policy's refusal); the review refuses the plan (its reason); or the plan runs on
/// a physical target, which needs the reviewed physical approval this path does not carry.
pub fn step_plan(
    loop_definition: &LoopDefinition,
    sample: Option<&ExperimentSample>,
    method: IdentificationMethod,
    baseline: &BaselineSummary,
    hold_ms: f64,
    validation: bool,
) -> Result<ControlExperimentPlan, Failure> {
    approved_step_plan(
        loop_definition,
        sample,
        method,
        baseline,
        hold_ms,
        validation,
        false,
    )
}

/// The reviewed plan of the step with the engineer's approval of a physical target (`approved_step_plan`, A2.5): as
/// [`step_plan`], but a plan on a physical target is accepted when `physical_approved` is set. The plan then needs the runtime's
/// own approval at its start (E10.2), which the start sends with the approval.
///
/// # Errors
/// As [`step_plan`]; and a plan on a physical target that the approval does not cover.
pub fn approved_step_plan(
    loop_definition: &LoopDefinition,
    sample: Option<&ExperimentSample>,
    method: IdentificationMethod,
    baseline: &BaselineSummary,
    hold_ms: f64,
    validation: bool,
    physical_approved: bool,
) -> Result<ControlExperimentPlan, Failure> {
    let range = plant_input_range(loop_definition)?;
    let amplitude = identification_amplitude(&AmplitudeInput {
        loop_definition,
        sample,
        kind: method.kind,
        automatic: method.automatic,
        noise: baseline.noise,
        validation,
        fallback_amplitude: (range.maximum - range.minimum) * 0.1,
        on_off: on_off_actuator(loop_definition),
    })
    .map_err(|error| error.failure())?;
    // A relay's switching band is centred on the baseline only for a loop with one plant input: a loop that steps several inputs
    // keeps the measurement's own centre (`preparePlan` passes the baseline only when there is one channel).
    let single_input = loop_definition
        .channels
        .as_ref()
        .is_none_or(|channels| channels.inputs.len() <= 1);
    let reviewed = build_control_experiment_plan(&PlanInput {
        loop_definition,
        sample,
        kind: ControlExperimentRequestKind::from(method.kind),
        amplitude,
        duration_seconds: hold_ms / 1_000.0,
        physical_topology_reviewed: physical_approved,
        seed: Some(step_seed(loop_definition, validation)),
        process_center: single_input.then_some(baseline.mean),
        process_noise: single_input.then_some(baseline.half_range),
        hold_seconds: None,
    });
    match reviewed {
        ControlExperimentPlanResult::Accepted { plan }
            if plan.source == PlanSource::Simulation || physical_approved =>
        {
            Ok(*plan)
        }
        ControlExperimentPlanResult::Accepted { .. } => Err(physical_target()),
        ControlExperimentPlanResult::Rejected { reason } => {
            Err(Failure::new(&reason, Cause::Unclassified, NextAction::None))
        }
    }
}

/// The journal record of the step's experiment (`persist(plan, owner)`, E11.2): its identity in the loop, its reviewed plan
/// and the recording it runs with, checked as a reopened panel decodes it.
///
/// # Errors
/// The record does not decode for the loop: the plan or the recording's owner is outside the loop's scope or breaks a rule of
/// the stored form.
pub fn experiment_recovery(
    loop_definition: &LoopDefinition,
    plan: &ControlExperimentPlan,
    owner: &RecordingRecoveryOwner,
) -> Result<ControlExperimentRecovery, Failure> {
    let recovery = ControlExperimentRecovery {
        schema_version: 2,
        experiment_id: plan.id.clone(),
        project_id: loop_definition.project_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        plan: Some(plan.clone()),
        recording_owner: Some(owner.clone()),
    };
    let scope = ControlExperimentRecoveryScope {
        project_id: loop_definition.project_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
    };
    let decodes = serde_json::to_value(&recovery)
        .ok()
        .and_then(|value| decode_control_experiment_recovery(&value, &scope))
        .is_some();
    if decodes {
        Ok(recovery)
    } else {
        Err(Failure::new(
            EXPERIMENT_RECOVERY_TEXT,
            Cause::Implementation,
            NextAction::None,
        ))
    }
}

/// What the start of `experiment_id` gave (`dispatch`). An answer that starts that experiment, running with its restoration
/// pending or not required, is a start (E5.3); any other answer never confirms one (E11.8), so its outcome is unknown. A
/// refusal with one of [`REFUSED_START_CODES`] came before any experiment existed and ends the run with its typed action
/// (E0.4); any other refusal, and no answer at all, leaves the outcome unknown.
#[must_use]
pub fn start_outcome(experiment_id: &str, read: &StartRead) -> StartOutcome {
    match read {
        StartRead::Answer(answer) => {
            if checked_experiment_start(answer, experiment_id).is_ok() {
                StartOutcome::Started
            } else {
                StartOutcome::Unknown
            }
        }
        StartRead::Refused {
            code: Some(code),
            text,
        } if REFUSED_START_CODES.contains(&code.as_str()) => {
            StartOutcome::Refused(Failure::from_runtime(code, text))
        }
        StartRead::Refused { .. } | StartRead::Unreadable(_) => StartOutcome::Unknown,
    }
}

/// What the identity of a start whose outcome was unknown means (`reconcileStart`, E11.4): a running or ended experiment is
/// followed to its end, as a resumed campaign follows it; an absent one never ran; an unreadable one stays the runtime's.
#[must_use]
pub fn start_reconciliation(identity: &ExperimentIdentity) -> StartReconciliation {
    match identity {
        ExperimentIdentity::Running { .. } | ExperimentIdentity::Terminal { .. } => {
            StartReconciliation::Follow
        }
        ExperimentIdentity::Absent { reason } => StartReconciliation::Absent(Failure::new(
            &format!(
                "The experiment start was not confirmed, and the runtime holds no experiment with its identity, so none ran. {reason}"
            ),
            Cause::Unclassified,
            NextAction::None,
        )),
        ExperimentIdentity::Unresolved { reason } => StartReconciliation::Unresolved(Failure::new(
            &format!(
                "The experiment start was not confirmed and its status cannot be read. {reason} The start is not sent again."
            ),
            Cause::Unclassified,
            NextAction::None,
        )),
    }
}

/// How a hold ended, from the runtime's terminal status (B3.4 steps 5 and 6): the restoration first, which only the runtime's
/// restored or not-required state confirms; then the stop, which is normal only for a completed experiment that reached its
/// duration or was finished early.
#[must_use]
pub fn hold_end(state: &Value) -> HoldEnd {
    if !restoration_of(state).is_some_and(restoration_confirmed) {
        return HoldEnd::Unrestored(Failure::new(
            UNRESTORED_TEXT,
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    let completed = state.get("status").and_then(Value::as_str) == Some("completed");
    let stop_reason = state.get("stop_reason").and_then(Value::as_str);
    experiment_stop_failure(completed, stop_reason).map_or(HoldEnd::Completed, |text| {
        HoldEnd::Stopped(Failure::new(&text, Cause::Unclassified, NextAction::None))
    })
}

/// What a hold that stopped before its end, with its restoration confirmed, means for the run (B3.4 step 5, B3.9; §0.1b):
/// the engineer's Cancel when the operation was `cancelled`, because the Cancel aborted the experiment through the runtime's
/// restoration path, so an interrupted verb ends as interrupted; otherwise the stop's own failure, `stopped`. A restoration
/// that is not confirmed is never a cancellation (`HoldEnd::Unrestored` keeps its failure).
#[must_use]
pub fn stopped_hold_outcome(cancelled: bool, stopped: Failure) -> Failure {
    if cancelled {
        identification_cancelled()
    } else {
        stopped
    }
}

/// Whether a status answer shows the experiment ended with its restoration confirmed (`confirmRestored`): its phase is
/// terminal and the runtime reports it restored, or no restoration was required.
#[must_use]
pub fn ended_and_restored(state: &Value) -> bool {
    classify_control_experiment_status(state) == ControlExperimentRuntimePhase::Terminal
        && restoration_of(state).is_some_and(restoration_confirmed)
}

/// The failure of a Cancel whose experiment's restoration is not confirmed within [`RESTORATION_POLLS`] reads. The study stays
/// blocked until it is (B3.9).
#[must_use]
pub fn restoration_unconfirmed() -> Failure {
    Failure::new(RECONNECT_TEXT, Cause::Unclassified, NextAction::None)
}

/// What an explicit Cancel does with its journaled experiment, by the experiment's identity (`cancel`, E11.4, E11.5).
#[must_use]
pub fn cancel_step(identity: &ExperimentIdentity) -> CancelStep {
    match identity {
        ExperimentIdentity::Running { .. } => CancelStep::Abort,
        ExperimentIdentity::Terminal { .. } => CancelStep::Confirm,
        ExperimentIdentity::Absent { .. } => CancelStep::Nothing,
        ExperimentIdentity::Unresolved { .. } => CancelStep::Unconfirmed(restoration_unconfirmed()),
    }
}

/// Where the fit starts and the steady baseline window ends (`completeRecording`'s arguments; B3.4, N1.3): the window that
/// ends at `baseline_end_seconds` and lasts `window_seconds`, from the recording's start at the earliest.
#[must_use]
pub fn fit_window(baseline_end_seconds: f64, window_seconds: f64) -> FitWindow {
    FitWindow {
        fit_from_seconds: (baseline_end_seconds - window_seconds).max(0.0),
        baseline_end_seconds,
    }
}

/// The fit window a connected record's `request` was taken with (B3.4, N1.3), so that a refit of the kept recording (B3.6)
/// starts where the live fit started. The record writes the start of the steady baseline window only when it is after the
/// recording's start, and its end only when it is after that start (`connected_identification_record`): an unwritten start
/// is the recording's start, and an unwritten end is the window's start.
#[must_use]
pub fn stored_fit_window(request: &ConnectedRequest) -> FitWindow {
    let fit_from_seconds = request.baseline_start_seconds.unwrap_or(0.0);
    FitWindow {
        fit_from_seconds,
        baseline_end_seconds: request.baseline_end_seconds.unwrap_or(fit_from_seconds),
    }
}

/// The refusal of Identify plant, Open recording and Reset while the loop's journal holds the operations `operation_ids`
/// (A8.4, A8.5; Identify page specification, Reset), or `None` when it holds none. The refusal names the operations and
/// offers to wait for them (B3.9).
#[must_use]
pub fn unfinished_operation_refusal(operation_ids: &[&str]) -> Option<Failure> {
    (!operation_ids.is_empty()).then(|| {
        Failure::new(
            &format!("{UNFINISHED_TEXT} Journaled: {}.", operation_ids.join(", ")),
            Cause::Unclassified,
            NextAction::Wait,
        )
    })
}

/// The refusal of a request on a loop that an operation of the same engine is already working on (A8.5). It names that
/// operation and waits for it to end, because two operations on one loop could both pass the journal's check before either
/// journals anything.
#[must_use]
pub fn loop_busy_refusal(operation: &str) -> Failure {
    Failure::new(
        &format!(
            "{operation} is running in this engine on this loop. Wait for it to end, then try again."
        ),
        Cause::Unclassified,
        NextAction::Wait,
    )
}

/// The refusal of a live action, or of a resume, that has no runtime connection (E11.1): it reaches the runtime, so nothing
/// starts and nothing is journaled until the runtime is started and connected.
#[must_use]
pub fn runtime_required() -> Failure {
    Failure::new(
        RUNTIME_REQUIRED_TEXT,
        Cause::Runtime,
        NextAction::ConnectRuntime,
    )
}

/// The refusal of a Cancel that names an operation the loop's journal does not hold (§0.1b, D6).
#[must_use]
pub fn not_journaled(loop_id: &str, operation_id: &str) -> Failure {
    Failure::new(
        &format!("Operation {operation_id} is not journaled for loop {loop_id}."),
        Cause::Settings,
        NextAction::None,
    )
}

/// A failure of the engine's own wiring, which no input of the user causes (`Cause::Implementation`): the run gave a model check
/// where an identification was asked for. Nothing is kept.
#[must_use]
pub fn model_check_instead_of_model() -> Failure {
    Failure::new(
        "The identification recorded a model check instead of fitting a model.",
        Cause::Implementation,
        NextAction::None,
    )
}

/// A failure of the engine's own wiring (`Cause::Implementation`): the resumed campaign's journal named no experiment, so the
/// method the campaign used is unknown.
#[must_use]
pub fn resumed_without_experiment() -> Failure {
    Failure::new(
        "The resumed campaign's journal named no experiment, so its method is unknown.",
        Cause::Implementation,
        NextAction::None,
    )
}

/// A failure of the engine's own wiring (`Cause::Implementation`): one recording's fit gave an accepted model, which takes two
/// independent recordings.
#[must_use]
pub fn accepted_model_from_one_record() -> Failure {
    Failure::new(
        "A fit of one recording gave an accepted model, which takes two independent recordings.",
        Cause::Implementation,
        NextAction::None,
    )
}

/// A failure of the engine's own wiring (`Cause::Implementation`): the model was not fitted from exactly one recording, so the
/// study cannot keep the request it was taken with.
#[must_use]
pub fn model_not_from_one_record() -> Failure {
    Failure::new(
        "The model was not fitted from one recording, so the study cannot keep the request it was taken with.",
        Cause::Implementation,
        NextAction::None,
    )
}

/// A failure of the engine's own wiring (`Cause::Implementation`): the recording `recording_id` of a fit was not kept in the
/// study, so it cannot be released.
#[must_use]
pub fn recording_not_kept(recording_id: &str) -> Failure {
    Failure::new(
        &format!("The recording {recording_id} was not kept in the study."),
        Cause::Implementation,
        NextAction::None,
    )
}

/// The refusal of a connected record whose evaluation does not pass, or `None` (B3.9). It names what the evaluation found
/// (A11.1), says that the recording is kept and is not qualified for a fit, and offers no action.
#[must_use]
pub fn evaluation_refusal<E: RunEvaluation>(
    record: &RuntimeVerificationResult<E>,
) -> Option<Failure> {
    if record.evaluation.status() == RequirementStatus::Pass {
        return None;
    }
    let finding = record
        .evaluation
        .finding()
        .map_or_else(String::new, |text| format!(" {text}"));
    Some(Failure::new(
        &format!(
            "{EVALUATION_TEXT}{finding} The recording is kept and is not qualified for a fit."
        ),
        Cause::Unclassified,
        NextAction::None,
    ))
}

/// The failure of a recorded fit that gives no model (B3.5, B3.9). A refusal is the answer, kept as its sentence. Where no
/// attempt gave the model, each attempt's reason is kept (A11.1), and a different method, a larger step or a longer duration
/// is the fix ("No adequate model"). The missing nonzero step (N2.6) gives the same fix, and on a project-scripted loop, where
/// another method is refused, the loop setup instead (B3.9, C12). A path that a later stage ports is named.
#[must_use]
pub fn fit_failure(loop_definition: &LoopDefinition, error: &RecordedFitError) -> Failure {
    match error {
        RecordedFitError::Refused(refusal) => {
            let next_action = if refusal.0 == MISSING_NONZERO_STEP {
                another_method_action(loop_definition)
            } else {
                NextAction::None
            };
            Failure::new(&refusal.0, Cause::Unclassified, next_action)
        }
        RecordedFitError::Parked { path } => Failure::new(
            &format!("Control Studio's Rust engine does not run {path} yet."),
            Cause::ControllerCapability,
            NextAction::None,
        ),
        RecordedFitError::StateModelRequired { attempts } => Failure::new(
            &attempts.join(" "),
            Cause::ModelOrData,
            another_method_action(loop_definition),
        ),
    }
}

/// The next action of a fit whose fix is another method (B3.9: a missing nonzero step, no adequate model): another method,
/// or on a project-scripted loop, where another method is refused (C12), the loop setup, to check the project's scenario.
fn another_method_action(loop_definition: &LoopDefinition) -> NextAction {
    if project_scripted(loop_definition).is_some() {
        NextAction::OpenLoopSetup
    } else {
        NextAction::ChooseMethod
    }
}

/// Whether an identification's model is provisional (specification "Single experiment and provisional model"): a model fitted
/// from the run's own recording is not independent validation and is never validated evidence. Only a model accepted on two
/// independent records (B5.2), or one whose identity is validated, is not provisional.
#[must_use]
pub fn is_provisional<E>(result: &AutomaticIdentificationResult<E>) -> bool {
    match result {
        AutomaticIdentificationResult::Accepted { .. } => false,
        AutomaticIdentificationResult::Model { model, .. }
        | AutomaticIdentificationResult::StateModel { model, .. } => {
            model.identity().validation != ModelValidation::Validated
        }
    }
}

/// The model of a fitted result, whichever form it has: a single-channel model, or a state model of several channels (C7, C8).
/// `None` for an accepted identification, whose model is checked by the session that accepts it.
#[must_use]
fn fitted_model<E>(result: &AutomaticIdentificationResult<E>) -> Option<&GeneralModel> {
    match result {
        AutomaticIdentificationResult::Model { model, .. }
        | AutomaticIdentificationResult::StateModel { model, .. } => Some(model.as_ref()),
        AutomaticIdentificationResult::Accepted { .. } => None,
    }
}

/// Checks an identified model against the loop before it becomes the selected model (`qualifyModel` of
/// `acceptIdentificationResult`): its evidence and, for one input and one output, their units and ranges. A model that does
/// not qualify is replaced by identifying one that does (B4.7). An accepted identification's model is selected and checked by
/// the session that accepts it; a single connected run never gives one.
///
/// # Errors
/// The model does not qualify for the loop; the failure names the check.
pub fn qualification<E>(
    loop_definition: &LoopDefinition,
    result: &AutomaticIdentificationResult<E>,
) -> Result<(), Failure> {
    let Some(model) = fitted_model(result) else {
        return Ok(());
    };
    assert_qualified_model_result(loop_definition, model).map_err(|refusal| {
        // The model's input meets the actuator range, which the output limits give: a missing or invalid limit is the
        // precondition's refusal, and Limits opens there (B3.9). Any other refusal is identified again.
        match (&refusal, plant_input_range(loop_definition)) {
            (CompatibilityRefusal::InputActuatorRange { .. }, Err(limits)) => limits,
            _ => Failure::new(
                &compatibility_text(&refusal),
                Cause::ModelOrData,
                NextAction::Identify,
            ),
        }
    })
}

/// The refusal of a physical target on this path (A2.5). Declining approval offers no action (B3.9).
fn physical_target() -> Failure {
    Failure::new(PHYSICAL_TARGET_TEXT, Cause::Approval, NextAction::None)
}

/// The refusal of a start whose review no longer describes the campaign (§0.1a, E11.8). It offers no action of its own: the
/// review is asked for again (B3.9).
#[must_use]
pub fn review_changed_refusal() -> Failure {
    Failure::new(REVIEW_CHANGED_TEXT, Cause::Approval, NextAction::None)
}

/// The restoration a status answer reports, when it is one of the runtime's words (E5.6).
fn restoration_of(state: &Value) -> Option<RestorationState> {
    state
        .get("restoration")
        .cloned()
        .and_then(|value| crate::json::decode_value(value).ok())
}

/// A time in seconds as whole milliseconds, rounded down, or `None` below 1 ms.
// Checked: at least 1. A float-to-integer cast saturates, so an infinite time keeps the longest record period, as
// JavaScript's `Infinity` does.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_milliseconds(seconds: f64) -> Option<u64> {
    let milliseconds = (seconds * 1_000.0).floor();
    (milliseconds >= 1.0).then_some(milliseconds as u64)
}

#[cfg(test)]
#[path = "connected_run_tests.rs"]
mod tests;
