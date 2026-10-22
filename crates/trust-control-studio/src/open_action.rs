//! The open answer of `controlStudio/open` (specification 35, §0.1a, A8.4 and A8.6): the project's loops, or one loop's study read
//! into the page's first answer. Opening starts no test, tuning, validation or write.

use std::path::Path;

use trust_control::failure::Failure;
use trust_control::identification::experiment_limits::{held_experiment_limits, ExperimentLimits};
use trust_control::identification::identify_result::{study_candidate, study_identification};
use trust_control::identification::method_offers::identify_method_offers;
use trust_control::identification::policy::project_scripted;
use trust_control::identification::project_test::project_test_form;
use trust_control::loop_definition::channel_choices;
use trust_control::messages::{
    controller_offer, default_family, live_sides, requirement_outputs, scenario_row_signal,
    scenario_template, setpoint_choices, HeldInputs, ManualIdentificationAnswer, OpenRequest,
    OpenResult,
};
use trust_control::setup::discovery::study_tune_methods;
use trust_control::study::analysis_records::current_records;
use trust_control::study::current_model::current_model;
use trust_control::study::identify::IdentifySection;
use trust_control::study::inputs::{held_requirements, stage_evidence_of, InputsSection};
use trust_control::study::tune::search_record::study_search_candidate;
use trust_control::study::tune::state;
use trust_control::study::tune::TuneSection;
use trust_control::study::validate::ValidateSection;

use crate::application_state::ApplicationState;
use crate::identify_action;
use crate::journal::{Journal, JournalEntry};
use crate::{loops, storage};

/// Lists the project's loops, or opens the loop's study: the checked loop, its stages and their evidence, the page last
/// viewed (A8.6), the last Validate result and the Identify result while each is current, and the journaled operations it
/// reattaches to. The stages follow
/// the study's evidence: its recordings, its Identify result while that is current for the loop's signals, and its Tune candidate
/// while it was designed on that current model (A8.4). Opening starts no test, tuning, validation or write (A8.6).
///
/// # Errors
/// The loop cannot be opened, or its study, its Identify or Tune section or its journal cannot be read.
pub fn open(request: &OpenRequest) -> Result<OpenResult, Failure> {
    open_with_running(request, Vec::new())
}

/// [`open`] with the operations this engine runs for the loop, which the page follows (A8.6). The command line runs its own
/// operations inside its verb, so it opens with none.
pub(crate) fn open_with_running(
    request: &OpenRequest,
    running: Vec<String>,
) -> Result<OpenResult, Failure> {
    let project_root = Path::new(&request.project_root);
    let Some(loop_id) = &request.loop_id else {
        return Ok(OpenResult::Loops {
            loops: loops::list(project_root, &request.runtime_id)?,
        });
    };
    let loop_definition = loops::open(project_root, &request.runtime_id, loop_id)?;
    // A loop that uses a feature the Rust engine does not run yet stays on the TypeScript engine until its port (V6 decision D9):
    // its study is that engine's, so `open` refuses before it reads, or moves, the study.
    loop_definition.rust_engine_support()?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let journal = Journal::open(
        Path::new(&request.storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let evidence = stage_evidence_of(&loop_definition, &study)?;
    // The Validate page's offers and the block of a checkpoint that waits for its Restore (B6.4, B7, A8.6), read from the journal and
    // the study without the runtime. An unresolved checkpoint of an operation this engine runs for the loop is followed, not blocked.
    let application = ApplicationState::of(&journal, &study, &evidence, !running.is_empty())?;
    let stages = application.stages(&evidence);
    // The last Validate result is answered while the study holds it as current (A8.4, A8.6): a stale one is history.
    let validation = if evidence.validation_has_result {
        study
            .section::<ValidateSection>()?
            .map(|section| section.result(evidence))
    } else {
        None
    };
    let identification = study_identification(&study, &loop_definition, evidence)?;
    let tune = study_candidate(&study, &loop_definition, evidence)?.map(|mut result| {
        trust_control::messages::TuneWords::attach(&loop_definition, &mut result);
        result
    });
    let search_candidate = study_search_candidate(&study, evidence)?;
    // The kept analysis records that are current for the loop and the study's current model, fitted or supplied (B10.2, B3.7, A8.4).
    let selected_model = current_model(&study)?.map(|current| current.model);
    let analysis = current_records(&study, &loop_definition.revision, selected_model.as_ref());
    let inputs = study.section::<InputsSection>()?.unwrap_or_default();
    let state_fields = state::state_fields(&loop_definition, inputs.state_design.as_ref());
    let choices = channel_choices(&loop_definition);
    let project_test = project_scripted(&loop_definition).is_some();
    let identify_methods = identify_method_offers(&loop_definition);
    let (limits, held, manual) = editors_start(project_root, &loop_definition, &inputs);
    let project_form = project_test_form(&loop_definition);
    let measurement_noise_band_required = noise_band_required(&loop_definition);
    let (methods, method_titles) = study_methods(project_root, &loop_definition, &study)?
        .map_or((None, None), |(methods, titles)| (Some(methods), titles));
    // The offers the page renders after open (C2, C4, B3.6, B4.3): the families and the manual rows, the default family, the setpoint choices
    // and the scenario template. The default family follows the study's candidate, then its pins (B4.2).
    let tune_section = study.section::<TuneSection>()?;
    let pins = tune_section
        .as_ref()
        .and_then(|section| section.pins.clone());
    let offer = controller_offer(
        &loop_definition,
        default_family(tune.as_ref(), pins.as_ref()),
    );
    let setpoints = setpoint_choices(&loop_definition);
    let declared_text = state::declared_design_text(&loop_definition);
    let sides = live_sides(&loop_definition);
    // The Requirements editor's outputs and what their rows start from (B4.3, C7): the requirements the study holds, else Tune's.
    let outputs = requirement_outputs(
        &loop_definition,
        held_requirements(Some(&inputs), tune_section.as_ref()),
    );
    let template = scenario_template(&loop_definition, recorded_duration(&inputs));
    let row_signal = scenario_row_signal(&loop_definition);
    Ok(OpenResult::Opened {
        stages,
        reattached: journaled_operations(journal.entries()),
        running,
        validation,
        stage_evidence: evidence,
        identification,
        tune,
        search_candidate,
        methods,
        method_titles,
        loop_definition: Box::new(loop_definition),
        page: study.page,
        page_analysis: study.analysis.clone(),
        page_view: study.view.clone(),
        analysis,
        inputs,
        apply_available: application.offers.apply,
        restore_available: application.offers.restore,
        keep_available: application.offers.keep,
        apply_observation_resumable: application.offers.resume,
        primary_action: application.primary_action(),
        state_fields,
        channel_choices: choices,
        measurement_noise_band_required,
        project_test,
        identify_methods,
        experiment_limits: limits,
        held_inputs: Some(held),
        manual_identification: Some(manual),
        project_test_form: project_form,
        controller_offer: offer,
        setpoint_choices: setpoints,
        scenario_template: template,
        scenario_row_signal: row_signal,
        state_declared_text: declared_text,
        live_sides: sides,
        requirement_outputs: outputs,
    })
}

/// What the Identify page's editors start from, for the open answer (A8.6, B3.2, B3.3): the Limits editor's limits (the loop's declared
/// defaults with the study's own record over them), what the recorded inputs mean for the editors, and the Recorded-data fit the study
/// keeps, replayed from its recordings. The page shows the fit when it shows that method.
fn editors_start(
    project_root: &Path,
    loop_definition: &trust_control::loop_definition::LoopDefinition,
    inputs: &InputsSection,
) -> (ExperimentLimits, HeldInputs, ManualIdentificationAnswer) {
    (
        held_experiment_limits(loop_definition, inputs.acquisition.as_ref()),
        HeldInputs::of(inputs),
        identify_action::identify_manual::kept_state_of(project_root, loop_definition),
    )
}

/// Whether the loop's own law needs the measurement noise band among its requirements (B4.3): a program law whose components
/// name it.
fn noise_band_required(loop_definition: &trust_control::loop_definition::LoopDefinition) -> bool {
    loop_definition
        .controller
        .project
        .as_ref()
        .is_some_and(|project| {
            trust_control::tuning::requirements::noise_band_required(&project.components)
        })
}

/// The validation duration the study's inputs record, or 0 before the engineer gives one (B4.3).
fn recorded_duration(inputs: &InputsSection) -> f64 {
    inputs
        .requirements
        .as_ref()
        .map_or(0.0, |record| record.duration_seconds)
}

/// The Tune page's methods and their titles (C4, N5.R): the loop's own, with the relay rule when the study holds a qualified relay
/// record. An unreadable record offers no relay rule; Tune still refuses it with the reason it gives.
fn study_methods(
    project_root: &Path,
    loop_definition: &trust_control::loop_definition::LoopDefinition,
    study: &trust_control::study::Study,
) -> Result<Option<trust_control::setup::discovery::TuneMethodOffer>, Failure> {
    let identify = study.section::<IdentifySection>()?;
    let relay =
        super::tune_action::study_relay_evidence(project_root, loop_definition, identify.as_ref())
            .ok()
            .flatten();
    Ok(study_tune_methods(loop_definition, relay.as_ref()))
}

/// The journaled operations that reopening reattaches to (A8.5, A8.6): the recordings, experiments, campaigns and isolated jobs that
/// the journal holds unfinished. A checkpoint is not an operation the runtime runs: an Apply's checkpoint is answered as Restore and
/// Keep availability, and a mode checkpoint is returned by the identification's resume (B6.4), so neither is listed here.
fn journaled_operations(entries: &[JournalEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| {
            entry.holds_unfinished_operation()
                && !matches!(
                    entry,
                    JournalEntry::Commissioning { .. } | JournalEntry::IdentificationMode { .. }
                )
        })
        .map(|entry| entry.operation_id().to_string())
        .collect()
}

#[cfg(test)]
#[path = "open_action_tests.rs"]
mod tests;
