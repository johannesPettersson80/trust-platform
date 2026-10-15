//! Identify on a connected runtime (`connectedIdentification.ts` and `engineerActions/find.ts`; specification 35, A2.5, A6.2,
//! A8.3 to A8.6, A9, A10 and B3; runtime protocol annex E5 and E11): the live campaign that records the plant, steps its input
//! once, holds the step until it settles, confirms the restoration and fits the recording, and the routing of the Identify
//! action around it. Every decision is in `trust-control` (`identification::connected_run` and the rules it calls); this module
//! makes the calls those decisions need, in their order, and reaches the runtime only through [`IdentifyRuntime`].
//!
//! One plant input only: stepping several inputs one after another (`connectedIdentificationChannels.ts`) is parked with stage
//! V7, so the loop is the one channel. A method that needs the other mode enters it through a transfer before anything is
//! recorded, and the loop returns to the mode it was found in once nothing of the run is left running (`identify_mode`, B3.4
//! step 1, A6.2). The engine outlives a closed panel, so the release of a closed panel (`dispose`) has no counterpart here: the
//! runtime keeps its experiment and recording, and the journal stays (A8.5, A8.6).
//! The evaluation of a connected run is the caller's ([`RunEvaluator`]): production passes the limits check of
//! `recorded_limits` (B3.4 step 6), and the requirement evaluation of N8 is stage V3.

// The Identify types keep the name of the action they serve.
#![allow(clippy::module_name_repetitions)]

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;
use trust_control::experiment::{ControlExperimentPlan, ExperimentSample, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{
    configured_acquisition, AcquisitionSettings,
};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::connected_record::{
    BoundsCheck, RuntimeVerificationResult, ValidationRun,
};
use trust_control::identification::connected_run::{
    live_write_refusal, qualification, StartRead, IDENTIFY_OPERATION,
};
use trust_control::identification::independent_validation::RunEvaluation;
use trust_control::identification::policy::IdentificationMethod;
use trust_control::identification::reconcile::StatusRead;
use trust_control::identification::texts::MODE_TRANSFER_UNAVAILABLE;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{CampaignRestoration, RecordedChartSample};
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::RecordedRun;
use trust_control::runtime::RuntimeConnection;

use super::identify_campaign::{ExperimentClient, RecordingFollower, SystemClock};
use crate::identify_action::identify_manual::is_project_test;
use crate::journal::Journal;
use crate::operation::OperationContext;
use crate::runtime_client::recording_coordinator::RecordingSession;
use crate::runtime_client::runtime_recording::{
    PendingReleases, RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
};
use crate::runtime_client::RuntimeClient;

pub(crate) mod cancel;
mod connected;
#[cfg(test)]
mod final_wiring_tests;
#[cfg(test)]
mod long_campaign_tests;
#[cfg(test)]
mod project_test_tests;
pub(crate) mod run;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
pub(crate) mod test_support;

pub use connected::ConnectedRuntime;

/// The evaluation of a connected run against the limits of its record (B3.4 step 6; the production evaluation is
/// `recorded_limits::evaluate_recorded_limits`). The record keeps what it returns, and only a run whose evaluation passes is
/// fitted.
pub type RunEvaluator<'a, E> = &'a dyn Fn(&ValidationRun, &[BoundsCheck]) -> Result<E, Failure>;

/// The live sample of the loop a run identifies (`ports.sample()`): one coherent scan of its signals, read when the run starts
/// and again when the step is planned.
pub trait LoopSampler {
    /// The loop's sample, or `None` when no fresh synchronized sample is available.
    ///
    /// # Errors
    /// The runtime cannot be reached, or refuses the inspection.
    fn sample(&mut self) -> Result<Option<ExperimentSample>, Failure>;
}

/// Where a run keeps its stopped recording (`ports.retain`): in the study, before the runtime buffer is released (E4.7, E11.6).
pub trait RecordingRetention {
    /// Keeps the stopped `run` with `evidence`, the verdict that labels it, so an unconfirmed start or an incomplete recording
    /// stays labelled (E4.5, E11.8).
    ///
    /// # Errors
    /// The recording cannot be kept. The runtime buffer is then not released, so the keep can be retried.
    fn retain(&mut self, run: &RecordedRun, evidence: &RecordingEvidence) -> Result<(), Failure>;
}

/// The recording a run follows, stops, keeps and releases: the runtime's recording session, or a test's.
pub trait IdentifyRecording: RecordingFollower + RecordingSession<Recording = RecordedRun> {
    /// The recording's owner, which the journal keeps (E11.2).
    fn owner(&self) -> RecordingRecoveryOwner;

    /// The verdict on a stopped run of this recording: complete only when its acquisition is, and never when its start was not
    /// acknowledged (E4.5, E11.8).
    fn evidence(&self, run: &RecordedRun) -> RecordingEvidence;
}

/// The runtime a run works on (the runtime requests and the recording start of `ConnectedIdentification`): its recordings, the
/// experiment client a campaign holds for its baseline and hold, and the run's own start and status reads.
pub trait IdentifyRuntime {
    /// The recording the runtime starts and recovers.
    type Recording: IdentifyRecording;
    /// The experiment client of the campaign.
    type Experiment: ExperimentClient;

    /// Starts a recording of `loop_definition`; its owner is journaled through `persistence` before the start is sent (E11.2).
    ///
    /// # Errors
    /// The start failed, or its acknowledgement was lost and its owner is kept (E11.4).
    fn start_recording(
        &self,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<Self::Recording, RecordingStartError<Self::Recording>>;

    /// The session of a journaled recording, which can only stop and release it (E11.6).
    ///
    /// # Errors
    /// The owner does not belong to `loop_definition`.
    fn recover_recording(
        &self,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
    ) -> Result<Self::Recording, Failure>;

    /// A client of the runtime's experiment, for a campaign's baseline and hold or a confirmation of a restoration.
    fn experiment(&self) -> Self::Experiment;

    /// Sends the start of `plan` once, and says what came back (E5.3, E0.4).
    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead;

    /// Reads the status of `experiment_id` and says what came back, with a refusal's code (E5.6, E11.4).
    fn experiment_status(&self, experiment_id: &str) -> StatusRead;

    /// The control connection on which a mode change transfers the loop (annex E6): a method that needs the other mode enters it
    /// through a transfer, and the loop returns to the mode it was found in (B3.4 step 1, A6.2). A runtime that takes no
    /// transfers refuses, which is the default.
    ///
    /// # Errors
    /// The runtime takes no controller transfers.
    fn transfer_connection(&self) -> Result<RuntimeConnection, Failure> {
        Err(Failure::new(
            MODE_TRANSFER_UNAVAILABLE,
            Cause::Unclassified,
            NextAction::None,
        ))
    }

    /// The runtime's I/O list (`io.list`), which gives the address of each variable by its name (C7). A runtime that has none
    /// answers `null`, so no command has an address.
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses the request.
    fn io_list(&self) -> Result<Value, Failure> {
        Ok(Value::Null)
    }
}

/// What a connected run identifies and how it judges its recording.
pub struct IdentifyRequest<'a, E> {
    /// The loop. [`identify_plant`] applies the acquisition settings to it (`configuredAcquisition`) and qualifies the model
    /// against it; [`run_identification`] runs it as given.
    pub loop_definition: &'a LoopDefinition,
    /// The output's expected settling time in s, when the study declares one or Validate's requirements set it (B3.4 step 2).
    /// The recording's record period follows it (A4.2).
    pub expected_settling_seconds: Option<f64>,
    /// The evaluation of the connected run: production passes `recorded_limits::evaluate_recorded_limits` (B3.4 step 6).
    pub evaluate: RunEvaluator<'a, E>,
}

/// Whether a run identifies the plant, records the independent model check, or runs a project test (C12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPurpose {
    /// Records the plant and fits the recording (`run`).
    Identify,
    /// Records the opposite excitation for the independent model check and does not fit it (`recordValidation`, B5.2).
    RecordValidation,
    /// Records the project's own scenario and fits it (C12). A fit that the recording cannot support does not fail the test: the run
    /// completes with the fit's refusal as its note, and keeps no model.
    ProjectTest,
}

/// The ports a run reads and writes beside the runtime: the live sample and the study.
pub struct IdentifyPorts<'a, S, K> {
    /// The loop's live sample.
    pub sampler: &'a mut S,
    /// Where the recording is kept.
    pub retention: &'a mut K,
}

/// What a run gave.
#[derive(Debug, Clone, PartialEq)]
pub enum IdentifyResult<E> {
    /// The fitted identification (`run`).
    Identified {
        /// The model and the records it was fitted from.
        result: AutomaticIdentificationResult<E>,
        /// Whether the model is provisional: a fit from the run's own recording is never validated evidence (specification
        /// "Single experiment and provisional model"), so it can be tuned but cannot authorize Apply.
        provisional: bool,
    },
    /// The recorded run that is not fitted: the independent model check (`recordValidation`), or a project test whose fit refused (C12).
    Recorded(Box<RuntimeVerificationResult<E>>),
}

/// The outcome of one connected run (`run` and `recordValidation` of `ConnectedIdentification`).
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifyOutcome<E> {
    /// What the run gave.
    pub result: IdentifyResult<E>,
    /// The runtime recording the result came from: the run's identifier in its record.
    pub recording_id: String,
    /// The method the run used.
    pub method: IdentificationMethod,
    /// The plant input the run stepped, by symbol.
    pub plant_input: String,
    /// The measured output the run recorded, by symbol.
    pub measured_output: String,
    /// The restoration the run's experiments confirmed (C12); none when the run completed no step.
    pub restoration: Option<CampaignRestoration>,
    /// The samples of the kept recording, which the answer hands the page to draw its recorded signals from.
    pub recorded_samples: Vec<RecordedChartSample>,
    /// The fit's refusal of a project test whose recording the fit cannot support, in full (C12): the test completed with its recording
    /// and keeps no model. None for every other run.
    pub fit_refusal: Option<String>,
}

/// Runs one connected identification (`run`) or records the independent model check (`recordValidation`) on the runtime of
/// `connection`, reporting through `context` and stopping at its next step once it is cancelled. The journal is the loop's
/// journal in `storage_folder` (A8.3). A method that needs the other mode enters it through a transfer first (B3.4 step 1).
/// Whatever happens, the run then settles: it ends an experiment it owns through the runtime's restoration path, and once no
/// experiment of it is left running it keeps its recording, clears its journal and returns the loop to the mode it was found
/// in (A6.2). An experiment whose state is unknown is left to the runtime with its journal and the mode (A8.5, E11.4).
///
/// # Errors
/// The journal cannot be read; the sample, the method, a mode that cannot be read, a limit or the target refuses the run before
/// anything is recorded; the loop cannot be moved to the mode the method needs; the recording, the baseline, the start, the
/// hold, the restoration, the record or the fit fails; the settling fails, whose failure then replaces the run's; or the loop
/// cannot be returned to its mode, whose failure replaces only a run that succeeded.
pub fn run_identification<S, K, E>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &IdentifyRequest<'_, E>,
    purpose: RunPurpose,
    IdentifyPorts { sampler, retention }: IdentifyPorts<'_, S, K>,
) -> Result<IdentifyOutcome<E>, Failure>
where
    S: LoopSampler,
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    let loop_definition = request.loop_definition;
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    let runtime = ConnectedRuntime::new(connection, pending);
    let clock = SystemClock::new();
    run::run(
        run::RunPorts {
            runtime: &runtime,
            sampler,
            retention,
            clock: &clock,
            report: context,
            journal: &journal,
        },
        request,
        purpose,
    )
}

/// The Identify action (`identifyPlant` of `engineerActions/find.ts`): the loop configured by `settings` (the engineer's
/// acquisition settings, checked with `checked_acquisition_settings` when a request carries them), the authority of a live
/// write on the target (A2.5), the connected run, and the qualification of the identified model for the loop.
///
/// # Errors
/// The settings cannot configure the loop; the target is physical, which needs an approval this path does not carry; the run
/// fails (see [`run_identification`]); or the identified model does not qualify for the loop.
pub fn identify_plant<S, K, E>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &IdentifyRequest<'_, E>,
    settings: Option<&AcquisitionSettings>,
    ports: IdentifyPorts<'_, S, K>,
) -> Result<IdentifyOutcome<E>, Failure>
where
    S: LoopSampler,
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    // A project test (C12) runs the project's scenario as a test: a refused fit is its note, not its failure.
    let project_test =
        settings.is_some_and(|settings| is_project_test(request.loop_definition, settings.method));
    let purpose = if project_test {
        RunPurpose::ProjectTest
    } else {
        RunPurpose::Identify
    };
    let outcome = run_for_purpose(
        context,
        connection,
        pending,
        storage_folder,
        request,
        settings,
        ports,
        purpose,
    )?;
    if let IdentifyResult::Identified { result, .. } = &outcome.result {
        qualification(request.loop_definition, result)?;
    }
    Ok(outcome)
}

/// The record of a project test that the manual identification awaits as its independent recording (B3.3, C12, B5.2): the run takes
/// the campaign that `identify_plant` runs and keeps its recording, but it fits nothing (`RunPurpose::RecordValidation`), so the test's
/// own model is not kept and a refusal of a fit cannot end the record.
///
/// # Errors
/// As [`identify_plant`], without the qualification of a fitted model.
pub fn record_for_manual_check<S, K, E>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &IdentifyRequest<'_, E>,
    settings: Option<&AcquisitionSettings>,
    ports: IdentifyPorts<'_, S, K>,
) -> Result<IdentifyOutcome<E>, Failure>
where
    S: LoopSampler,
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    run_for_purpose(
        context,
        connection,
        pending,
        storage_folder,
        request,
        settings,
        ports,
        RunPurpose::RecordValidation,
    )
}

/// The run of `purpose` on the acquisition that `settings` configure: the checks that come before any run, then the run itself.
#[allow(clippy::too_many_arguments)] // The run's inputs, as `identify_plant` takes them, and the purpose.
fn run_for_purpose<S, K, E>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &IdentifyRequest<'_, E>,
    settings: Option<&AcquisitionSettings>,
    ports: IdentifyPorts<'_, S, K>,
    purpose: RunPurpose,
) -> Result<IdentifyOutcome<E>, Failure>
where
    S: LoopSampler,
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    let acquisition = configured_acquisition(request.loop_definition, settings)
        .map_err(|error| error.failure())?;
    let source = RuntimeClient::connect(connection)?.status()?.source();
    if let Some(refusal) = live_write_refusal(source, IDENTIFY_OPERATION) {
        return Err(refusal);
    }
    let configured = IdentifyRequest {
        loop_definition: &acquisition,
        ..*request
    };
    run_identification(
        context,
        connection,
        pending,
        storage_folder,
        &configured,
        purpose,
        ports,
    )
}

/// The engineer's Cancel of an identification whose run has ended but left work owned (`cancel` of `ConnectedIdentification`):
/// the campaign journaled in `storage_folder` for `loop_definition`. An experiment of it that still runs is aborted through
/// the runtime's restoration path, and its restoration is confirmed within 50 status reads 100 ms apart; an ended one has its
/// restoration confirmed; one the runtime does not hold never ran, so nothing is aborted. Its recording is then stopped and
/// kept through `retention` before its buffer is released, and the journal is cleared. Nothing else is aborted or restored.
///
/// # Errors
/// The journal cannot be read or cleared; the experiment's restoration cannot be confirmed (reconnect the original runtime);
/// or the recording cannot be recovered, stopped or kept. The journal then stays.
pub fn cancel_identification<K: RecordingRetention>(
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
    retention: &mut K,
) -> Result<(), Failure> {
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    cancel::cancel_journaled(
        &ConnectedRuntime::new(connection, pending),
        &SystemClock::new(),
        &journal,
        loop_definition,
        retention,
    )
}
