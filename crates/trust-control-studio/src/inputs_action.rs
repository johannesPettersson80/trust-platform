//! The method `controlStudio/inputs` (specification 35, §0.1a; A8.1, A8.4 and B4.2 to B4.3): it records the inputs the study holds,
//! as the page's editors change them, and answers at once with the stages and with what the change invalidated.
//!
//! It starts no operation and runs no stage, so it reaches no runtime. It refuses the change while an operation of the loop runs, and
//! while an Apply's checkpoint waits for its Restore (B6.4, annex E11.7), with the block's reason and the next action Restore: the
//! page's editors stay usable, and the engine refuses. Every rule is trust-control's: the section's checks, what a change invalidates,
//! the stages the study then has, and the Validate page's offers after it (B7).

use std::path::PathBuf;

use serde_json::Value;
use trust_control::analysis::currency::stale_shown;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::range_refusal;
use trust_control::loop_definition::{
    channel_choices, check_selection, ChannelSelection, LoopDefinition,
};
use trust_control::messages::{AnalysisTool, InputsRequest, InputsResult};
use trust_control::study::current_model::current_model;
use trust_control::study::inputs::{
    check_events_on_grid, check_within_duration, evidence_after_invalidation, held_requirements,
    held_scenarios, invalidation, stage_evidence_of, HeldTest, InputsSection, Invalidation,
};
use trust_control::study::inputs_change::apply_change;
use trust_control::study::tune::requirements::study_requirements;
use trust_control::study::tune::state;
use trust_control::study::tune::TuneSection;
use trust_control::study::validate::ValidateSection;
use trust_control::tuning::automatic_criteria::keep_generated_fields;
use trust_control::tuning::requirements::RequirementRefusal;

use crate::application_state::{refuse_while_restore_waits, ApplicationState};
use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::journal::Journal;
use crate::settings::{self, Environment, Platform};
use crate::{loops, storage};

/// The name of the operation that holds the loop, as a refusal names it (A8.5). The inputs cannot change while it runs.
const OPERATION_NAME: &str = "Inputs";

/// The refusal of a request that names both the whole inputs and one change of them, or neither (A8.1).
const FORM_REFUSAL: &str =
    "The request names either the whole inputs or one change of them, not both and not neither.";

/// Answers `controlStudio/inputs`: records the inputs the request gives in the loop's study, and answers the inputs the study holds,
/// the stages after the change and what it invalidated (A8.4). The request gives either the whole section, which replaces the earlier
/// inputs, or one change of the page, which the engine merges into them (`trust_control::study::inputs_change`). Nothing else is
/// written, and nothing runs.
///
/// # Errors
/// The loop cannot be opened; an operation of this engine works on the loop (the inputs are read-only while it runs); the loop's
/// journal cannot be read, or holds an Apply's checkpoint that waits for its Restore (B6.4); the study or its sections cannot be read;
/// the request names both or neither of its two forms, or the change names an input the study does not take; an input breaks a rule
/// of its own (trust-control's checks, with the next action they name); or the study cannot be written.
pub fn answer(claims: &LoopClaims, request: InputsRequest) -> Result<Value, Failure> {
    let InputsRequest {
        project_root,
        runtime_id,
        loop_id,
        inputs,
        change,
        channel_selection,
        storage_folder,
    } = request;
    let project_root = PathBuf::from(project_root);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    // The claim ends when this answer returns: a change is refused only while an operation of the loop holds it.
    let _claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let journal = journal_without_waiting_checkpoint(storage_folder, &loop_definition)?;
    let study = storage::open_study(
        &project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    let tune = study.section::<TuneSection>()?;
    let before = study.section::<InputsSection>()?;
    // The request gives the whole section, or one change of the page that the engine merges into the recorded inputs (A8.1, A8.4).
    let inputs = match (inputs, change) {
        (Some(inputs), None) => inputs,
        (None, Some(change)) => apply_change(
            &before.clone().unwrap_or_default(),
            &change,
            &loop_definition.revision,
        )?,
        _ => {
            return Err(Failure::new(
                FORM_REFUSAL,
                Cause::Settings,
                NextAction::None,
            ))
        }
    };
    // The tuning method is the `method` input alone: a requirement record keeps no copy of it (A8.1).
    let mut inputs = inputs.without_requirement_method();
    keep_automatic_requirements(&study, &loop_definition, &mut inputs)?;
    let before_scenarios = held_scenarios(&loop_definition, before.as_ref());
    let after_scenarios = held_scenarios(&loop_definition, Some(&inputs));
    let invalidated = invalidation(
        &HeldTest {
            requirements: held_requirements(before.as_ref(), tune.as_ref()),
            scenarios: &before_scenarios,
        },
        &HeldTest {
            requirements: held_requirements(Some(&inputs), tune.as_ref()),
            scenarios: &after_scenarios,
        },
    );
    check_tolerance_span(&loop_definition, &inputs)?;
    check_acquisition_ranges(&loop_definition, &inputs)?;
    check_scenario_events(&loop_definition, before.as_ref(), &inputs)?;
    let mut next = study.clone();
    // A section that breaks one of its rules is refused here, and nothing is written (B10.4).
    next.set_section(&inputs)?;
    // A change of the requirements or of the scenarios must leave every declared check inside the validation duration (B4.3): a check
    // that ends after it is refused here, under the duration field, and nothing is written.
    let recorded = before.clone().unwrap_or_default();
    let test_changed =
        recorded.requirements != inputs.requirements || recorded.scenarios != inputs.scenarios;
    if let Some(requirements) = inputs.requirements.as_ref().filter(|_| test_changed) {
        check_within_duration(requirements, &after_scenarios)?;
    }
    let channels_changed = record_channel_selection(
        &project_root,
        &runtime_id,
        &loop_id,
        &study,
        &mut next,
        channel_selection,
    )?;
    let invalidated = Invalidation::of(invalidated.validation || channels_changed);
    // The study is written only when an input it holds changed (A8.1): an answer that changes nothing writes nothing.
    let changed = before.clone().unwrap_or_default() != inputs || channels_changed;
    if changed {
        storage::save_study(&project_root, &next, &[])?;
    }
    // The stages read the loop with the selection the study now holds (A8.1).
    let after = loops::open(&project_root, &runtime_id, &loop_id)?;
    // A validation the change invalidated names no current result, so the stages and the page's table follow the invalidation (A8.4).
    let stage_evidence =
        evidence_after_invalidation(stage_evidence_of(&after, &next)?, invalidated);
    // The Validate page's offers after the change, by the rule of every answer that states them (B7, A8.4): a validation the change
    // left not current offers no Apply. This answer holds the loop's claim, so no operation of this engine works on it.
    let application = ApplicationState::of(&journal, &next, &stage_evidence, false)?;
    let stages = application.stages(&stage_evidence);
    // The fields follow the family and the solver policy of the inputs this answer records (D-B12, C4).
    let state_fields = state::state_fields(&after, inputs.state_design.as_ref());
    let stale = stale_analyses(&next, &after)?;
    to_value(&InputsResult {
        stages,
        stage_evidence,
        apply_available: application.offers.apply,
        primary_action: application.primary_action(),
        invalidated,
        inputs,
        changed,
        state_fields,
        channel_choices: channel_choices(&after),
        stale,
    })
}

/// The tools whose recorded analysis is no longer current for the study's model (B9.9, A8.4): the analysis the study records is stale when
/// the model it was calculated for is not the model the study holds for `loop_definition` now. The engine names it, and the page drops
/// that answer and asks nothing.
///
/// # Errors
/// A stored section cannot be read (B10.4).
fn stale_analyses(
    study: &trust_control::study::Study,
    loop_definition: &LoopDefinition,
) -> Result<Vec<AnalysisTool>, Failure> {
    let model = current_model(study)?
        .filter(|current| current.is_current_for(loop_definition))
        .map(|current| current.model.identity().revision.clone());
    Ok(stale_shown(study.analysis.as_ref(), model.as_deref()))
}

/// The loop's journal, read from the folder the request names ([`journal_folder`]), while no Apply's checkpoint in it waits for its
/// Restore: while one does, the inputs the stages read wait too (B6.4, annex E11.7), and the change is refused with the block's reason
/// and the next action Restore, before anything is read or written.
///
/// # Errors
/// The journal's folder is not found or the journal cannot be read, or it holds an Apply's checkpoint that waits for its Restore.
fn journal_without_waiting_checkpoint(
    storage_folder: Option<String>,
    loop_definition: &LoopDefinition,
) -> Result<Journal, Failure> {
    let journal = Journal::open(
        &journal_folder(storage_folder, &|name| std::env::var_os(name))?,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_restore_waits(&journal)?;
    Ok(journal)
}

/// Keeps each generated requirement of `inputs` Automatic only while the change leaves its value as the page showed it (B4.3): the
/// engine decides which. The page showed the record the study holds, or, while it holds none, the record that the answers carry for the
/// current model, every value of it Automatic's (`study_requirements`). Compared with nothing, a first change would keep every field
/// Automatic, and Tune would replace the values the engineer typed (checklist F29a).
///
/// # Errors
/// A section of the study or its model cannot be read (B10.4).
fn keep_automatic_requirements(
    study: &trust_control::study::Study,
    loop_definition: &LoopDefinition,
    inputs: &mut InputsSection,
) -> Result<(), Failure> {
    let Some(requirements) = inputs.requirements.as_mut() else {
        return Ok(());
    };
    let before = study.section::<InputsSection>()?;
    let mut shown = before
        .as_ref()
        .and_then(|section| section.requirements.clone());
    if shown.is_none() {
        let tune = study.section::<TuneSection>()?;
        shown = current_model(study)?
            .filter(|current| current.is_current_for(loop_definition))
            .and_then(|current| {
                study_requirements(loop_definition, &current, tune.as_ref(), before.as_ref())
            });
    }
    if let Some(shown) = shown {
        keep_generated_fields(&shown, requirements);
    }
    Ok(())
}

/// The folder of the operation journals that the request names (A8.3), or the engine's journal folder, the one the command line uses,
/// when the request names none or leaves it empty.
///
/// # Errors
/// The request names no folder, and the environment does not give the user's state folder.
pub(crate) fn journal_folder(
    storage_folder: Option<String>,
    environment: Environment<'_>,
) -> Result<PathBuf, Failure> {
    match storage_folder.filter(|folder| !folder.is_empty()) {
        Some(folder) => Ok(PathBuf::from(folder)),
        None => settings::default_journal_folder(Platform::current(), environment),
    }
}

/// Records the page's channel selection, checked against the loop's declared channels, and answers whether it changed. A changed
/// selection binds other plant signals, so the candidate designed and the validations run on the earlier ones go (A8.4); the model
/// follows the signals by itself (`identify_signals`).
fn record_channel_selection(
    project_root: &std::path::Path,
    runtime_id: &str,
    loop_id: &str,
    study: &trust_control::study::Study,
    next: &mut trust_control::study::Study,
    selection: Option<ChannelSelection>,
) -> Result<bool, Failure> {
    let Some(selection) = selection else {
        return Ok(false);
    };
    let declared = loops::open_declared(project_root, runtime_id, loop_id)?;
    check_selection(&declared, &selection)?;
    let changed = study.section::<ChannelSelection>()?.as_ref() != Some(&selection);
    next.set_section(&selection)?;
    if changed {
        next.remove_section::<TuneSection>();
        next.remove_section::<ValidateSection>();
    }
    Ok(changed)
}

/// The Auto-entry tolerance may not exceed the loop's output span (B4.3): the loop holds that span, so the record is checked here.
fn check_tolerance_span(
    loop_definition: &LoopDefinition,
    inputs: &InputsSection,
) -> Result<(), Failure> {
    let Some(requirements) = &inputs.requirements else {
        return Ok(());
    };
    let span = match (
        loop_definition.limits.output_minimum,
        loop_definition.limits.output_maximum,
    ) {
        (Some(minimum), Some(maximum)) => Some(maximum - minimum),
        _ => None,
    };
    requirements
        .check_output_span(span)
        .map_err(RequirementRefusal::into_failure)
}

/// A change of the verification scenarios must write every timed change on a sample of the loop's task (B4.3): the loop holds the task
/// period, so an event between two samples is refused here, under the scenarios' field, and nothing is written. A list that the change
/// leaves as it was is not checked again.
fn check_scenario_events(
    loop_definition: &LoopDefinition,
    before: Option<&InputsSection>,
    inputs: &InputsSection,
) -> Result<(), Failure> {
    let changed =
        before.and_then(|section| section.scenarios.as_deref()) != inputs.scenarios.as_deref();
    let Some(scenarios) = inputs.scenarios.as_deref().filter(|_| changed) else {
        return Ok(());
    };
    check_events_on_grid(scenarios, loop_definition.controller.task_period_ms)
}

/// The refusal of an acquisition range beyond the loop's declared limits (B3.2), under the field of Limits that holds it. The range is
/// checked here, where the loop is known, so that the page shows the sentence under that field; nothing is written when it is refused.
fn check_acquisition_ranges(
    loop_definition: &LoopDefinition,
    inputs: &InputsSection,
) -> Result<(), Failure> {
    inputs
        .acquisition
        .as_ref()
        .and_then(|settings| range_refusal(loop_definition, settings))
        .map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "inputs_action_tests.rs"]
mod tests;
