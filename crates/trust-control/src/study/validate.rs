//! The Validate section of a study (specification 35, A8.1, A8.4, B5 and B10.2; numerical annex N8): the validation of the
//! candidate that Tune designed. The section keeps the conditions the runs shared (N8.1), the independent model check when this
//! validation ran one (B5.2), each scenario's current and candidate runs with their requirement results (B5.3, B5.4), the
//! overall result (N8.5) and whether Apply may write the candidate (B5.4).
//!
//! A validation is current only while the model, the candidate and the requirements are the ones it validated (A8.4). A new
//! model, a changed candidate or changed requirements leave it behind as history, and it can then not authorize Apply.
//! [`check_before_start`] refuses Validate without a current model and a current candidate (B5.5), and [`validate_outcome`]
//! composes the result from the jobs of the campaign, each judged by the rules of `validation` (N8.3 to N8.5, B5.4).

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::RunEvidence;
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::LoopDefinition;
use crate::messages::{
    ModelCheck, RequirementOutcome, RunOutcome, ScenarioOutcome, ValidateResult,
};
use crate::model::GeneralModel;
use crate::stages::{stage_availability, StageEvidence};
use crate::study::identify::IdentifySection;
use crate::study::is_recording_id;
use crate::study::tune::{refused_as, stage_evidence as tune_stage_evidence, TuneSection};
use crate::study::StudySection;
use crate::tuning::automatic_criteria::validate_criteria;
use crate::tuning::candidate::{validate_parameters, ControlPidCandidate};
use crate::tuning::requirements::TuningRequirements;
use crate::validation::configured_plant_verification::{isolated_plant_name, VerificationPlant};
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::engineer_verification_plan::{VerificationPlan, FIXED_PARAMETER_PREFIX};
use crate::validation::isolated_run_protocol::{
    IsolatedJobState, IsolatedRunEvidence, IsolatedRunStatus,
};
use crate::validation::run_requests::{observation_unit, run_seed, validation_run_of};
use crate::validation::run_validation::{
    apply_eligible, evaluate_recorded_run, RecordedRunEvaluation, RunCheck, RunRequirement,
    RunRequirementResult,
};
use crate::validation::texts;

/// The refusal of Validate without a model that is current for the loop's signals (B5.5: Identify).
const NO_MODEL: &str = "The study has no model for the loop's current signals. Identify the plant first, so Validate has a model to check.";
/// The refusal of Validate without a candidate designed on the current model (B5.5: Tune).
const NO_CANDIDATE: &str = "The study has no candidate for the current model. Tune the controller first, so Validate has a candidate to check.";
/// The sentence of a validation in which no requirement was checked, which can never pass (N8.5).
const NO_EVIDENCE: &str = "No requirement was checked, so the validation has no evidence to pass.";
/// The refusal of a program that changed during the campaign, so that its runs do not compare (B5.3).
const PROGRAM_CHANGED: &str = "The program changed during the mandatory verification campaign.";
/// The refusal of jobs that do not belong to the campaign they were given with.
const JOBS_MISMATCH: &str = "The validation was given jobs that do not match its campaign's cases.";

/// The validation conditions that the current and the candidate runs share (N8.1): the sample grid, the record period, the
/// seed, the values the runs start from and the signals they observe. A scenario's own events and initial values belong to
/// its runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationConditions {
    /// The task period in ms (A4.3): the step of every run.
    pub step_ms: u64,
    /// The run's duration in ms.
    pub duration_ms: u64,
    /// The scans each sample averages (A4.2); 1 records every scan.
    pub record_every: u64,
    /// The seed of the runs: the loop's experiment seed, or 1.
    pub seed: u64,
    /// The values the runs start from, by symbol, before any scenario's own values (N8.1).
    pub initial_values: BTreeMap<String, f64>,
    /// The signals the runs observe, in the plan's order.
    pub observed_symbols: Vec<String>,
}

/// The Validate section of a study, kept under the key `validate` (A8.1). It records what was validated and the result: the
/// candidate is unapplied, and it stays with the model and the requirements it was validated against (A8.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateSection {
    /// The revision of the model the candidate was validated on (A8.4).
    pub model_revision: String,
    /// The candidate that was validated, unapplied (N8.9). A changed candidate invalidates the validation (A8.4).
    pub candidate: ControlPidCandidate,
    /// The requirements that were validated (B4.3). Changed requirements invalidate the validation (A8.4).
    pub requirements: TuningRequirements,
    /// The conditions that the runs shared (N8.1).
    pub conditions: ValidationConditions,
    /// The independent model check that this validation ran (B5.2). Absent when the model was already qualified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_check: Option<ModelCheck>,
    /// Each scenario's current and candidate runs and their requirement results (B5.3, B5.4), in the campaign's order.
    pub scenarios: Vec<ScenarioOutcome>,
    /// The overall result of the candidate (N8.5).
    pub status: RequirementStatus,
    /// Whether Apply may write the candidate: the model is qualified and the candidate passed (B5.4).
    pub apply_available: bool,
}

impl ValidateSection {
    /// Whether the validation is current for the model and the Tune section (A8.4): it validated the study's model, the candidate
    /// that Tune designed on it, and the requirements that candidate was designed to.
    #[must_use]
    pub fn is_current(&self, model: &GeneralModel, tune: &TuneSection) -> bool {
        self.model_revision == model.identity().revision
            && self.candidate == tune.candidate
            && self.requirements == tune.requirements
    }
}

impl StudySection for ValidateSection {
    const KEY: &'static str = "validate";

    /// The model revision is named, the requirements and the candidate hold the rules that Tune's do (B4.3, C1), the sample
    /// grid is not empty, the runs share one program revision, and the overall result and the Apply rule follow from the runs
    /// (N8.5, B5.4). A section that breaks one is never kept and never read (B10.4).
    fn check(&self) -> Result<(), Failure> {
        if self.model_revision.is_empty() {
            return Err(invalid("it names no model revision", Cause::ModelOrData));
        }
        self.requirements
            .validate()
            .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
        if let Some(criteria) = &self.requirements.automatic_criteria {
            validate_criteria(criteria)
                .map_err(refused_as(Cause::Settings, NextAction::OpenRequirements))?;
        }
        validate_parameters(&self.candidate.parameters)
            .map_err(refused_as(Cause::Settings, NextAction::None))?;
        if self.candidate.applied {
            return Err(invalid(
                "its candidate is marked applied, but Validate never writes to the controller (N8.9)",
                Cause::Settings,
            ));
        }
        if self.conditions.step_ms == 0 || self.conditions.duration_ms == 0 {
            return Err(invalid("its sample grid is empty", Cause::ModelOrData));
        }
        if self
            .model_check
            .as_ref()
            .is_some_and(|check| !is_recording_id(&check.recording_id))
        {
            return Err(invalid(
                "its model check names no recording of the study",
                Cause::ModelOrData,
            ));
        }
        check_scenarios(&self.scenarios)?;
        if self.status != overall_status(&self.scenarios) {
            return Err(invalid(
                "its overall result does not follow from its runs (N8.5)",
                Cause::ModelOrData,
            ));
        }
        if self.apply_available != applies(self.model_check.as_ref(), self.status) {
            return Err(invalid(
                "its Apply rule does not follow from its model check and its result (B5.4)",
                Cause::ModelOrData,
            ));
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        self.model_check
            .iter()
            .map(|check| check.recording_id.as_str())
            .collect()
    }
}

/// What the refusals of Validate read (B5.5): the loop, the study's Identify and Tune sections, and the readiness of the
/// isolated simulation (E8.2): the project revision it runs on, or the reason it cannot run.
#[derive(Debug, Clone, Copy)]
pub struct ValidateInputs<'a> {
    /// The loop that is validated.
    pub loop_definition: &'a LoopDefinition,
    /// The study's Identify section, or `None` when the study has none.
    pub identify: Option<&'a IdentifySection>,
    /// The study's Tune section, or `None` when the study has none.
    pub tune: Option<&'a TuneSection>,
    /// The answer of the readiness check (E8.2, `ready_project_revision`).
    pub readiness: &'a Result<String, Failure>,
}

/// The refusals that come before Validate starts, in the order of B5.5: no model that is current for the loop, no candidate
/// that is current for that model, a rule of the Rust engine that the loop breaks, and an isolated simulation that is not
/// ready. Each refusal carries the next action that fixes it.
///
/// # Errors
/// The first of the refusals above that holds; for the readiness, the runtime's own failure, with its reason.
pub fn check_before_start(inputs: &ValidateInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    let identify = inputs
        .identify
        .filter(|section| section.is_current_for(loop_definition))
        .ok_or_else(no_model)?;
    inputs
        .tune
        .filter(|section| section.is_current_for(&identify.selected_model))
        .ok_or_else(no_candidate)?;
    loop_definition.rust_engine_support()?;
    match inputs.readiness {
        Ok(_) => Ok(()),
        Err(failure) => Err(failure.clone()),
    }
}

/// The overall result of a validation (N8.5, the campaign rule): Fail when a scenario's candidate run fails; Pass when every
/// scenario's candidate run passes and there is at least one scenario; Inconclusive otherwise. A run that did not complete,
/// or a scenario without its run, is Inconclusive, so missing evidence never passes.
#[must_use]
pub fn overall_status(scenarios: &[ScenarioOutcome]) -> RequirementStatus {
    let statuses: Vec<RequirementStatus> = scenarios
        .iter()
        .map(|scenario| scenario.candidate.status)
        .collect();
    if statuses.contains(&RequirementStatus::Fail) {
        RequirementStatus::Fail
    } else if !statuses.is_empty()
        && statuses
            .iter()
            .all(|status| *status == RequirementStatus::Pass)
    {
        RequirementStatus::Pass
    } else {
        RequirementStatus::Inconclusive
    }
}

/// Whether Apply may write the candidate (B5.4): the model check is not needed or passed, and the candidate's overall result
/// passed. The composition gives the same answer through `apply_eligible`.
fn applies(model_check: Option<&ModelCheck>, status: RequirementStatus) -> bool {
    let model_passed = match model_check {
        None => true,
        Some(check) => check.status == RequirementStatus::Pass,
    };
    model_passed && status == RequirementStatus::Pass
}

/// How the model stands for one validation (B5.2): qualified by an earlier independent check, which the Identify section names
/// by its recording, or provisional and checked by this validation.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelQualification {
    /// The model is already qualified, so no check runs now.
    Qualified,
    /// The model was provisional, and this validation ran the independent check, whose result is given.
    Checked(ModelCheck),
}

/// The jobs of one campaign case (B5.3): the isolated run of the current settings and the run of the candidate, each already
/// checked against its answer (`IsolatedRunStatus::check`), and the plant the case ran against.
#[derive(Debug, Clone, Copy)]
pub struct CaseJobs<'a> {
    /// The plant the case ran against: the configured plant, or the model.
    pub plant: &'a VerificationPlant,
    /// The job of the current settings.
    pub current: &'a IsolatedRunStatus,
    /// The job of the candidate.
    pub candidate: &'a IsolatedRunStatus,
}

/// What a validation is composed from (B5.1 to B5.4): the loop, its selected model and its Tune section, the stage evidence
/// before this validation, the campaign, the jobs of its cases in the campaign's order, and how the model stands.
pub struct ValidationInput<'a> {
    /// The loop the campaign tests.
    pub loop_definition: &'a LoopDefinition,
    /// The study's selected model, which is the model that is validated (B5.2).
    pub model: &'a GeneralModel,
    /// The study's Tune section: its candidate and requirements are the ones that are validated (B4.3).
    pub tune: &'a TuneSection,
    /// The evidence that sets the stages before this validation (`stage_evidence` without a Validate section): the Tune
    /// stage's, and whether the study holds a recording. This validation's own result is added to it.
    pub before: StageEvidence,
    /// The cases of the campaign, the target-response case first (`engineer_verification_campaign`).
    pub cases: &'a [CampaignCase],
    /// The jobs of each case, in the order of `cases`.
    pub jobs: &'a [CaseJobs<'a>],
    /// How the model stands for this validation (B5.2).
    pub qualification: ModelQualification,
    /// Why the model replaced the configured plant for every case (B5.3), when it did; the result carries it.
    pub plant_reason: Option<String>,
}

/// What Validate composes: the section to keep in the study, the result to answer, and the failure that stops Apply, with its
/// next action (B5.5), when Apply is not available.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidateOutcome {
    /// The section to keep in the study.
    pub section: ValidateSection,
    /// The result to answer.
    pub result: ValidateResult,
    /// Why Apply is not available, with its next action (B5.5); absent when it is available.
    pub failure: Option<Failure>,
}

/// Composes the validation from the jobs of its campaign (B5.1 to B5.4). Each completed run is evaluated against its case's
/// requirements (`evaluate_recorded_run`); each other job has no evidence, so its requirements are Inconclusive. The overall
/// result is the campaign rule (N8.5), and Apply is available only when the model check passed and the candidate passed
/// (`apply_eligible`). A failed model check still composes the result, with Apply unavailable: stopping after it is the
/// caller's choice (B5.5).
///
/// # Errors
/// The jobs do not match the cases, the runs ran on different program revisions, or a completed run is not one the evaluator
/// accepts (its requirements or its record).
pub fn validate_outcome(input: &ValidationInput<'_>) -> Result<ValidateOutcome, Failure> {
    let loop_definition = input.loop_definition;
    let tune = input.tune;
    if input.cases.len() != input.jobs.len() {
        return Err(Failure::new(
            JOBS_MISMATCH,
            Cause::Implementation,
            NextAction::None,
        ));
    }
    if !one_revision(input.jobs) {
        return Err(Failure::new(
            PROGRAM_CHANGED,
            Cause::Settings,
            NextAction::Validate,
        ));
    }
    let nominal = input
        .cases
        .first()
        .ok_or_else(|| Failure::new(JOBS_MISMATCH, Cause::Implementation, NextAction::None))?;
    let model_revision = input.model.identity().revision.clone();
    let mut scenarios = Vec::with_capacity(input.cases.len());
    let mut candidate_evaluations = Vec::with_capacity(input.cases.len());
    for (case, jobs) in input.cases.iter().zip(input.jobs) {
        let plant = isolated_plant_name(jobs.plant, &model_revision);
        let averaged = case.plan.record_every > 1;
        let current = job_outcome(loop_definition, case, jobs.current, &plant, averaged)?;
        let candidate = job_outcome(loop_definition, case, jobs.candidate, &plant, averaged)?;
        scenarios.push(ScenarioOutcome {
            id: case.id.clone(),
            plant,
            current: current.run,
            candidate: candidate.run,
        });
        candidate_evaluations.push(candidate.evaluation);
    }
    let status = overall_status(&scenarios);
    let model_status = match &input.qualification {
        ModelQualification::Qualified => RequirementStatus::Pass,
        ModelQualification::Checked(check) => check.status,
    };
    let model_check = match &input.qualification {
        ModelQualification::Qualified => None,
        ModelQualification::Checked(check) => Some(check.clone()),
    };
    let eligible = status == RequirementStatus::Pass
        && matches!(
            candidate_evaluations.first(),
            Some(Some(evaluation)) if apply_eligible(model_status, evaluation)
        );
    let section = ValidateSection {
        model_revision,
        candidate: tune.candidate,
        requirements: tune.requirements.clone(),
        conditions: conditions_of(loop_definition, &nominal.plan),
        model_check: model_check.clone(),
        scenarios: scenarios.clone(),
        status,
        apply_available: eligible,
    };
    let stages = stage_availability(&StageEvidence {
        validation_has_result: true,
        validation_passed: eligible,
        validation_inconclusive: status == RequirementStatus::Inconclusive,
        ..input.before
    });
    let failure = if eligible {
        None
    } else {
        Some(failure_of(
            model_check.as_ref(),
            status,
            &scenarios,
            &candidate_evaluations,
        ))
    };
    Ok(ValidateOutcome {
        section,
        result: ValidateResult {
            model_check,
            scenarios,
            status,
            apply_available: eligible,
            plant_reason: input.plant_reason.clone(),
            stages,
        },
        failure,
    })
}

/// The evidence that sets the stages' status (A8.4): the Tune stage's evidence, and the Validate result when it is current for
/// the study's model, its Tune candidate and its requirements. A passed current result makes Apply available; an inconclusive
/// one needs data, and a failed one has failed.
#[must_use]
pub fn stage_evidence(
    loop_definition: &LoopDefinition,
    identify: Option<&IdentifySection>,
    tune: Option<&TuneSection>,
    validate: Option<&ValidateSection>,
    has_recording: bool,
) -> StageEvidence {
    let evidence = tune_stage_evidence(loop_definition, identify, tune, has_recording);
    let current = validate.filter(|section| {
        identify.zip(tune).is_some_and(|(identify, tune)| {
            identify.is_current_for(loop_definition)
                && section.is_current(&identify.selected_model, tune)
        })
    });
    StageEvidence {
        validation_has_result: current.is_some(),
        validation_passed: current.is_some_and(|section| section.apply_available),
        validation_inconclusive: current
            .is_some_and(|section| section.status == RequirementStatus::Inconclusive),
        ..evidence
    }
}

/// One job as the validation reads it: its outcome for the result, and its evaluation when it completed.
struct JobOutcome {
    /// The outcome of the job for the result.
    run: RunOutcome,
    /// The evaluation of a completed run; `None` when the job did not complete.
    evaluation: Option<RecordedRunEvaluation>,
}

/// The outcome of one job: a completed run is evaluated against its case's requirements (N8.3, N8.4). Any other job has no
/// evidence, so its requirements are Inconclusive (B5.5, N8.5).
fn job_outcome(
    loop_definition: &LoopDefinition,
    case: &CampaignCase,
    job: &IsolatedRunStatus,
    plant: &str,
    averaged: bool,
) -> Result<JobOutcome, Failure> {
    let evaluation = if let IsolatedRunEvidence::Complete(record) = job.evidence() {
        let run = validation_run_of(record)?;
        let requirements = held_parameters(&case.requirements, &record.parameters);
        Some(evaluate_recorded_run(&run, &requirements, averaged)?)
    } else {
        None
    };
    let (status, requirements): (RequirementStatus, Vec<RequirementOutcome>) = match &evaluation {
        Some(evaluation) => (
            evaluation.status,
            evaluation
                .requirements
                .iter()
                .map(|result| requirement_of(result, plant))
                .collect(),
        ),
        None => (
            RequirementStatus::Inconclusive,
            not_run_requirements(
                loop_definition,
                &case.requirements,
                plant,
                &texts::run_not_completed(job_state_word(job.status), job.reason.as_deref()),
            ),
        ),
    };
    Ok(JobOutcome {
        run: RunOutcome {
            run_id: job.job_id.clone(),
            project_revision: job.project_revision.clone(),
            evidence: RunEvidence::StExecuted,
            status,
            requirements,
        },
        evaluation,
    })
}

/// The requirements of a run with its parameter overrides (B5.3, C0): a fixed parameter that the run overrode must hold the
/// override's value through the run, not the value the plan was built with; the other requirements are the case's own.
fn held_parameters(
    requirements: &[RunRequirement],
    parameters: &BTreeMap<String, f64>,
) -> Vec<RunRequirement> {
    requirements
        .iter()
        .map(|requirement| {
            let held = requirement
                .id
                .strip_prefix(FIXED_PARAMETER_PREFIX)
                .and_then(|symbol| parameters.get(symbol));
            match (held, &requirement.check) {
                (Some(&value), RunCheck::Bounds { .. }) => RunRequirement {
                    check: RunCheck::Bounds {
                        minimum: value,
                        maximum: value,
                    },
                    ..requirement.clone()
                },
                _ => requirement.clone(),
            }
        })
        .collect()
}

/// One requirement's result as the result keeps it, with the plant the run ran against as its source (B5.4).
fn requirement_of(result: &RunRequirementResult, source: &str) -> RequirementOutcome {
    RequirementOutcome {
        id: result.id.clone(),
        status: result.status,
        value: result.value,
        limit: result.limit,
        unit: result.unit.clone(),
        source: source.to_owned(),
        reason: result.reason.clone(),
    }
}

/// The requirements of a job that did not complete: each is Inconclusive and names the unit of its channel (B5.4).
fn not_run_requirements(
    loop_definition: &LoopDefinition,
    requirements: &[RunRequirement],
    source: &str,
    reason: &str,
) -> Vec<RequirementOutcome> {
    requirements
        .iter()
        .map(|requirement| RequirementOutcome {
            id: requirement.id.clone(),
            status: RequirementStatus::Inconclusive,
            value: None,
            limit: None,
            unit: observation_unit(loop_definition, &requirement.channel).unwrap_or_default(),
            source: source.to_owned(),
            reason: reason.to_owned(),
        })
        .collect()
}

/// The word of a job's state as the runtime writes it (E8.4), for the reason of a run that did not complete.
fn job_state_word(state: IsolatedJobState) -> &'static str {
    match state {
        IsolatedJobState::Queued => "queued",
        IsolatedJobState::Running => "running",
        IsolatedJobState::Completed => "completed",
        IsolatedJobState::Cancelled => "cancelled",
        IsolatedJobState::Failed => "failed",
    }
}

/// The requirements of the candidate runs of every scenario, in the campaign's order.
fn candidate_requirements(
    scenarios: &[ScenarioOutcome],
) -> impl Iterator<Item = &RequirementOutcome> {
    scenarios
        .iter()
        .flat_map(|scenario| scenario.candidate.requirements.iter())
}

/// Why Apply is not available, with the next action that fixes it (B5.5). A model check that did not pass comes first (Identify),
/// then a candidate requirement that failed (Tune). A requirement without evidence has no action of its own: the reason says
/// what is missing.
fn failure_of(
    model_check: Option<&ModelCheck>,
    status: RequirementStatus,
    scenarios: &[ScenarioOutcome],
    evaluations: &[Option<RecordedRunEvaluation>],
) -> Failure {
    if let Some(check) = model_check.filter(|check| check.status != RequirementStatus::Pass) {
        return Failure::new(&check.reason, Cause::ModelOrData, NextAction::Identify);
    }
    if status == RequirementStatus::Fail {
        if let Some(failed) = candidate_requirements(scenarios)
            .find(|requirement| requirement.status == RequirementStatus::Fail)
        {
            return Failure::new(&failed.reason, Cause::Settings, NextAction::Tune);
        }
    }
    let reason = candidate_requirements(scenarios)
        .find(|requirement| requirement.status != RequirementStatus::Pass)
        .map_or(NO_EVIDENCE, |requirement| requirement.reason.as_str());
    let cause = if evaluations.iter().any(Option::is_none) {
        Cause::Runtime
    } else {
        Cause::ModelOrData
    };
    Failure::new(reason, cause, NextAction::None)
}

/// The conditions that the runs of a validation share, from its target-response plan (N8.1).
fn conditions_of(
    loop_definition: &LoopDefinition,
    plan: &VerificationPlan,
) -> ValidationConditions {
    ValidationConditions {
        step_ms: plan.step_ms,
        duration_ms: plan.duration_ms,
        record_every: plan.record_every,
        seed: run_seed(loop_definition),
        initial_values: plan.initial_values.clone(),
        observed_symbols: plan.observed_symbols.clone(),
    }
}

/// Whether the jobs of a campaign, the current settings' and the candidate's, all ran on one project revision.
fn one_revision(jobs: &[CaseJobs<'_>]) -> bool {
    let mut revisions = jobs.iter().flat_map(|case_jobs| {
        [
            case_jobs.current.project_revision.as_str(),
            case_jobs.candidate.project_revision.as_str(),
        ]
    });
    match revisions.next() {
        None => true,
        Some(first) => revisions.all(|revision| revision == first),
    }
}

/// Checks the scenarios of a section: each has a name and a plant, no name repeats, each run has an identifier, and every run
/// ran on one project revision (B5.3).
fn check_scenarios(scenarios: &[ScenarioOutcome]) -> Result<(), Failure> {
    let mut ids = BTreeSet::new();
    let mut revisions = BTreeSet::new();
    for scenario in scenarios {
        if scenario.id.is_empty() || scenario.plant.is_empty() || !ids.insert(scenario.id.as_str())
        {
            return Err(invalid(
                "a scenario is unnamed, has no plant, or repeats another",
                Cause::ModelOrData,
            ));
        }
        for run in [&scenario.current, &scenario.candidate] {
            if run.run_id.is_empty() {
                return Err(invalid("a run has no identifier", Cause::ModelOrData));
            }
            revisions.insert(run.project_revision.as_str());
        }
    }
    if revisions.len() > 1 {
        return Err(invalid(
            "its runs ran on different program revisions",
            Cause::ModelOrData,
        ));
    }
    Ok(())
}

/// The failure of a stored Validate section that breaks one of its rules.
fn invalid(reason: &str, cause: Cause) -> Failure {
    Failure::new(
        &format!("The validate section of the study cannot be used: {reason}."),
        cause,
        NextAction::None,
    )
}

/// The failure of a study with no model for the loop's current signals (B5.5: Identify).
fn no_model() -> Failure {
    Failure::new(NO_MODEL, Cause::ModelOrData, NextAction::Identify)
}

/// The failure of a study with no candidate designed on its current model (B5.5: Tune).
fn no_candidate() -> Failure {
    Failure::new(NO_CANDIDATE, Cause::Settings, NextAction::Tune)
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
