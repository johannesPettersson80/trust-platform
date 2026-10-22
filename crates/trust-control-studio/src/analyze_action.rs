//! The analysis method `controlStudio/analyze` (specification 35, §0.1a, B9.9, B10.2 and A7).
//!
//! `analyze` answers the analysis tools of the Tune page. Without a `tool` it answers the tools that apply to the model and the loop.
//! A quick tool (`poleZero`, `leadLag`, `stateStructure`, `relativeGain`) is calculated at once and answered with its envelope. An
//! operation (`rootLocus`, `routh`, `frequency`, `feedback`, `measuredFrequency`, `measuredPaths`, `initialState`) claims its loop, runs under the budget of
//! A7, and its final update carries the envelope. A qualifying result of a loop the engine serves is kept in its study (B10.2).
//!
//! Every rule and every sentence is trust-control's (`analysis::*`); this module reads the loop, its study, its declared analysis and its
//! recording, decides which model the request analyses, and forwards the request. A method failure (an input outside its rule, a tool that
//! does not apply, a missing model) is the answer's failure; a refusal of the calculation is the envelope's failure.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;
use trust_control::analysis::budget::{Budget, ANALYSIS_BUDGET, BUDGET_EXCEEDED};
use trust_control::analysis::currency::{reads_study_model, stale_answers};
use trust_control::analysis::declared::{DeclaredControlAnalysis, SavedDeclaredAnalysis};
use trust_control::analysis::declared_tools;
use trust_control::analysis::feedback_tool;
use trust_control::analysis::frequency;
use trust_control::analysis::general_feedback::{FeedbackDefinition, SavedGeneralFeedback};
use trust_control::analysis::initial_state;
use trust_control::analysis::lead_lag_tool;
use trust_control::analysis::measured_frequency;
use trust_control::analysis::measured_paths;
use trust_control::analysis::model_views;
use trust_control::analysis::pole_view;
use trust_control::analysis::routh_session::SavedRouthAnalysis;
use trust_control::analysis::tool::{self, Facts};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    AnalysisEnvelope, AnalysisInputs, AnalysisPage, AnalysisResult, AnalysisTool, AnalyzeAnswer,
    AnalyzeRequest, ModelRevision,
};
use trust_control::model::{decode_general_model, GeneralModel, ResponseConnection};
use trust_control::recording::RecordedRun;
use trust_control::study::analysis_records::{self, KeptAnalysis};
use trust_control::study::current_model::current_model;
use trust_control::study::Study;

use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::operation::{cancelled, OperationContext, Operations};
use crate::{loops, storage};

/// The command of the operation that `analyze` starts, as the operation updates name it.
const ANALYZE_COMMAND: &str = "analyze";
/// The most bytes of JSON of a model in a request (§0.1a; N4.1).
const MAXIMUM_MODEL_BYTES: usize = 256_000;

/// Answers `controlStudio/analyze` (§0.1a): the tools that apply, a quick calculation's envelope, or the started operation. The loop is
/// opened as `controlStudio/open` opens it. The model is the request's, else the study's; a loop the engine serves reads its study, and
/// a loop it does not serve (its structure is parked) keeps nothing and takes its model from the request. A loop whose file uses a
/// parked structure, so that the engine cannot open it at all, answers the tools that apply to the request's model and controller, and
/// keeps nothing (`resolve_without_loop`, C12).
///
/// # Errors
/// The loop cannot be opened, and the request gives no model for a parked loop; the request's model or controller is not a valid
/// general model, or is larger than 256,000 bytes; the declared analysis of the loop file cannot be read; the tool does not apply, or no
/// model is at hand; an input is outside its rule; or an operation of this engine works on the loop (A8.5). Each failure names the input
/// or the cause, with its next action.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: &AnalyzeRequest,
) -> Result<Value, Failure> {
    let project_root = PathBuf::from(&request.project_root);
    let (loop_definition, resolved, claim_key) =
        match loops::open(&project_root, &request.runtime_id, &request.loop_id) {
            Ok(loop_definition) => {
                let resolved = resolve(&project_root, &loop_definition, request)?;
                let claim_key = (
                    loop_definition.project_id.clone(),
                    loop_definition.id.clone(),
                );
                (Some(loop_definition), resolved, claim_key)
            }
            // Only the parked refusal falls back, and only for a request that carries a model: the loop file names the identity that
            // an operation claims (`loops::parked_identity`). Any other failure of opening the loop is answered as it is, except that
            // the list of tools for a loop the engine refuses offers none: the refusal is the open answer's (B9.9).
            Err(failure) => match loops::parked_identity(&project_root, &request.loop_id) {
                Some(claim_key) if request.model.is_some() => {
                    (None, resolve_without_loop(request)?, claim_key)
                }
                _ if request.tool.is_none() && failure.cause == Cause::ControllerCapability => {
                    return to_value(&AnalyzeAnswer::Tools {
                        tools: Vec::new(),
                        stale: Vec::new(),
                    });
                }
                _ => return Err(failure),
            },
        };
    let Some(tool) = request.tool else {
        // The answers the page holds that no longer belong to the model analysed now are listed, so the page drops them (B9.9).
        let model = resolved
            .model
            .as_ref()
            .map(|model| model.identity().revision.as_str());
        return to_value(&AnalyzeAnswer::Tools {
            tools: tool::offers(&resolved.facts()),
            stale: stale_answers(request.held.as_deref().unwrap_or_default(), model),
        });
    };
    if !tool::applies(tool, &resolved.facts()) {
        return Err(refused(tool, &resolved.facts()));
    }
    let inputs = tool::read_inputs(tool, &request.inputs, &resolved.facts());
    let defaults = tool::defaults_for(tool, &resolved.facts());
    let controller_given = resolved.controller.is_some() || resolved.declared.is_some();
    check_inputs(tool, &inputs, resolved.model.as_ref(), controller_given)?;
    let definition = definition_for(tool, &inputs, &resolved)?;
    let model_revision = resolved.model.as_ref().map(revision_of);
    // The calculation is the study's own, whose analysis the engine records as the one the page shows (B9.9), when the request names
    // no model and no controller of its own and the loop has a study.
    let records_shown = resolved.served && request.model.is_none() && request.controller.is_none();
    let controller_revision = if uses_controller(tool, &inputs) {
        resolved.controller.as_ref().map(revision_of)
    } else {
        None
    };
    if tool::presentation(tool).operation {
        let work = Work {
            project_root,
            loop_definition,
            served: resolved.served,
            tool,
            inputs,
            defaults,
            records_shown,
            model: resolved.model,
            model_revision,
            controller_revision,
            definition,
            declared: resolved.declared,
        };
        return start_operation(operations, claims, claim_key, work);
    }
    let outcome = match resolved.model.as_ref() {
        Some(model) => quick(tool, model, &inputs),
        None => Err(tool::no_model()),
    };
    if records_shown {
        record_shown(
            &project_root,
            loop_definition.as_ref(),
            tool,
            &inputs,
            model_revision.as_ref(),
        )?;
    }
    to_value(&AnalyzeAnswer::Calculation(Box::new(envelope(
        tool,
        inputs,
        defaults,
        model_revision,
        controller_revision,
        outcome,
    ))))
}

/// Starts the operation of a tool that runs under the budget of A7, holding the loop's claim until it ends (A8.5), and answers its identifier
/// at once.
///
/// # Errors
/// An operation of this engine works on the loop already.
fn start_operation(
    operations: &Operations,
    claims: &LoopClaims,
    (project_id, loop_id): (String, String),
    work: Work,
) -> Result<Value, Failure> {
    let claim = claims.claim(&project_id, &loop_id, tool::presentation(work.tool).label)?;
    let started = operations.start(ANALYZE_COMMAND, move |context| {
        let worked = run(context, &work);
        drop(claim);
        worked
    });
    to_value(&AnalyzeAnswer::Started {
        operation_id: started.operation_id,
    })
}

/// The model and the controller of a request for a loop that cannot be opened: the request's own, with no loop, no declared analysis
/// and no study (B10.2). Such a loop's tools are the ones that apply to these facts, operations included (C12, B9.9).
fn resolve_without_loop(request: &AnalyzeRequest) -> Result<Resolved, Failure> {
    Ok(Resolved {
        served: false,
        model: request
            .model
            .as_ref()
            .map(|value| model_of(value, "model"))
            .transpose()?,
        controller: request
            .controller
            .as_ref()
            .map(|value| model_of(value, "controller"))
            .transpose()?,
        declared: None,
        recordings: Vec::new(),
        multichannel: false,
    })
}

/// What the engine reads for one request: the model, the controller and the declared analysis, and the recordings the loop's study lists.
struct Resolved {
    /// Whether the engine serves the loop. Only a served loop has a study that keeps results (B10.2).
    served: bool,
    /// The model analysed: the request's, else the study's.
    model: Option<GeneralModel>,
    /// The controller the request carries.
    controller: Option<GeneralModel>,
    /// The declared analysis of the loop file.
    declared: Option<DeclaredControlAnalysis>,
    /// The recordings the study lists, by identifier; empty for a loop the engine does not serve.
    recordings: Vec<String>,
    /// Whether the loop declares as many bound inputs as outputs, two or more (C7): what `measuredPaths` needs.
    multichannel: bool,
}

impl Resolved {
    fn facts(&self) -> Facts<'_> {
        Facts {
            model: self.model.as_ref(),
            controller: self.controller.as_ref(),
            declared: self.declared.as_ref(),
            recordings: &self.recordings,
            multichannel: self.multichannel,
        }
    }
}

/// The model, the controller and the declared analysis of the request, read as §0.1a and A8 say.
fn resolve(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    request: &AnalyzeRequest,
) -> Result<Resolved, Failure> {
    let served = loop_definition.rust_engine_support().is_ok();
    let study = if served {
        Some(storage::open_study(
            project_root,
            &loop_definition.id,
            &loop_definition.revision,
        )?)
    } else {
        None
    };
    let study_model = match &study {
        Some(study) => study_model(study, loop_definition)?,
        None => None,
    };
    let model = match &request.model {
        Some(value) => Some(model_of(value, "model")?),
        None => study_model,
    };
    let controller = match &request.controller {
        Some(value) => Some(model_of(value, "controller")?),
        None => None,
    };
    let declared = match &loop_definition.analysis {
        Some(value) => Some(
            trust_control::json::decode_value::<DeclaredControlAnalysis>(value.clone()).map_err(
                |error| {
                    Failure::new(
                        &format!("The loop's declared analysis cannot be read: {error}."),
                        Cause::Implementation,
                        NextAction::OpenLoopSetup,
                    )
                },
            )?,
        ),
        None => None,
    };
    let recordings = study
        .as_ref()
        .map(|study| study.recordings.clone())
        .unwrap_or_default();
    let multichannel = loop_definition.channels.as_ref().is_some_and(|channels| {
        channels.inputs.len() >= 2 && channels.inputs.len() == channels.outputs.len()
    });
    Ok(Resolved {
        served,
        model,
        controller,
        declared,
        recordings,
        multichannel,
    })
}

/// The study's current model, fitted or supplied, while it is current for the loop (B3.7, A8.4): the one every stage reads.
fn study_model(
    study: &Study,
    loop_definition: &LoopDefinition,
) -> Result<Option<GeneralModel>, Failure> {
    Ok(current_model(study)?
        .filter(|current| current.is_current_for(loop_definition))
        .map(|current| current.model))
}

/// A general model from the request, checked against N4.1 and the size of §0.1a.
fn model_of(value: &Value, what: &str) -> Result<GeneralModel, Failure> {
    let size = serde_json::to_string(value).map_or(usize::MAX, |text| text.len());
    if size > MAXIMUM_MODEL_BYTES {
        return Err(Failure::new(
            &format!("The {what} is larger than 256,000 bytes of JSON."),
            Cause::Settings,
            NextAction::None,
        ));
    }
    decode_general_model(value).ok_or_else(|| {
        Failure::new(
            &format!("The {what} is not a valid general model: check its channels, units and matrices (N4.1)."),
            Cause::ModelOrData,
            NextAction::None,
        )
    })
}

/// The failure of a tool that does not apply: no model at hand names the Identify action, anything else names the method.
fn refused(tool: AnalysisTool, facts: &Facts<'_>) -> Failure {
    if facts.model.is_none() && reads_study_model(tool) {
        tool::no_model()
    } else {
        tool::not_applicable(tool)
    }
}

/// Whether the tool reads the controller model: `feedback`, and `initialState` through the feedback connection.
fn uses_controller(tool: AnalysisTool, inputs: &AnalysisInputs) -> bool {
    match tool {
        AnalysisTool::Feedback => true,
        AnalysisTool::InitialState => inputs.connection == Some(ResponseConnection::Feedback),
        _ => false,
    }
}

/// The input checks of the tools that run at once or as operations: each rule names its input (A7). Their refusals are method failures,
/// answered before anything starts.
fn check_inputs(
    tool: AnalysisTool,
    inputs: &AnalysisInputs,
    model: Option<&GeneralModel>,
    controller_given: bool,
) -> Result<(), Failure> {
    match (tool, model) {
        (AnalysisTool::PoleZero, Some(model)) => pole_view::view_of(model, inputs).map(|_| ()),
        (AnalysisTool::Frequency, Some(model)) => {
            frequency::checked_grid(model, inputs.grid).map(|_| ())
        }
        (AnalysisTool::LeadLag, _) => lead_lag_tool::checked_request(inputs).map(|_| ()),
        (AnalysisTool::InitialState, Some(model)) => {
            initial_state::checked_settings(model, inputs, controller_given).map(|_| ())
        }
        (AnalysisTool::MeasuredPaths, _) => measured_paths::checked_request(inputs),
        _ => Ok(()),
    }
}

/// The feedback definition of `feedback`, and of `initialState` through the feedback connection: the plant, the controller and their
/// channels, checked as trust-control checks them (N4.4). `None` for the other tools.
fn definition_for(
    tool: AnalysisTool,
    inputs: &AnalysisInputs,
    resolved: &Resolved,
) -> Result<Option<FeedbackDefinition>, Failure> {
    if !uses_controller(tool, inputs) {
        return Ok(None);
    }
    let model = resolved.model.as_ref().ok_or_else(tool::no_model)?;
    feedback_tool::feedback_definition(
        model,
        resolved.controller.as_ref(),
        resolved.declared.as_ref(),
        inputs,
    )
    .map(Some)
}

/// A quick tool's calculation, at once.
fn quick(
    tool: AnalysisTool,
    model: &GeneralModel,
    inputs: &AnalysisInputs,
) -> Result<AnalysisResult, Failure> {
    match tool {
        AnalysisTool::PoleZero => {
            pole_view::pole_zero_answer(model, inputs).map(AnalysisResult::PoleZero)
        }
        AnalysisTool::LeadLag => {
            lead_lag_tool::lead_lag_answer(model, inputs).map(AnalysisResult::LeadLag)
        }
        AnalysisTool::StateStructure => {
            model_views::state_structure_answer(model).map(AnalysisResult::StateStructure)
        }
        AnalysisTool::RelativeGain => {
            model_views::relative_gain_answer(model).map(AnalysisResult::RelativeGain)
        }
        other => Err(tool::not_applicable(other)),
    }
}

/// What an operation works with: owned, so that it moves to the operation's thread. The loop is `None` for a parked loop, which the
/// engine cannot open; only the measured response and `keep` read the loop, and neither runs for such a loop.
struct Work {
    project_root: PathBuf,
    loop_definition: Option<LoopDefinition>,
    served: bool,
    tool: AnalysisTool,
    inputs: AnalysisInputs,
    defaults: AnalysisInputs,
    /// Whether the engine records the calculation as the analysis the page shows (B9.9): the study's own, for a loop it serves.
    records_shown: bool,
    model: Option<GeneralModel>,
    model_revision: Option<ModelRevision>,
    controller_revision: Option<ModelRevision>,
    definition: Option<FeedbackDefinition>,
    declared: Option<DeclaredControlAnalysis>,
}

/// Runs the operation's calculation under its budget and reports its envelope as the final result (A7, B10.2). A budget overrun fails the
/// operation with the A7 sentence; a cancelled operation keeps nothing and reports cancelled; a refusal of the calculation is the
/// envelope's failure.
fn run(context: &OperationContext, work: &Work) -> Result<Value, Failure> {
    let started = Instant::now();
    let elapsed = || started.elapsed();
    let cancelled_now = || context.is_cancelled();
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled_now);
    let outcome = match calculate(work, &budget) {
        Ok(result) => {
            if context.is_cancelled() {
                return Err(cancelled());
            }
            keep(work, &result)?;
            Ok(result)
        }
        Err(_) if context.is_cancelled() => return Err(cancelled()),
        Err(failure) if failure.message == BUDGET_EXCEEDED => return Err(failure),
        Err(failure) => Err(failure),
    };
    if work.records_shown {
        record_shown(
            &work.project_root,
            work.loop_definition.as_ref(),
            work.tool,
            &work.inputs,
            work.model_revision.as_ref(),
        )?;
    }
    to_value(&envelope(
        work.tool,
        work.inputs.clone(),
        work.defaults.clone(),
        work.model_revision.clone(),
        work.controller_revision.clone(),
        outcome,
    ))
}

/// The calculation of an operation tool, under the budget.
fn calculate(work: &Work, budget: &Budget<'_>) -> Result<AnalysisResult, Failure> {
    let inputs = &work.inputs;
    match work.tool {
        AnalysisTool::Frequency => {
            frequency::frequency_answer(required_model(work)?, inputs, budget)
                .map(AnalysisResult::Frequency)
        }
        AnalysisTool::InitialState => initial_state::initial_state_answer(
            required_model(work)?,
            work.definition.as_ref(),
            inputs,
            budget,
        )
        .map(AnalysisResult::InitialState),
        AnalysisTool::Feedback => {
            let definition = work.definition.as_ref().ok_or_else(tool::no_model)?;
            feedback_tool::feedback_answer(definition, inputs, budget).map(AnalysisResult::Feedback)
        }
        AnalysisTool::RootLocus => {
            declared_tools::root_locus_answer(required_declared(work)?, inputs, budget)
                .map(AnalysisResult::RootLocus)
        }
        AnalysisTool::Routh => {
            declared_tools::routh_answer(required_declared(work)?, inputs, budget)
                .map(AnalysisResult::Routh)
        }
        AnalysisTool::MeasuredFrequency => {
            let loop_definition = required_loop(work)?;
            let (recording_id, recording) = measured_recording(work, loop_definition, inputs)?;
            measured_frequency::measured_frequency_answer(
                &recording_id,
                &recording,
                loop_definition,
                inputs,
                budget,
            )
            .map(AnalysisResult::MeasuredFrequency)
        }
        AnalysisTool::MeasuredPaths => {
            let loop_definition = required_loop(work)?;
            let (recording_id, recording) = paths_recording(work, loop_definition, inputs)?;
            measured_paths::measured_paths_answer(
                &recording_id,
                &recording,
                loop_definition,
                inputs,
                budget,
            )
            .map(AnalysisResult::MeasuredPaths)
        }
        other => Err(tool::not_applicable(other)),
    }
}

/// The recording a measured response measures (§0.1a, B9.9), with its identifier in the study: the one the request names, which the
/// study must list for the result to be kept, else the study's last kept recording that is a periodic record
/// (`measured_frequency::last_periodic_recording`). The answer names that identifier, never the run's own.
fn measured_recording(
    work: &Work,
    loop_definition: &LoopDefinition,
    inputs: &AnalysisInputs,
) -> Result<(String, RecordedRun), Failure> {
    if let Some(id) = inputs.recording_id.as_deref() {
        let file = storage::read_recording(&work.project_root, &loop_definition.id, id)?;
        return Ok((id.to_string(), file.run));
    }
    let study = storage::open_study(
        &work.project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    measured_frequency::last_periodic_recording(&study.recordings, loop_definition, |id| {
        storage::read_recording(&work.project_root, &loop_definition.id, id).map(|file| file.run)
    })
}

/// The recording that the measured MIMO paths read (§0.1a, C7): the one the request names, else the study's last recording, which the
/// answer names. The paths read every signal of the recording that the loop declares, so no periodic rule applies.
fn paths_recording(
    work: &Work,
    loop_definition: &LoopDefinition,
    inputs: &AnalysisInputs,
) -> Result<(String, RecordedRun), Failure> {
    if let Some(id) = inputs.recording_id.as_deref() {
        let file = storage::read_recording(&work.project_root, &loop_definition.id, id)?;
        return Ok((id.to_string(), file.run));
    }
    let study = storage::open_study(
        &work.project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    measured_paths::last_recording(&study.recordings, |id| {
        storage::read_recording(&work.project_root, &loop_definition.id, id).map(|file| file.run)
    })
}

/// Keeps a qualifying result of a served loop in its study (B10.2, A8.4): `rootLocus`, `routh`, `feedback`, `measuredFrequency` and `measuredPaths`.
/// The other results, and a loop the engine does not serve, keep nothing. trust-control's `keep` refuses a result whose recording the
/// study does not list, and then nothing is saved.
fn keep(work: &Work, result: &AnalysisResult) -> Result<(), Failure> {
    if !work.served {
        return Ok(());
    }
    let loop_definition = required_loop(work)?;
    let Some(kept) = kept_analysis(work, result)? else {
        return Ok(());
    };
    let mut study = storage::open_study(
        &work.project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    analysis_records::keep(&mut study, &loop_definition.revision, kept)?;
    storage::save_study(&work.project_root, &study, &[])
}

/// Records the analysis a calculation answered as the one the Tune page shows (B9.9, A8.1): the tool, the inputs the engine read for it and
/// the revision of the model it was calculated for, so that reopening shows it again and the engine can say when it is no longer current
/// (`stale` of an inputs answer). The page's own request (`controlStudio/page`) keeps that revision for the same tool. A loop the engine
/// does not open (`None`) has no study and records nothing.
///
/// # Errors
/// The study cannot be read or written.
fn record_shown(
    project_root: &Path,
    loop_definition: Option<&LoopDefinition>,
    tool: AnalysisTool,
    inputs: &AnalysisInputs,
    model: Option<&ModelRevision>,
) -> Result<(), Failure> {
    let Some(loop_definition) = loop_definition else {
        return Ok(());
    };
    let mut study =
        storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    study.analysis = Some(AnalysisPage {
        tool,
        inputs: inputs.clone(),
        model_revision: model.map(|model| model.revision.clone()),
    });
    storage::save_study(project_root, &study, &[])
}

/// The record of a kept result, with the definition it was calculated from; `None` for a result that the study does not keep (C17).
fn kept_analysis(work: &Work, result: &AnalysisResult) -> Result<Option<KeptAnalysis>, Failure> {
    Ok(match result {
        AnalysisResult::RootLocus(answer) => {
            Some(KeptAnalysis::RootLocus(Box::new(SavedDeclaredAnalysis {
                definition: required_declared(work)?.clone(),
                result: answer.clone(),
            })))
        }
        AnalysisResult::Routh(answer) => {
            let declared = required_declared(work)?;
            Some(KeptAnalysis::Routh(Box::new(SavedRouthAnalysis {
                selected_value: work
                    .inputs
                    .selected_value
                    .unwrap_or(declared.root_locus.selected_value),
                result: answer.clone(),
                definition: declared.clone(),
            })))
        }
        AnalysisResult::Feedback(answer) => {
            let definition = work.definition.clone().ok_or_else(tool::no_model)?;
            Some(KeptAnalysis::Feedback(Box::new(SavedGeneralFeedback {
                definition,
                result: answer.clone(),
            })))
        }
        AnalysisResult::MeasuredFrequency(answer) => {
            Some(KeptAnalysis::MeasuredFrequency(Box::new(answer.clone())))
        }
        AnalysisResult::MeasuredPaths(answer) => {
            Some(KeptAnalysis::MeasuredPaths(Box::new(answer.clone())))
        }
        _ => None,
    })
}

/// The loop that an operation reads (§0.1a): a loop the engine does not open has none, and the tools that read it do not apply to it.
fn required_loop(work: &Work) -> Result<&LoopDefinition, Failure> {
    work.loop_definition
        .as_ref()
        .ok_or_else(|| tool::not_applicable(work.tool))
}

fn required_model(work: &Work) -> Result<&GeneralModel, Failure> {
    work.model.as_ref().ok_or_else(tool::no_model)
}

fn required_declared(work: &Work) -> Result<&DeclaredControlAnalysis, Failure> {
    work.declared
        .as_ref()
        .ok_or_else(|| tool::not_applicable(work.tool))
}

/// The envelope of one calculation: the tool, the inputs and defaults it read, the revisions of the model and the controller, and either
/// the result or the refusal.
fn envelope(
    tool: AnalysisTool,
    inputs: AnalysisInputs,
    defaults: AnalysisInputs,
    model: Option<ModelRevision>,
    controller: Option<ModelRevision>,
    outcome: Result<AnalysisResult, Failure>,
) -> AnalysisEnvelope {
    let (result, failure) = match outcome {
        Ok(result) => (Some(result), None),
        Err(failure) => (None, Some(failure)),
    };
    AnalysisEnvelope {
        tool,
        inputs,
        defaults,
        model,
        controller,
        result,
        failure,
    }
}

fn revision_of(model: &GeneralModel) -> ModelRevision {
    ModelRevision {
        revision: model.identity().revision.clone(),
    }
}

#[cfg(test)]
#[path = "analyze_action_tests.rs"]
mod tests;
