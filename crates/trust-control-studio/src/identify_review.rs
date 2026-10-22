//! The review of the campaign that `controlStudio/identify` would run, and the engine's check of a start against it (specification
//! 35, §0.1a, A2.5, E11.8, B3.2 to B3.4 and C12). A project test's review needs the runtime: it reads the hold that the runtime applies
//! and the experiment settings that its identity binds, and a runtime that is not running is refused with the next action that applies
//! (A2.8). The rules are trust-control's (`campaign_review`, `simulator_start`); this module reads the runtime.

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::identification::acquisition_settings::AcquisitionSettings;
use trust_control::identification::campaign_review::{
    bound_to_settings, campaign_review, IdentifyReview,
};
use trust_control::identification::connected_run::{
    live_write_refusal, review_changed_refusal, IDENTIFY_OPERATION,
};
use trust_control::identification::policy::project_scripted;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::simulator_start::runtime_not_running;

use super::runtime_source;
use crate::commands::to_value;
use crate::identify_adapters::RuntimeSampler;
use crate::identify_schedule;
use crate::runtime_client::project_experiment_hold::applied_project_settings_over;
use crate::workflow::identify::LoopSampler;

/// The review of the campaign that `identify` would run with `settings` (§0.1a, B3.2 to B3.4, C12): the loop's live sample is read
/// when a runtime is connected, so the mode Automatic chooses from is the controller's. Without a connection the sample is absent,
/// and the review says what the campaign needs. A project test needs the runtime, which holds its hold and its experiment settings
/// and states the target's kind (A2.4): without a connection it is refused with the next action that applies to `runtime_mode`,
/// Start simulator or Connect runtime (A2.8). A project test reads the hold the runtime applies and the experiment settings, which
/// its identity binds (C12). It starts nothing and writes nothing.
///
/// # Errors
/// A project test without a runtime connection; the runtime cannot be reached for its sample, the loop's hold or its settings; or the
/// campaign's own checks refuse the settings, or the hold (trust-control's `campaign_review`, with the next action it names).
pub(super) fn review(
    loop_definition: &LoopDefinition,
    connection: Option<&RuntimeConnection>,
    runtime_mode: Option<&str>,
    settings: &AcquisitionSettings,
) -> Result<Value, Failure> {
    if connection.is_none() && project_scripted(loop_definition).is_some() {
        return Err(runtime_not_running(runtime_mode));
    }
    let sample = match connection {
        Some(connection) => {
            let source = runtime_source(connection)?;
            RuntimeSampler::new(connection, loop_definition, source).sample()?
        }
        None => None,
    };
    let applied = applied_project_settings_over(loop_definition, connection)?;
    let reviewed = campaign_review(
        loop_definition,
        settings,
        sample.as_ref(),
        applied.hold_seconds,
    )?;
    to_value(&bound_to_settings(reviewed, applied.values.as_ref()))
}

/// The engine's check of a start that `controlStudio/identify` runs inside its operation (§0.1a, A2.5, E11.8): the campaign the
/// start would run now, on the live target, must be the one the engineer reviewed, and on a physical target the engineer must
/// have approved it. A refused start writes nothing, and its operation ends with the refusal. The scheduling variable of a
/// gain-scheduled loop is checked against its operating point in the same scan as the sample (C10 Identify, D-21); the value that
/// it gives is returned for the identification to keep. A project test is reviewed again with the hold the runtime applies now and the
/// experiment settings it holds now (C12, Hold parameter and Parameter purpose), so a hold whose recording differs from the reviewed
/// one, or a setting changed since the review, is another campaign, and the start is refused.
///
/// # Errors
/// The runtime cannot be read for its sample, the loop's hold or its settings; the campaign's own checks refuse the settings or the
/// hold; [`start_refusal`] refuses the start; or the scheduling variable cannot be read or is outside the tolerance of its point
/// ([`identify_schedule::scheduling_start`]).
pub(super) fn confirm_start(
    loop_definition: &LoopDefinition,
    connection: &RuntimeConnection,
    settings: &AcquisitionSettings,
    review: Option<&str>,
    physical_approved: bool,
) -> Result<Option<f64>, Failure> {
    let source = runtime_source(connection)?;
    let sampler = RuntimeSampler::new(connection, loop_definition, source);
    let scan = sampler.scan()?;
    let sample = scan.as_ref().and_then(|scan| sampler.sample_of(scan));
    let applied = applied_project_settings_over(loop_definition, Some(connection))?;
    let reviewed = bound_to_settings(
        campaign_review(
            loop_definition,
            settings,
            sample.as_ref(),
            applied.hold_seconds,
        )?,
        applied.values.as_ref(),
    );
    if let Some(refusal) = start_refusal(&reviewed, review, physical_approved) {
        return Err(refusal);
    }
    let recorded = identify_schedule::value_in_scan(loop_definition, scan.as_ref());
    identify_schedule::scheduling_start(loop_definition, recorded)
}

/// The refusal of a start, from the review the engineer saw and the approval the start carries (§0.1a, A2.5, E11.8). A review
/// that names another identity than the campaign reviewed now is refused. On a physical target, which the reviewed campaign
/// shows with an approval block, the start needs both the review and the engineer's approval; the approval covers that identity
/// and nothing else. A start that names no review passes on a simulated target, where pressing the action is the confirmation.
/// `None` when the start may run.
pub(super) fn start_refusal(
    reviewed: &IdentifyReview,
    review: Option<&str>,
    physical_approved: bool,
) -> Option<Failure> {
    if review.is_some_and(|identity| identity != reviewed.identity) {
        return Some(review_changed_refusal());
    }
    if physical_approved && review.is_some() {
        return None;
    }
    let source = if reviewed.approval.is_some() {
        EvidenceSource::PhysicalPlant
    } else {
        EvidenceSource::SimulatedPlant
    };
    live_write_refusal(source, IDENTIFY_OPERATION)
}
