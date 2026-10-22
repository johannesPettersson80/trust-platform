//! The Tune method `controlStudio/tune` (specification 35, §0.1a, §0.1b, A8.4 to A8.6 and B4.1 to B4.7; Tune page specification).
//!
//! `tune` designs a candidate for the loop's controller from the study's current model and keeps it, unapplied, in the study's
//! Tune section (A8.1). It answers at once with its operation's identifier. The operation's final update carries trust-control's
//! `TuneResult`: the candidate, its predicted response, its margins and the stages.
//!
//! Tune never writes to the controller (B4.1). When the request names a runtime connection, Tune reads the running controller's
//! settings once, before the design, for the comparison of the predicted responses (B4.5); a read that fails leaves the comparison
//! out, and Tune does not fail for it. Tune keeps no journal, except the isolated runs of a finite search (`workflow::tune_search`,
//! which a loop with tuning axes starts instead of the design): otherwise the study is all it writes, and it owns no runtime
//! operation that a restart would have to reattach (A8.5). The request's `storageFolder`, the folder of the operation journals, is read for
//! the block of an Apply's checkpoint that waits for its Restore, which holds Tune (B6.4, B6.7), and for a gain schedule, to check its
//! other points' journals before their sections are stored (C10).
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
//! `run_stage` is the same Tune for a run (`controlStudio/run`, B8): it makes the same checks and the same design inside the
//! run's own operation, and claims nothing, because the run holds the loop.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::{ControllerProject, LoopDefinition};
use trust_control::messages::{TuneChoice, TuneMethod, TuneRequest, TuneWords};
use trust_control::runtime::RuntimeConnection;
use trust_control::study::current_model::{current_model, CurrentModel};
use trust_control::study::identify::IdentifySection;
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::design::gain_schedule::schedule_of;
use trust_control::study::tune::state::{self, StateDesignInput};
use trust_control::study::tune::structure_choice::structure_of_request;
use trust_control::study::tune::{
    check_before_start, holds_relay_record, relay_evidence_for, resolve_pins, tune_held,
    TuneInputs, TuneSection,
};
use trust_control::tuning::relay_rule::RelayCycleEvidence;
use trust_control::tuning::requirements::TuningRequirements;
use trust_control::tuning::settings_pins::ControllerSettingsPins;
use trust_control::tuning::texts;

use crate::application_state::refuse_while_restore_waits_in;
use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::operation::{cancelled, OperationContext, Operations};
use crate::runtime_client::RuntimeClient;
use crate::workflow::tune_schedule;
use crate::workflow::tune_search;
use crate::{loops, storage};

/// The refusal of the run's Tune stage on a finite search (C5): the search is its own operation, which `controlStudio/tune` starts.
const SEARCH_NOT_IN_RUN: &str =
    "A finite search runs as its own Tune operation, which controlStudio/tune starts; the run does not start it.";

/// The command of the operation that `tune` starts (§0.1a).
const TUNE_COMMAND: &str = "tune";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const OPERATION_NAME: &str = "Tune";

/// What the Tune operation works with: owned, so that it moves to the operation's thread.
struct TuneWork {
    /// The project folder, where the loop's study is.
    project_root: PathBuf,
    loop_definition: LoopDefinition,
    /// The study's current model (B3.7), or `None` when the study has none.
    model: Option<CurrentModel>,
    /// The study's Tune section from before, whose supplied requirements the design keeps (B4.3).
    previous: Option<TuneSection>,
    /// The requirements the engineer recorded in the study's inputs, which the design takes as given (B4.3), when there are any.
    held: Option<TuningRequirements>,
    /// The state-design choices the study keeps (D-B11), read for a state law's Tune only.
    state_design: Option<StateDesignInput>,
    /// The pinned settings of a switching controller (B4.2, C3), read for its Tune only.
    on_off_pins: Option<BTreeMap<String, f64>>,
    /// The relay cycle of the study's estimation recording, read for the relay rule only (N5.R).
    relay: Option<RelayCycleEvidence>,
    /// The pins the design takes, with their family (B4.2), when the request or the run names pins.
    pins: Option<ControllerSettingsPins>,
    request: TuneRequest,
}

impl TuneWork {
    /// What trust-control checks and designs from: the loop, the study's sections and the request.
    fn inputs(&self) -> TuneInputs<'_> {
        TuneInputs {
            loop_definition: &self.loop_definition,
            model: self.model.as_ref(),
            previous: self.previous.as_ref(),
            relay: self.relay.as_ref(),
            request: &self.request,
            pins: self.pins.as_ref(),
        }
    }
}

/// Where the pins of a Tune come from (B4.2). `controlStudio/tune` names them without a family, and the study gives it; a run passes
/// the pins its study recorded, which keep the family they were set for (B8).
enum PinSource {
    /// The request's pins, with the family that `resolve_pins` gives them: the structure the request names, else the family of the
    /// study's candidate, else the family of the study's recorded pins.
    Request,
    /// The pins a run read from its recorded inputs, as they are.
    Recorded(Option<ControllerSettingsPins>),
}

/// Answers `controlStudio/tune` (B4): designs the candidate for the loop's controller from the study's current model, keeps it
/// unapplied in the study's Tune section and answers at once with the operation's identifier; the result follows as the
/// operation's final update. The loop is first claimed in `claims` (one operation per loop at a time, A8.5); the operation
/// holds the claim until it ends, and a refused request lets it go at once.
///
/// # Errors
/// The loop cannot be opened, an operation of this engine works on it, or its study, one of its sections, its journal or the
/// estimation recording of the relay rule cannot be read; an Apply's checkpoint waits for its Restore (B6.4, B6.7); or the study
/// holds no current model that the request's method can tune (B4.7), which trust-control's `check_before_start` decides, with the
/// next action it names.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: TuneRequest,
) -> Result<Value, Failure> {
    // Tune keeps no journal (B4.1, A8.5); the request's `storage_folder` is read for the block of a waiting checkpoint. The request
    // stays whole, because the rules read the rest of it (method, choice, structure and pins) through `TuneInputs`.
    let project_root = PathBuf::from(&request.project_root);
    let loop_definition = loops::open(&project_root, &request.runtime_id, &request.loop_id)?;
    if loop_definition
        .controller
        .project
        .as_ref()
        .is_some_and(ControllerProject::is_search_law)
    {
        return tune_search::answer(operations, claims, &request, loop_definition);
    }
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    refuse_while_restore_waits_in(Path::new(&request.storage_folder), &loop_definition)?;
    let work = study_work(&project_root, &loop_definition, request, PinSource::Request)?;
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
/// The study, its sections or the loop's journal cannot be read; an Apply's checkpoint waits for its Restore (B6.4, B6.7); the checks
/// before the start refuse the design (B4.7); or the design fails.
pub(crate) fn run_stage(
    context: &OperationContext,
    project_root: &Path,
    loop_definition: &LoopDefinition,
    request: TuneRequest,
    pins: Option<ControllerSettingsPins>,
) -> Result<Value, Failure> {
    if loop_definition
        .controller
        .project
        .as_ref()
        .is_some_and(ControllerProject::is_search_law)
    {
        return Err(Failure::new(
            SEARCH_NOT_IN_RUN,
            Cause::Settings,
            NextAction::None,
        ));
    }
    refuse_while_restore_waits_in(Path::new(&request.storage_folder), loop_definition)?;
    let work = study_work(
        project_root,
        loop_definition,
        request,
        PinSource::Recorded(pins),
    )?;
    check_before_start(&work.inputs())?;
    run_tune(context, &work)
}

/// The study's sections and the requirements it holds, read for one Tune: the loop's Identify and Tune sections, the recorded
/// requirements of the study's inputs, the relay cycle the request needs, and the pins the design takes (`pins`).
///
/// # Errors
/// The study, one of its sections or the relay rule's estimation recording cannot be read; or the request's pins name no family
/// (`resolve_pins`).
fn study_work(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    request: TuneRequest,
    pins: PinSource,
) -> Result<TuneWork, Failure> {
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let model = current_model(&study)?;
    // Only a fit has a relay record (N5.R): a supplied model names no estimation recording.
    let identify = model.as_ref().and_then(|current| current.identify.as_ref());
    let relay = relay_evidence(project_root, loop_definition, identify, &request)?;
    let inputs = study.section::<InputsSection>()?;
    let previous = study.section::<TuneSection>()?;
    let request = with_study_structure(request, inputs.as_ref(), previous.as_ref());
    let declared = inputs.as_ref().and_then(InputsSection::declared_response);
    let state_design = inputs
        .as_ref()
        .and_then(|inputs| inputs.state_design.clone());
    let on_off_pins = inputs
        .as_ref()
        .and_then(|inputs| inputs.on_off_pins.clone());
    // A state law's pins are its design's (C4): the study's state design holds them, so the request's pins are not resolved for one.
    let state_law = loop_definition.controller.state_feedback.is_some();
    let pins = match pins {
        PinSource::Request if state_law => None,
        PinSource::Request => request
            .pins
            .as_ref()
            .map(|named| {
                resolve_pins(
                    named,
                    request.structure,
                    previous.as_ref().and_then(TuneSection::candidate_structure),
                    inputs.as_ref().and_then(|inputs| inputs.pins.as_ref()),
                )
            })
            .transpose()?,
        PinSource::Recorded(pins) => pins,
    };
    Ok(TuneWork {
        previous,
        held: inputs.and_then(|inputs| inputs.requirements),
        state_design,
        on_off_pins,
        model,
        relay,
        pins,
        project_root: project_root.to_path_buf(),
        loop_definition: loop_definition.clone(),
        request: with_declared_response(request, declared),
    })
}

/// The request with the structure the study gives when the request names none (B4.2, B4.4): the structure the study's inputs record,
/// and, for the manual method with none recorded, the family the Controller selector shows (`structure_of_request`). The page names a
/// structure only when the engineer chose one in the selector; a structure the request names is kept.
pub(crate) fn with_study_structure(
    request: TuneRequest,
    inputs: Option<&InputsSection>,
    previous: Option<&TuneSection>,
) -> TuneRequest {
    TuneRequest {
        structure: structure_of_request(
            request.structure,
            request.method,
            inputs.and_then(|inputs| inputs.structure),
            previous.and_then(TuneSection::candidate_structure),
            inputs.and_then(|inputs| inputs.pins.as_ref()),
        ),
        ..request
    }
}

/// The request with the response the study declares when the request names none (B4.3): Tune reads the response from the study's
/// requirements, as the automatic run does, so the page sends none of its own. A response the request names is kept.
fn with_declared_response(request: TuneRequest, declared: Option<TuneChoice>) -> TuneRequest {
    TuneRequest {
        choice: request.choice.or(declared),
        ..request
    }
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
    study_relay_evidence(project_root, loop_definition, identify)
}

/// The relay cycle of the study's estimation recording for the study's Identify section (N5.R, N2.9): `None` when the study has no
/// Identify result or its recording is not a Feedback relay. The open answer offers the relay rule from it, as Tune reads it.
///
/// # Errors
/// The estimation recording that the Identify result names cannot be read.
pub(crate) fn study_relay_evidence(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    identify: Option<&IdentifySection>,
) -> Result<Option<RelayCycleEvidence>, Failure> {
    // The section alone says whether the study keeps a relay record, so a study without one reads no recording.
    let Some(identify) = identify.filter(|identify| holds_relay_record(identify)) else {
        return Ok(None);
    };
    let recording = storage::read_recording(
        project_root,
        &loop_definition.id,
        &identify.estimation_recording_id,
    )?;
    Ok(relay_evidence_for(identify, loop_definition, &recording))
}

/// The settings the running controller holds now (B4.5, B5.3): the values of the loop's bound parameters, read through the runtime
/// connection as Validate reads them. `None` without a connection, and when the read fails: the comparison is then left out, and
/// Tune does not fail for it.
pub(crate) fn running_settings(
    connection: Option<&RuntimeConnection>,
    loop_definition: &LoopDefinition,
) -> Option<BTreeMap<String, f64>> {
    let mut client = RuntimeClient::connect(connection?).ok()?;
    client.parameter_values(loop_definition).ok()
}

/// The design (B4.4 to B4.6) on the study's model, kept unapplied in the study's Tune section (A8.1, N8.9); its result is the
/// operation's. The running controller's settings are read first, through the request's connection (B4.5). A cancelled operation
/// keeps nothing.
fn run_tune(context: &OperationContext, work: &TuneWork) -> Result<Value, Failure> {
    context.progress(texts::DESIGNING);
    let running = running_settings(work.request.connection.as_ref(), &work.loop_definition);
    // A gain schedule's table is designed from all its operating points (C10), which `tune_schedule` gathers; the other points' sections are
    // stored with this one, after the design.
    let mut siblings = Vec::new();
    let mut outcome = if let Some(schedule) = schedule_of(&work.loop_definition) {
        let designed = tune_schedule::run(
            &work.project_root,
            schedule,
            &work.inputs(),
            work.held.as_ref(),
            running.as_ref(),
        )?;
        siblings = designed.siblings;
        designed.own
    } else if work.request.method == Some(TuneMethod::StateSpace) {
        state::run(
            &work.inputs(),
            work.held.as_ref(),
            work.state_design.as_ref(),
        )?
    } else if work.loop_definition.controller.on_off.is_some() {
        trust_control::study::tune::on_off::run(
            &work.inputs(),
            work.held.as_ref(),
            work.on_off_pins.as_ref(),
        )?
    } else {
        tune_held(&work.inputs(), work.held.as_ref(), running.as_ref())?
    };
    // Every design answers with the engine's words (B4.4 to B4.6): the margins, the outcome sentence, the setting rows and the line of a
    // finite horizon. The state law and the switching controller are designed above without `tune_held`, which gives the single loop
    // its words, so the words are given here, once, to whichever design answered. The page writes none of them.
    TuneWords::attach(&work.loop_definition, &mut outcome.result);
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
    // The other points' sections are stored first, then this one (C10 Tune). A write that fails after another one names the points that keep
    // the table.
    let storage_folder = PathBuf::from(&work.request.storage_folder);
    tune_schedule::store_siblings(&work.project_root, &storage_folder, &siblings)?;
    let stored = study
        .set_section(&outcome.section)
        .and_then(|()| storage::save_study(&work.project_root, &study, &[]));
    stored.map_err(|failure| tune_schedule::own_write_failure(&siblings, failure))?;
    to_value(&outcome.result)
}

#[cfg(test)]
#[path = "tune_action_tests.rs"]
mod tests;
