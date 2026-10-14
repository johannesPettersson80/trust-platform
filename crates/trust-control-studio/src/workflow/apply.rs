//! The Apply workflow of Control Studio (specification 35, §0.1b, B6.1 to B6.3, B6.7, A2.5, A6.2 to A6.4, A8.5, C1 and C13; annex E6,
//! E7, E11.2 and E11.10; V4 decisions 1 to 5). `review_apply` answers what the confirmation would write, read from the study and the live
//! runtime, and writes nothing; `run_apply` runs the reviewed Apply while the runtime still gives the review's identity.
//!
//! The run follows `execute` of `pidCommissioning.ts` and `proportionalCommissioning.ts` with trust-control's decisions: wait for the
//! starting point, journal the commissioning checkpoint before the first write, stage Manual when needed, apply and confirm the
//! candidate's record, enter Auto, and observe the validated target change (`apply_observation`). A failed requirement keeps the settings
//! applied and offers Restore; a departure from the permitted range, a cancel or any other failure after the first write runs the one
//! Restore at once (`workflow::restore`). The study's receipts are the Apply action's.
//!
//! The modules: `review` (the preconditions, the study's inputs and the review), `law` (the PID and proportional laws), `wait` (the
//! starting point), `journaling` (the checkpoint), `run` (the steps after the confirmation) and `ending` (the Restore, the recording kept
//! and the outcome).

#![allow(clippy::module_name_repetitions)] // `ApplyContext`, `review_apply` and `run_apply` are the V4 contract's names.

mod ending;
mod journaling;
mod law;
mod review;
mod run;
mod wait;

#[cfg(test)]
pub(crate) mod test_support;

use std::path::Path;
use std::time::{Duration, Instant};

use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::application_records::{
    approval_block, ApplicationReview, ApplyOutcome, ReviewedAction,
};
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;
use trust_control::study::Study;

use crate::journal::Journal;
use crate::operation::OperationContext;
use crate::workflow::identify_campaign::CampaignReport;

pub(crate) use ending::{
    keep_recording, outcome_of, release_kept, restore_at_once, stop_unfinished,
};
pub(crate) use journaling::{checkpoint_stage, CheckpointWriter};
pub(crate) use law::law_target;
pub(crate) use review::{candidate_gains, measured_noise};

use law::{LawSetup, LawStart};
use review::{preconditions, prepare, reviewed_parameters, ApplyInputs, Clients, Prepared};

/// What an Apply works with (B6.1, A2.5, A8.3): the loop, its study and its runtime, the loop's operation journal, whether the engineer
/// approved a write on a physical target, and the project folder whose study keeps the recordings.
pub struct ApplyContext<'a> {
    /// The loop being applied.
    pub loop_definition: &'a LoopDefinition,
    /// The loop's working study: its validated candidate, its model and its requirements.
    pub study: &'a Study,
    /// The runtime the Apply reads and writes (E11.1).
    pub connection: &'a RuntimeConnection,
    /// The loop's operation journal, where the commissioning checkpoint is kept (A8.5).
    pub journal: &'a mut Journal,
    /// Whether the engineer approved a write on a physical target immediately before it starts (A2.5).
    pub physical_approved: bool,
    /// The project folder, whose study keeps the recordings the model was fitted from (A8.3).
    pub project_root: &'a Path,
}

/// The pauses of an Apply's waits and follows, and this receiver's own clock for the connection-loss rule (A3.4).
pub(crate) struct ApplyTiming<'t> {
    /// The pause between two readings of the starting point (B6.2; `pollMs` of `awaitCommissioningStartingPoint`).
    pub(crate) wait_poll: Duration,
    /// The pause between two pages of the observed recording (B6.3; `pollMs` of `followAppliedResponse`).
    pub(crate) follow_poll: Duration,
    /// This receiver's wall clock, in ms (A3.4); never compared with the runtime's clock.
    pub(crate) wall_ms: &'t dyn Fn() -> u64,
}

impl<'t> ApplyTiming<'t> {
    /// The pauses of a running engine (`awaitCommissioningStartingPoint`: 500 ms; `followAppliedResponse`: 250 ms) with `wall_ms`.
    #[must_use]
    pub(crate) fn production(wall_ms: &'t dyn Fn() -> u64) -> Self {
        Self {
            wait_poll: Duration::from_millis(500),
            follow_poll: Duration::from_millis(250),
            wall_ms,
        }
    }
}

/// This receiver's wall clock in ms, from the moment it is made (A3.4).
pub(crate) fn wall_clock() -> impl Fn() -> u64 {
    let started = Instant::now();
    move || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The review of an Apply (§0.1b, B6.1, B6.2; V4 decision 1): the mode the loop is in, each parameter's running and candidate value,
/// the sentence of the reviewed run, whether the target is physical, the validated operating point, and the identity the confirmation
/// names. It reads the study and the runtime, writes nothing and starts no operation.
///
/// # Errors
/// In this order: the study holds no current validation that passed (Validate); the Rust engine does not run the loop; the loop's
/// journal holds an unfinished operation other than an earlier Apply's checkpoint (Wait); the runtime cannot be reached or refuses; the
/// journal holds an earlier Apply's unresolved checkpoint of the running program (Restore, E11.7); or the adapter, the record or the law
/// refuses the loop. A completed, recovered or retired checkpoint, and an unresolved one of a replaced runtime (B6.4), do not refuse: the
/// run replaces or retires them. A physical target is reviewed and marked `physical`, since the engineer approves the reviewed write
/// (A2.5); its confirmation needs that approval.
pub fn review_apply(context: &mut ApplyContext<'_>) -> Result<ApplicationReview, Failure> {
    let inputs = preconditions(context)?;
    review_with(context, &inputs)
}

/// The review of [`review_apply`] once the preconditions that need no runtime hold, with what the study gives in `inputs`.
pub(crate) fn review_with(
    context: &mut ApplyContext<'_>,
    inputs: &ApplyInputs,
) -> Result<ApplicationReview, Failure> {
    let mut clients = Clients::connect(context.connection)?;
    let prepared = prepare(context, inputs, &mut clients)?;
    let parameters = reviewed_parameters(&prepared.runtime_binding, &prepared.review);
    let Prepared {
        source,
        kind,
        binding,
        review,
        first,
        found_mode,
        identity,
        ..
    } = prepared;
    let start = LawStart::begin(kind, setup_of(context, inputs), binding, review, &first)?;
    // A2.5: what a physical approval names, for the parameters the confirmation writes and the observation's duration.
    let approval = approval_block(
        context.loop_definition,
        ReviewedAction::Apply,
        &parameters,
        Some(inputs.task.duration_seconds),
    );
    Ok(ApplicationReview {
        action: ReviewedAction::Apply,
        identity,
        found_mode,
        parameters,
        description: start.description().to_owned(),
        physical: source == ApplySource::Physical,
        operating_point: inputs.model.identity().operating_point.clone(),
        approval,
    })
}

/// Runs the reviewed Apply (B6.1 to B6.3, §0.1b Confirmation; V4 decisions 1, 3 and 4) as the operation `operation`: refused when the
/// identity recomputed from the live runtime is not `identity`, then waited, written, observed and assessed. Its outcome is the result,
/// whether the one Restore confirmed the previous settings, and whether Restore and Keep in project are offered.
///
/// # Errors
/// The preconditions of [`review_apply`]; the runtime changed since the review (`review_changed`); the target is physical and `context`
/// carries no approval (A2.5); or a refusal before the first write, such as a cancelled wait or a plant input away from its validated
/// point. A failure after the first write is the outcome's result.
pub fn run_apply(
    context: &mut ApplyContext<'_>,
    operation: &OperationContext,
    identity: &str,
) -> Result<ApplyOutcome, Failure> {
    let clock = wall_clock();
    run_reporting(
        context,
        operation,
        identity,
        &ApplyTiming::production(&clock),
    )
}

/// [`run_apply`], reporting to `report` with the pauses and the clock of `timing`.
pub(crate) fn run_reporting(
    context: &mut ApplyContext<'_>,
    report: &dyn CampaignReport,
    identity: &str,
    timing: &ApplyTiming<'_>,
) -> Result<ApplyOutcome, Failure> {
    let inputs = preconditions(context)?;
    run::run_confirmed(context, &inputs, identity, report, timing)
}

/// What the laws begin with: the loop, and the study's model, requirements and measured noise.
fn setup_of<'a>(context: &ApplyContext<'a>, inputs: &'a ApplyInputs) -> LawSetup<'a> {
    LawSetup {
        loop_definition: context.loop_definition,
        model: &inputs.model,
        task: &inputs.task,
        process_noise: inputs.process_noise,
    }
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
