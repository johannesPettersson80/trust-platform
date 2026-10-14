//! The Tune method `controlStudio/tune` (specification 35, §0.1a, §0.1b, A8.4 to A8.6 and B4.1 to B4.7; Tune page specification).
//!
//! `tune` designs a candidate for the loop's controller from the study's current model and keeps it, unapplied, in the study's
//! Tune section (A8.1). It answers at once with its operation's identifier. The operation's final update carries trust-control's
//! `TuneResult`: the candidate, its predicted response, its margins and the stages.
//!
//! Tune never reaches the runtime and never writes to the controller (B4.1), so its request carries no runtime connection. It
//! also keeps no journal: the study is all it writes, and it owns no runtime operation that a restart would have to reattach
//! (A8.5). The request's `storageFolder`, the folder of the operation journals, is therefore not read here.
//!
//! What can be refused without the design is refused before anything starts: the loop must open, no operation of this engine
//! may work on it, and the study must hold a current model that the request's method can tune (B4.7, A8.4). Those last checks
//! are trust-control's `check_before_start`, and every rule of the design itself is trust-control's too. This module claims the
//! loop, runs the design as an operation and stores the section it produces. The operation reports one short progress message
//! (Tune page specification) and checks for a cancel before its only write, so an aborted Tune keeps nothing.
//!
//! The design takes the requirements the study holds: the ones the engineer recorded in the study's inputs (B4.3), when there
//! are any. The relay rule (numerical annex N5.R) is the one method that needs more than the study's sections: when the request
//! asks for it, the engine reads the study's estimation recording, and trust-control decides what the relay cycle in it qualifies
//! as.
//!
//! [`run_stage`] is the same Tune for a run (`controlStudio/run`, B8): it makes the same checks and the same design inside the
//! run's own operation, and claims nothing, because the run holds the loop.

use std::path::{Path, PathBuf};

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{TuneMethod, TuneRequest};
use trust_control::study::identify::IdentifySection;
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::{
    check_before_start, relay_evidence_for, tune_held, TuneInputs, TuneSection,
};
use trust_control::tuning::relay_rule::RelayCycleEvidence;
use trust_control::tuning::requirements::TuningRequirements;
use trust_control::tuning::texts;

use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::operation::{cancelled, OperationContext, Operations};
use crate::{loops, storage};

/// The command of the operation that `tune` starts (§0.1a).
const TUNE_COMMAND: &str = "tune";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const OPERATION_NAME: &str = "Tune";

/// What the Tune operation works with: owned, so that it moves to the operation's thread.
struct TuneWork {
    /// The project folder, where the loop's study is.
    project_root: PathBuf,
    loop_definition: LoopDefinition,
    /// The study's Identify section, or `None` when the study has none.
    identify: Option<IdentifySection>,
    /// The study's Tune section from before, whose supplied requirements the design keeps (B4.3).
    previous: Option<TuneSection>,
    /// The requirements the engineer recorded in the study's inputs, which the design takes as given (B4.3), when there are any.
    held: Option<TuningRequirements>,
    /// The relay cycle of the study's estimation recording, read for the relay rule only (N5.R).
    relay: Option<RelayCycleEvidence>,
    request: TuneRequest,
}

impl TuneWork {
    /// What trust-control checks and designs from: the loop, the study's sections and the request.
    fn inputs(&self) -> TuneInputs<'_> {
        TuneInputs {
            loop_definition: &self.loop_definition,
            identify: self.identify.as_ref(),
            previous: self.previous.as_ref(),
            relay: self.relay.as_ref(),
            request: &self.request,
        }
    }
}

/// Answers `controlStudio/tune` (B4): designs the candidate for the loop's controller from the study's current model, keeps it
/// unapplied in the study's Tune section and answers at once with the operation's identifier; the result follows as the
/// operation's final update. The loop is first claimed in `claims` (one operation per loop at a time, A8.5); the operation
/// holds the claim until it ends, and a refused request lets it go at once.
///
/// # Errors
/// The loop cannot be opened, an operation of this engine works on it, or its study, one of its sections or the estimation
/// recording of the relay rule cannot be read; or the study holds no current model that the request's method can tune (B4.7),
/// which trust-control's `check_before_start` decides, with the next action it names.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: TuneRequest,
) -> Result<Value, Failure> {
    // The request's `storage_folder` is not read: Tune keeps no journal (B4.1, A8.5). The request stays whole, because the
    // rules read the rest of it (method, choice, structure and pins) through `TuneInputs`.
    let project_root = PathBuf::from(&request.project_root);
    let loop_definition = loops::open(&project_root, &request.runtime_id, &request.loop_id)?;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let work = study_work(&project_root, &loop_definition, request)?;
    // A refusal returns here, and the claim ends with it.
    check_before_start(&work.inputs())?;
    // The claim ends before the operation reports its end; a panic of the work ends it too, because the claim is dropped
    // with the work.
    let started = operations.start(TUNE_COMMAND, move |context| {
        let worked = run_tune(context, &work);
        drop(claim);
        worked
    });
    to_value(&started)
}

/// The Tune stage of a run (B8): the same checks and the same design as `controlStudio/tune`, on the study as it is, inside the
/// run's own operation. The run holds the loop's claim, so nothing is claimed here, and its Cancel reaches the design through
/// `context`.
///
/// # Errors
/// The study or its sections cannot be read; the checks before the start refuse the design (B4.7); or the design fails.
pub(crate) fn run_stage(
    context: &OperationContext,
    project_root: &Path,
    loop_definition: &LoopDefinition,
    request: TuneRequest,
) -> Result<Value, Failure> {
    let work = study_work(project_root, loop_definition, request)?;
    check_before_start(&work.inputs())?;
    run_tune(context, &work)
}

/// The study's sections and the requirements it holds, read for one Tune: the loop's Identify and Tune sections, the recorded
/// requirements of the study's inputs, and the relay cycle the request needs.
///
/// # Errors
/// The study, one of its sections or the relay rule's estimation recording cannot be read.
fn study_work(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    request: TuneRequest,
) -> Result<TuneWork, Failure> {
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let identify = study.section::<IdentifySection>()?;
    let relay = relay_evidence(project_root, loop_definition, identify.as_ref(), &request)?;
    Ok(TuneWork {
        previous: study.section::<TuneSection>()?,
        held: study
            .section::<InputsSection>()?
            .and_then(|inputs| inputs.requirements),
        identify,
        relay,
        project_root: project_root.to_path_buf(),
        loop_definition: loop_definition.clone(),
        request,
    })
}

/// The relay cycle of the study's estimation recording (N5.R), read only when the request asks for the relay rule: the file is
/// read here, and trust-control decides what the cycle qualifies as. `None` when the request names another method, or the study
/// has no Identify result.
///
/// # Errors
/// The estimation recording that the Identify result names cannot be read.
fn relay_evidence(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    identify: Option<&IdentifySection>,
    request: &TuneRequest,
) -> Result<Option<RelayCycleEvidence>, Failure> {
    if request.method != Some(TuneMethod::Relay) {
        return Ok(None);
    }
    let Some(identify) = identify else {
        return Ok(None);
    };
    let recording = storage::read_recording(
        project_root,
        &loop_definition.id,
        &identify.estimation_recording_id,
    )?;
    Ok(relay_evidence_for(identify, loop_definition, &recording))
}

/// The design (B4.4 to B4.6) on the study's model, kept unapplied in the study's Tune section (A8.1, N8.9); its result is the
/// operation's. A cancelled operation keeps nothing.
fn run_tune(context: &OperationContext, work: &TuneWork) -> Result<Value, Failure> {
    context.progress(texts::DESIGNING);
    let outcome = tune_held(&work.inputs(), work.held.as_ref())?;
    if context.is_cancelled() {
        return Err(cancelled());
    }
    // The study is read again for the write, so that a page the engineer chose while the design ran is kept (A8.1), and so are
    // the sections another request of the project replaced meanwhile. The recordings stay as they are: Tune adds none.
    let loop_definition = &work.loop_definition;
    let mut study = storage::open_study(
        &work.project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    study.set_section(&outcome.section)?;
    storage::save_study(&work.project_root, &study, &[])?;
    to_value(&outcome.result)
}

#[cfg(test)]
#[path = "tune_action_tests.rs"]
mod tests;
