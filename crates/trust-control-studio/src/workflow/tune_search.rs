//! The finite search of a program's law (specification 35, C5; A7, A8.5 and E11.2): `controlStudio/tune` with the automatic method on a
//! loop whose `controller.project` declares tuning axes runs the search as an operation. The operation claims the loop, runs each trial as
//! isolated runs of the study's selected model through the runtime's simulation runs, journals each run before it is dispatched (as
//! Validate's runs are, `workflow::validate::run_job`), reports after each trial, and stops at a cancel, at the end of its wall-time
//! budget, or at a change of the program's revision.
//!
//! Every rule is trust-control's: the grid, each trial's settings and request, the bounds, J, the ranking, the budgets and the answer are
//! in `trust_control::tuning::search`, and the section that keeps the candidate is `trust_control::study::tune::search_record`. This module
//! makes the runtime calls in the order those rules need. A search keeps a candidate only when it completed or ran out of budget with a
//! qualifying trial; a cancelled, changed or failed search keeps nothing in the study, and its answer carries its trials.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::unfinished_operation_refusal;
use trust_control::identification::independent_validation::RequirementStatus;
use trust_control::limits::ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS;
use trust_control::loop_definition::{ControllerProject, LoopDefinition};
use trust_control::messages::TuneRequest;
use trust_control::model::{GeneralModel, SimulationOverride};
use trust_control::runtime::RuntimeConnection;
use trust_control::study::current_model::{current_model, CurrentModel};
use trust_control::study::inputs::stage_evidence_of;
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::search_record::search_section;
use trust_control::study::tune::TuneSection;
use trust_control::tuning::requirements::{response_deadline_seconds, TuningRequirements};
use trust_control::tuning::search::{
    answer_of, case_status, check_scan_budget, check_trial_count, grid, has_scenarios,
    names_objective, run_trials, scans_of, score_unit_of, trial_count, trial_evidence,
    trial_run_request, trial_settings, SearchCandidate, SearchStop, SearchTrial, TrialEvidence,
    TrialRun, NO_PLAN, OBJECTIVE_MISSING, SCENARIOS_MISSING, TARGET_CASE,
};
use trust_control::tuning::settings_pins::ControllerSettingsPins;
use trust_control::tuning::texts::PINS_LOOP_REVISION;
use trust_control::validation::engineer_verification_campaign::{
    engineer_verification_campaign, CampaignCase,
};
use trust_control::validation::isolated_run_protocol::{IsolatedRunEvidence, IsolatedRunStatus};
use trust_control::validation::model_route::model_route;

use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::journal::{Journal, JournalEntry};
use crate::operation::{OperationContext, Operations};
use crate::runtime_client::RuntimeClient;
use crate::storage;
use crate::workflow::validate::{new_job_id, run_job, JobTiming, RunIdentity, RunSession};

/// The command of the operation that `tune` starts for a finite search (§0.1a).
const TUNE_COMMAND: &str = "tune";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const OPERATION_NAME: &str = "Tune";
/// The refusal of a search that names a formula or the relay rule: a finite search is the automatic method.
const METHOD_REFUSED: &str =
    "A finite search tunes this law: choose Automatic, which runs the search, rather than a formula or the relay rule.";
/// The refusal of a search without a connected runtime, which runs its trials.
const NO_CONNECTION: &str =
    "The finite search runs its trials on the connected runtime. Connect a runtime, then tune again.";
/// The refusal of a search whose study holds no model for the loop's current signals.
const NO_MODEL: &str = "The study has no model for the loop's current signals. Identify the plant first, so Tune has a model to design on.";
/// The refusal of a search whose study holds no requirements, which the search needs for its bounds.
const NO_REQUIREMENTS: &str = "The finite search needs the requirements: set the setpoint change, the error and settling limits and the validation duration.";
/// The refusal of a loop without a parameter binding, which every trial's settings need.
const NO_LOOP_BINDING: &str = "The loop declares no parameter binding.";
/// The progress text that starts the search's trials.
const PREPARING: &str = "Preparing the finite search on the selected model.";

/// What the search works with: owned, so that it moves to the operation's thread.
struct SearchWork {
    project_root: PathBuf,
    storage_folder: PathBuf,
    loop_definition: LoopDefinition,
    /// The program's law, which declares the axes, the objective and the budgets.
    project: ControllerProject,
    /// The study's current model (B3.7), which every trial runs against.
    model: CurrentModel,
    /// The requirements the search's bounds are checked against (B4.3).
    requirements: TuningRequirements,
    /// The pinned settings the request names, as the search honours them (C5).
    pins: Option<ControllerSettingsPins>,
    /// The pinned values by symbol.
    pin_values: BTreeMap<String, f64>,
    connection: RuntimeConnection,
    /// The setpoint channel of the loop, the reference of J.
    reference: Option<String>,
    /// The process variable of the loop, the feedback of J.
    feedback: String,
    /// The settling deadline of the requirements, in seconds: J is the error after it (C5).
    settling_deadline_seconds: f64,
    /// The unit of J, from the loop's process variable (`score_unit_of`).
    score_unit: String,
}

/// One planned trial: its settings, the selected model's inline plant and its campaign's cases, each judged on its own requirements.
struct TrialPlan {
    settings: BTreeMap<String, f64>,
    simulation: SimulationOverride,
    cases: Vec<CampaignCase>,
}

/// Answers `controlStudio/tune` for a loop whose program's law is a finite search (C5): the operation claims the loop, and its
/// final update carries the search's answer. A refusal before the start lets the claim go at once.
///
/// # Errors
/// The request names a method, has no runtime connection or pins another loop revision; the study holds no current model or no
/// requirements; the journal holds an unfinished operation; the grid breaks a budget; or no trial has a valid plan.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: TuneRequest,
    loop_definition: LoopDefinition,
) -> Result<Value, Failure> {
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let work = search_work(request, loop_definition)?;
    // A refusal returns here, and the claim ends with it.
    let started = operations.start(TUNE_COMMAND, move |context| {
        let worked = run_search(context, &work);
        drop(claim);
        worked
    });
    to_value(&started)
}

/// What the search needs before its operation starts, read from the study and the request (B4.7, C5).
///
/// # Errors
/// The refusals of [`answer`] that come before the start.
fn search_work(
    request: TuneRequest,
    loop_definition: LoopDefinition,
) -> Result<SearchWork, Failure> {
    if request.method.is_some() {
        return Err(Failure::new(
            METHOD_REFUSED,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let project_root = PathBuf::from(&request.project_root);
    let storage_folder = PathBuf::from(&request.storage_folder);
    let connection = request
        .connection
        .clone()
        .ok_or_else(|| Failure::new(NO_CONNECTION, Cause::Runtime, NextAction::ConnectRuntime))?;
    let project = loop_definition
        .controller
        .project
        .clone()
        .filter(ControllerProject::is_search_law)
        .ok_or_else(|| Failure::new(METHOD_REFUSED, Cause::Settings, NextAction::None))?;
    if let Some(pins) = &request.pins {
        if pins.loop_revision != loop_definition.revision {
            return Err(Failure::new(
                PINS_LOOP_REVISION,
                Cause::Settings,
                NextAction::None,
            ));
        }
    }
    let study = storage::open_study(
        &project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    let model = current_model(&study)?
        .filter(|current| current.is_current_for(&loop_definition))
        .ok_or_else(|| Failure::new(NO_MODEL, Cause::ModelOrData, NextAction::Identify))?;
    let held = study
        .section::<InputsSection>()?
        .and_then(|inputs| inputs.requirements);
    let previous = study.section::<TuneSection>()?;
    let requirements = held
        .or_else(|| previous.map(|section| section.requirements))
        .ok_or_else(|| {
            Failure::new(
                NO_REQUIREMENTS,
                Cause::Settings,
                NextAction::OpenRequirements,
            )
        })?;
    requirements.validate().map_err(|refusal| {
        Failure::new(&refusal.0, Cause::Settings, NextAction::OpenRequirements)
    })?;
    let journal = Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    let pin_values = request
        .pins
        .as_ref()
        .map(|pins| pins.values.clone())
        .unwrap_or_default();
    let pins = request.pins;
    let settling_deadline_seconds = response_deadline_seconds(
        requirements.settling_time_seconds,
        loop_definition.controller.task_period_ms,
    );
    Ok(SearchWork {
        project_root,
        storage_folder,
        reference: loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.clone()),
        feedback: loop_definition.signals.process_variable.symbol.clone(),
        score_unit: score_unit_of(&loop_definition.signals.process_variable.unit),
        loop_definition,
        project,
        model,
        requirements,
        pins,
        pin_values,
        connection,
        settling_deadline_seconds,
    })
}

/// Refuses a search while the loop's journal holds an operation that runs or whose outcome is unconfirmed (A8.5, E11.4).
///
/// # Errors
/// The journal holds such an operation.
pub(crate) fn refuse_while_journaled(journal: &Journal) -> Result<(), Failure> {
    let ids: Vec<&str> = journal
        .entries()
        .iter()
        .filter(|entry| entry.holds_unfinished_operation())
        .map(JournalEntry::operation_id)
        .collect();
    unfinished_operation_refusal(&ids).map_or(Ok(()), Err)
}

/// Runs the search (C5): the trials are planned on the runtime's current settings, the budgets are checked before the first trial, and
/// the trials run in their listed order. The answer is the search's; a candidate is kept only when the search completed or ran out of
/// budget with a qualifying trial.
///
/// # Errors
/// The runtime cannot be read; no trial has a valid plan; an objective or a required scenario is missing from a trial; the trials break
/// the total-cycle budget; or the candidate cannot be stored.
fn run_search(context: &OperationContext, work: &SearchWork) -> Result<Value, Failure> {
    context.progress(PREPARING);
    let mut client = RuntimeClient::connect(&work.connection)?;
    let observed = client.parameter_values(&work.loop_definition)?;
    let revision = client.isolated_run_readiness()?;
    let (mut trials, plans) = plan_trials(work, &observed)?;
    let scans = plans.iter().flatten().fold(0_u64, |total, plan| {
        total.saturating_add(scans_of(&plan.cases))
    });
    check_scan_budget(scans, work.project.maximum_cycles)?;
    let deadline = Instant::now() + Duration::from_millis(work.project.wall_budget_ms);
    let total = trials.len();
    let mut journal = Journal::open(
        &work.storage_folder,
        &work.loop_definition.project_id,
        &work.loop_definition.id,
    )?;
    let is_cancelled = || context.is_cancelled();
    let progress = |text: &str| context.progress(text);
    let identity = RunIdentity {
        project_id: work.loop_definition.project_id.clone(),
        runtime_id: work.loop_definition.runtime_id.clone(),
        loop_id: work.loop_definition.id.clone(),
        project_revision: revision.clone(),
    };
    let mut session = RunSession {
        client: &mut client,
        journal: &mut journal,
        identity,
        is_cancelled: &is_cancelled,
        progress: &progress,
        timing: JobTiming::PRODUCTION,
    };
    let stop = run_trials(
        &mut trials,
        |index| stop_before(context, deadline, index, total),
        |index| match plans.get(index).and_then(Option::as_ref) {
            Some(plan) => run_trial(&mut session, work, plan, &revision, deadline),
            None => Err(SearchStop::RunFailed(NO_PLAN.to_owned())),
        },
    );
    let answer = answer_of(stop, trials, work.score_unit.clone());
    if let Some(candidate) = answer.candidate_to_keep() {
        keep_candidate(work, candidate)?;
    }
    // The stages are the study's once the search has kept its candidate: the evidence and the stages `controlStudio/open` gives (A8.6).
    let study = storage::open_study(
        &work.project_root,
        &work.loop_definition.id,
        &work.loop_definition.revision,
    )?;
    let evidence = stage_evidence_of(&work.loop_definition, &study)?;
    to_value(&answer.with_stages(evidence))
}

/// Plans every trial of the grid (C5): its settings, its model route and its campaign. A trial whose plan is refused is kept with the
/// refusal as its reason, so that it is shown and never run. The objective and the required scenarios must hold for every planned trial.
///
/// # Errors
/// The grid breaks a budget; the objective or a required scenario is missing from a planned trial; no trial has a valid plan; or the
/// loop has no parameter binding.
fn plan_trials(
    work: &SearchWork,
    observed: &BTreeMap<String, f64>,
) -> Result<(Vec<SearchTrial>, Vec<Option<TrialPlan>>), Failure> {
    check_trial_count(
        trial_count(&work.project.axes, &work.pin_values),
        work.project.maximum_candidates,
    )?;
    let mut trials = Vec::new();
    let mut plans = Vec::new();
    let mut refusals = Vec::new();
    for combination in grid(&work.project.axes, &work.pin_values) {
        match plan_trial(work, observed, &combination) {
            Ok(plan) => {
                let objective = &work.project.objective.requirement_id;
                if !names_objective(&plan.cases, objective) {
                    return Err(Failure::new(
                        OBJECTIVE_MISSING,
                        Cause::Settings,
                        NextAction::OpenRequirements,
                    ));
                }
                if !has_scenarios(&plan.cases, &work.project.required_scenario_ids) {
                    return Err(Failure::new(
                        SCENARIOS_MISSING,
                        Cause::Settings,
                        NextAction::OpenRequirements,
                    ));
                }
                trials.push(SearchTrial::planned(combination, None));
                plans.push(Some(plan));
            }
            Err(refusal) => {
                refusals.push(refusal.message.clone());
                trials.push(SearchTrial::planned(combination, Some(refusal.message)));
                plans.push(None);
            }
        }
    }
    if plans.iter().all(Option::is_none) {
        return Err(Failure::new(
            &format!("{NO_PLAN} {}", refusals.join(" ")),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok((trials, plans))
}

/// The plan of one trial (C5): its settings on the loop's binding, the selected model's route at those settings, and the campaign of the
/// loop with the settings and the route's initial values.
///
/// # Errors
/// The loop has no parameter binding, a setting is unread, the model's route refuses, or the campaign refuses.
fn plan_trial(
    work: &SearchWork,
    observed: &BTreeMap<String, f64>,
    combination: &BTreeMap<String, f64>,
) -> Result<TrialPlan, Failure> {
    let binding = work
        .loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| Failure::new(NO_LOOP_BINDING, Cause::Settings, NextAction::OpenLoopSetup))?;
    let settings = trial_settings(binding, observed, &work.pin_values, combination)?;
    let model: &GeneralModel = &work.model.model;
    let route = model_route(&work.loop_definition, model, &settings)?;
    let cases = engineer_verification_campaign(
        &work.loop_definition,
        model,
        &work.requirements,
        &settings,
        &route.initial_values,
    )?;
    Ok(TrialPlan {
        settings,
        simulation: route.simulation,
        cases,
    })
}

/// Before a trial (C5): the search ends when the engineer cancelled it or its wall-time budget ended. It also reports the trial it is about to run.
fn stop_before(
    context: &OperationContext,
    deadline: Instant,
    index: usize,
    total: usize,
) -> Option<SearchStop> {
    if context.is_cancelled() {
        return Some(SearchStop::Cancelled);
    }
    if Instant::now() >= deadline {
        return Some(SearchStop::BudgetExhausted);
    }
    context.progress(&format!("Trial {} of {total}.", index + 1));
    None
}

/// Runs one trial's cases as isolated runs, each journaled before its start, and gives the trial's evidence (C5): its bounds from the
/// cases' verdicts, and J from the target-response record after the settling deadline.
///
/// # Errors
/// The search's stop: cancelled, its budget ended, the program's revision changed, or a run could not be carried to its end.
fn run_trial(
    session: &mut RunSession<'_>,
    work: &SearchWork,
    plan: &TrialPlan,
    revision: &str,
    deadline: Instant,
) -> Result<TrialEvidence, SearchStop> {
    let mut statuses: Vec<RequirementStatus> = Vec::with_capacity(plan.cases.len());
    let mut target: Option<IsolatedRunStatus> = None;
    for case in &plan.cases {
        let remaining_ms = u64::try_from(
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis(),
        )
        .unwrap_or(u64::MAX)
        .min(ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS);
        if remaining_ms == 0 {
            return Err(SearchStop::BudgetExhausted);
        }
        let job = new_job_id();
        let mut request = trial_run_request(&TrialRun {
            loop_definition: &work.loop_definition,
            model: &work.model.model,
            simulation: &plan.simulation,
            case,
            settings: &plan.settings,
            request_id: &job,
            expected_project_revision: revision,
        })
        .map_err(|failure| SearchStop::RunFailed(failure.message))?;
        request.wall_budget_ms = request.wall_budget_ms.min(remaining_ms);
        let status = match run_job(session, &request) {
            Ok(status) => status,
            Err(failure) => return Err(stop_of_failure(session, revision, failure.message)),
        };
        if status.project_revision != revision {
            return Err(SearchStop::ProgramChanged);
        }
        statuses.push(case_status(case, &status));
        if case.id == TARGET_CASE {
            target = Some(status);
        }
    }
    let target_record = target.as_ref().and_then(|status| match status.evidence() {
        IsolatedRunEvidence::Complete(record) => Some(record),
        _ => None,
    });
    let channels = work
        .reference
        .as_deref()
        .map(|reference| (reference, work.feedback.as_str()));
    Ok(trial_evidence(
        &statuses,
        target_record,
        channels,
        work.settling_deadline_seconds,
    ))
}

/// The stop that a run's failure ends the search with (C5): a cancel the engineer asked for, a program whose revision the runtime now
/// reports as another, or the run's own failure.
fn stop_of_failure(session: &mut RunSession<'_>, revision: &str, text: String) -> SearchStop {
    if (session.is_cancelled)() {
        return SearchStop::Cancelled;
    }
    match session.client.isolated_run_readiness() {
        Ok(current) if current != revision => SearchStop::ProgramChanged,
        _ => SearchStop::RunFailed(text),
    }
}

/// Keeps the search's candidate as the study's Tune section (A8.1, B4.1): unapplied, with the requirements and the model revision it
/// was designed on. The section replaces the study's earlier Tune section.
///
/// # Errors
/// The study cannot be read or saved.
fn keep_candidate(work: &SearchWork, candidate: SearchCandidate) -> Result<(), Failure> {
    let section = search_section(
        work.model.model.identity().revision.clone(),
        work.requirements.clone(),
        candidate,
        work.pins.clone(),
    );
    let mut study = storage::open_study(
        &work.project_root,
        &work.loop_definition.id,
        &work.loop_definition.revision,
    )?;
    study.set_section(&section)?;
    storage::save_study(&work.project_root, &study, &[])
}
