//! The sequencer of Control Studio's automatic progression (specification 35, B8 and §0.1a `controlStudio/run`): it runs the stages of a
//! run in their order, each through the stage function of its own method, and stops at the first failure or at the input an engineer
//! must give. It decides nothing of its own: which stages run is trust-control's `run_plan`, given the evidence the study holds
//! (`planned_evidence` only adds a fresh identification the request asks for), the input a stage asks for, with the sentence that
//! asks for it, is its failure's (`NeededInput::of_failure`), and what each stage does is that stage's rule.
//!
//! The stages keep their own results as they finish (identify and tune write the study, validate writes the study too), so a stop
//! keeps every result the run obtained, and a Cancel stops the running stage at its next step.

#![allow(clippy::module_name_repetitions)] // `RunWork` names the module's subject.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::identification::acquisition_settings::{
    declared_settings, AcquisitionMethod, AcquisitionSettings,
};
use trust_control::identification::connected_run::runtime_required;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{RunResult, RunThrough, TuneRequest};
use trust_control::runtime::RuntimeConnection;
use trust_control::study::inputs::{stage_evidence_of, InputsSection};
use trust_control::study::run_plan::{planned_evidence, run_plan, NeededInput};
use trust_control::study::Page;
use trust_control::tuning::settings_pins::PinnedSettings;

use crate::application_state::state_now;
use crate::commands::to_value;
use crate::operation::{cancelled, OperationContext};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::{identify_action, storage, tune_action, validate_action};

/// What a run works with: owned, so that it moves to the operation's thread.
pub struct RunWork {
    /// The project folder, where the loop's study is.
    pub project_root: PathBuf,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: PathBuf,
    /// The runtime recordings the run stops are released through this engine's one list (E4.7).
    pub pending: Arc<PendingReleases>,
    /// The loop the run works on.
    pub loop_definition: LoopDefinition,
    /// Where the runtime listens, when the run identifies or validates (E11.1).
    pub connection: Option<RuntimeConnection>,
    /// The stage the run goes through to (B8).
    pub through: RunThrough,
    /// Whether Identify runs its campaign even when the study holds a current model (B8: Identify plant).
    pub identify_again: bool,
}

/// Runs the stages of `work` (B8) and answers the run's result: the stages after it, the evidence the page gates on, the identification
/// the run made, and the input the run waits for when a stage needs one, with the engine's sentence that asks for it. Any other failure
/// of a stage stops the run as the operation's failure, and the results the stages kept stay in the study.
///
/// # Errors
/// A stage fails for a reason that is not an input the engineer must give, or the run is cancelled, or the study cannot be read.
pub fn sequence(context: &OperationContext, work: &RunWork) -> Result<Value, Failure> {
    let mut identification = None;
    let asked = match run_stages(context, work, &mut identification) {
        Ok(asked) => asked,
        Err(failure) => match NeededInput::of_failure(&failure) {
            Some(asked) => Some(asked),
            None => return Err(failure),
        },
    };
    // The stages after the run as the study and its journal hold them (B9.4, B7): the Apply stage can start exactly when Apply is
    // offered, so a run whose validation passed offers Apply as the open answer would.
    let (application, stage_evidence) = state_now(
        &work.project_root,
        &work.storage_folder,
        &work.loop_definition,
    )?;
    to_value(&RunResult {
        stages: application.stages(&stage_evidence),
        stage_evidence,
        identification,
        needs: asked.as_ref().map(|asked| asked.needs),
        needs_message: asked.as_ref().map(|asked| asked.message.clone()),
        needs_field: asked.and_then(|asked| asked.field),
    })
}

/// The stages a run executes, in order, and the input it waits for when one of them needs it (`None` when the run went through).
/// The manual method's settings are asked for before any stage runs, when Tune is due (B8), so the engineer is not asked after a long
/// identification.
fn run_stages(
    context: &OperationContext,
    work: &RunWork,
    identification: &mut Option<Value>,
) -> Result<Option<NeededInput>, Failure> {
    let loop_definition = &work.loop_definition;
    let study = storage::open_study(
        &work.project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    let inputs = study.section::<InputsSection>()?.unwrap_or_default();
    let evidence = planned_evidence(
        stage_evidence_of(loop_definition, &study)?,
        work.identify_again,
    );
    let plan = run_plan(&evidence, work.through);
    if plan.tune && inputs.needs_manual_settings() {
        return Ok(Some(NeededInput::manual_settings()));
    }
    if plan.identify {
        context.enter_stage(Page::Identify);
        // The stage requires the runtime itself, after the checks that need none: a method the loop cannot run and a missing limit are
        // named before the missing connection, so the page can ask for the limits on a stopped runtime (B3.4 step 1, E11.1).
        *identification = Some(identify_action::run_stage(
            context,
            &work.project_root,
            &work.storage_folder,
            &work.pending,
            loop_definition,
            work.connection.as_ref(),
            &acquisition_of(loop_definition, &inputs),
        )?);
    }
    stop_if_cancelled(context)?;
    if plan.tune {
        context.enter_stage(Page::Tune);
        tune_action::run_stage(
            context,
            &work.project_root,
            loop_definition,
            tune_request(work, &inputs),
            inputs.pins.clone(),
        )?;
    }
    stop_if_cancelled(context)?;
    if plan.validate {
        context.enter_stage(Page::Validate);
        validate_action::run_stage(
            context,
            &work.project_root,
            &work.storage_folder,
            &work.pending,
            loop_definition,
            connection_of(work)?,
        )?;
    }
    Ok(None)
}

/// The runtime connection a stage that reaches the runtime needs (E11.1).
fn connection_of(work: &RunWork) -> Result<&RuntimeConnection, Failure> {
    work.connection.as_ref().ok_or_else(runtime_required)
}

/// The acquisition settings Identify runs with: the study's recorded ones, or the loop's declared experiment with Automatic (B3.3)
/// when none are recorded. A loop that declares no duration then asks for its limits, as Identify does.
fn acquisition_of(loop_definition: &LoopDefinition, inputs: &InputsSection) -> AcquisitionSettings {
    inputs
        .acquisition
        .clone()
        .unwrap_or_else(|| declared_settings(loop_definition, AcquisitionMethod::Automatic))
}

/// The Tune request a run makes from the study's recorded inputs (B4.2): the method, the response, the structure, the pins and the
/// manual settings the engineer recorded for the manual method. Each is what `controlStudio/tune` would receive for the same choices.
/// The run's connection goes with it, so that Tune reads the running controller's settings for the comparison (B4.5).
fn tune_request(work: &RunWork, inputs: &InputsSection) -> TuneRequest {
    TuneRequest {
        project_root: work.project_root.display().to_string(),
        runtime_id: work.loop_definition.runtime_id.clone(),
        loop_id: work.loop_definition.id.clone(),
        storage_folder: work.storage_folder.display().to_string(),
        method: inputs.method,
        choice: inputs.declared_response(),
        structure: inputs.structure,
        pins: inputs.pins.as_ref().map(PinnedSettings::from),
        settings: inputs.manual_settings().cloned(),
        connection: work.connection.clone(),
    }
}

/// Refuses to start the next stage once the run is cancelled (Cancel stops the run at its next step, B8).
fn stop_if_cancelled(context: &OperationContext) -> Result<(), Failure> {
    if context.is_cancelled() {
        return Err(cancelled());
    }
    Ok(())
}
