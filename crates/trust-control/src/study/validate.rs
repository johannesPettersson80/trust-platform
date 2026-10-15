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
    InitializationOutcome, LimitSide, ModelCheck, RequirementChannel, RequirementOutcome,
    RunOutcome, ScenarioOutcome, ValidateResult, ValidationPlot, ValueRange,
};
use crate::model::GeneralModel;
use crate::stages::{stage_availability, StageEvidence};
use crate::study::current_model::CurrentModel;
use crate::study::is_recording_id;
use crate::study::tune::design::feedforward::shape_of;
use crate::study::tune::{refused_as, stage_evidence as tune_stage_evidence, TuneSection};
use crate::study::StudySection;
use crate::tuning::automatic_criteria::{validate_criteria, AutomaticCriteria};
use crate::tuning::candidate::{validate_parameters, ControlPidCandidate, LawCandidate};
use crate::tuning::project_candidate::{served_feedforward, ProjectCandidate};
use crate::tuning::project_seed::{FeedforwardPidSeed, FeedforwardTerms};
use crate::tuning::requirements::TuningRequirements;
use crate::validation::configured_plant_verification::{isolated_plant_name, VerificationPlant};
use crate::validation::engineer_scenario_effects::requirements_failure;
use crate::validation::engineer_verification_campaign::{declared_scenarios, CampaignCase};
use crate::validation::engineer_verification_plan::{VerificationPlan, FIXED_PARAMETER_PREFIX};
use crate::validation::feedforward_case::measured_disturbance_case;
use crate::validation::initialization_case::ENTRY_CASE_ID;
use crate::validation::initialization_texts;
use crate::validation::isolated_run_protocol::{
    IsolatedJobState, IsolatedRunEvidence, IsolatedRunStatus,
};
use crate::validation::run_requests::{observation_unit, run_seed, validation_run_of};
use crate::validation::run_validation::{
    apply_eligible, evaluate_recorded_run, DwellState, ErrorReference, Movement,
    RecordedRunEvaluation, RunCheck, RunRequirement, RunRequirementResult,
};
use crate::validation::scenario_contract::{valid_engineer_scenarios, EngineerScenario};
use crate::validation::texts;

mod answer;

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
/// The words the table names the measured-disturbance case with (C8).
const DISTURBANCE_CASE: &str = "Measured-disturbance case";
/// The identity and the label of the disturbance case's one row (C8).
const DISTURBANCE_ROW_ID: &str = "feedforward-disturbance";
const DISTURBANCE_ROW_LABEL: &str = "Largest deviation after settling";
/// The refusal of a feedforward whose Tune section holds no program candidate, which its disturbance case runs (C8).
const NO_FEEDFORWARD_CANDIDATE: &str =
    "The Tune result of the feedforward holds no program candidate, so its measured-disturbance case has no values to run.";
/// The refusal of a feedforward candidate that lacks a value its seed names (C8).
const FEEDFORWARD_VALUE_MISSING: &str =
    "The feedforward candidate holds no value for one of the variables its seed names.";

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
    /// The `Control.PID` candidate that was validated, unapplied (N8.9). Absent for a program's own law. A changed candidate
    /// invalidates the validation (A8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<ControlPidCandidate>,
    /// The program's own candidate that was validated, unapplied (N8.9, C2). Absent for a `Control.PID`. A changed candidate
    /// invalidates the validation (A8.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_candidate: Option<ProjectCandidate>,
    /// The requirements that were validated (B4.3). Changed requirements invalidate the validation (A8.4).
    pub requirements: TuningRequirements,
    /// The verification scenarios that the validation ran with (B4.3): the list the study declared, or the loop's own. A changed
    /// list invalidates the validation as changed requirements do (A8.4). A study written before this list was recorded reads it
    /// as empty, so its validation is current only where no scenario is declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_scenarios: Vec<EngineerScenario>,
    /// The conditions that the runs shared (N8.1).
    pub conditions: ValidationConditions,
    /// The independent model check that this validation ran (B5.2). Absent when the model was already qualified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_check: Option<ModelCheck>,
    /// Each scenario's current and candidate runs and their requirement results (B5.3, B5.4), in the campaign's order.
    pub scenarios: Vec<ScenarioOutcome>,
    /// The Manual-to-Auto entry case of an Auto commissioning, when this validation ran it (B5.3, B5.4). Absent when it ran none, and
    /// for a study written before the case was recorded (B10.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization: Option<InitializationOutcome>,
    /// The measured-disturbance case of a feedforward (C8), when this validation ran it: its row on the model, judged against the error
    /// allowance. Absent for any other loop, and for a study written before the case was recorded. A failed case ends Apply (B5.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disturbance_case: Option<DisturbanceCaseOutcome>,
    /// The sentence of the refusal that kept the Manual-to-Auto entry case from being planned (C1), when this validation refused it. The
    /// case is then Inconclusive with that sentence (B5.4). The study keeps the sentence, not its cause or next action, as it keeps no
    /// cause of a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_refusal: Option<String>,
    /// The overall result of the candidate (N8.5).
    pub status: RequirementStatus,
    /// Whether Apply may write the candidate: the model is qualified and the candidate passed (B5.4).
    pub apply_available: bool,
}

impl ValidateSection {
    /// Whether the validation is current (A8.4): it validated the study's model, the candidate that Tune designed on it, the
    /// requirements that candidate was designed to, and the declared scenarios the study holds now (`declared`).
    #[must_use]
    pub fn is_current(
        &self,
        model: &GeneralModel,
        tune: &TuneSection,
        declared: &[EngineerScenario],
    ) -> bool {
        self.model_revision == model.identity().revision
            && self.candidate == tune.candidate
            && self.project_candidate == tune.project_candidate
            && self.requirements == tune.requirements
            && self.declared_scenarios == declared
    }

    /// The candidate that was validated, whichever law it is for. `check` admits exactly one of the two candidates.
    #[must_use]
    pub fn law_candidate(&self) -> Option<LawCandidate> {
        match (&self.candidate, &self.project_candidate) {
            (Some(candidate), None) => Some(LawCandidate::Pid(*candidate)),
            (None, Some(candidate)) => Some(LawCandidate::Project(candidate.clone())),
            _ => None,
        }
    }

    /// The result of this validation as the study holds it (A8.6): the table's rows, the model check's row and whether Apply is
    /// available, with the failure that stops Apply when it does (B5.5). The plot and the plant reason are answer-only, because the
    /// study keeps neither (B5.4), so the result holds neither. `evidence` is the stage evidence the result is answered with, and
    /// its stages are computed from it.
    #[must_use]
    pub fn result(&self, evidence: StageEvidence) -> ValidateResult {
        // A refusal that the study kept is named by its sentence; a stored failure keeps no cause or next action of its own.
        let refusal = self
            .entry_refusal
            .as_ref()
            .map(|message| Failure::new(message, Cause::ModelOrData, NextAction::None));
        ValidateResult {
            model_check: self.model_check.clone(),
            scenarios: self.scenarios.clone(),
            requirements: self.requirements.clone(),
            status: self.status,
            apply_available: self.apply_available,
            // The study keeps no cause of a run, so a failure it holds is named without one (the cause is not shown on the page).
            failure: (!self.apply_available).then(|| {
                failure_of(
                    self.model_check.as_ref(),
                    self.status,
                    &self.scenarios,
                    &[],
                    self.initialization.as_ref(),
                    self.disturbance_case.as_ref(),
                    refusal.as_ref(),
                )
            }),
            plant_reason: None,
            stages: stage_availability(&evidence),
            stage_evidence: evidence,
            model_agreement: self.model_check.as_ref().map(answer::model_agreement),
            initialization: self.initialization.clone(),
            disturbance_case: self.disturbance_case.clone(),
            labels: answer::labels(),
            plot: None,
        }
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
        match (&self.candidate, &self.project_candidate) {
            (Some(candidate), None) => {
                validate_parameters(&candidate.parameters)
                    .map_err(refused_as(Cause::Settings, NextAction::None))?;
                if candidate.applied {
                    return Err(invalid(
                        "its candidate is marked applied, but Validate never writes to the controller (N8.9)",
                        Cause::Settings,
                    ));
                }
            }
            (None, Some(candidate)) => {
                if candidate.applied {
                    return Err(invalid(
                        "its program candidate is marked applied, but Validate never writes to the controller (N8.9)",
                        Cause::Settings,
                    ));
                }
                if !candidate.is_finite() {
                    return Err(invalid(
                        "its program candidate holds a value that is not finite",
                        Cause::Settings,
                    ));
                }
            }
            _ => {
                return Err(invalid(
                    "it holds neither a candidate nor a program candidate, or both (C1, C2)",
                    Cause::Settings,
                ))
            }
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
        if !valid_engineer_scenarios(&self.declared_scenarios) {
            return Err(requirements_failure(&texts::scenarios_invalid()));
        }
        check_scenarios(&self.scenarios)?;
        if let Some(entry) = &self.initialization {
            check_entry(entry)?;
        }
        if self.entry_refusal.is_some()
            && self.initialization.as_ref().map(|entry| entry.status)
                != Some(RequirementStatus::Inconclusive)
        {
            return Err(invalid(
                "its refused entry case is not Inconclusive (B5.4)",
                Cause::ModelOrData,
            ));
        }
        if let Some(case) = &self.disturbance_case {
            check_disturbance_case(case)?;
        }
        let overall = overall_result(
            &self.scenarios,
            self.initialization.as_ref(),
            self.disturbance_case.as_ref(),
        );
        if self.status != overall {
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

/// The measured-disturbance case of a feedforward's Validate (C8): one row on the model, as the Manual-to-Auto entry case is one set of rows
/// (B5.4). Its requirement compares the largest deviation of the measurement after the settling time with the error allowance, and its
/// result is that row's, so a case that cannot be judged is Inconclusive and never passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DisturbanceCaseOutcome {
    /// The words the table names the case with (C8): "Measured-disturbance case".
    pub label: String,
    /// The case's result (N8.5): the result of its row.
    pub status: RequirementStatus,
    /// The case's rows: its one requirement, with the measured deviation, the error allowance and the reason.
    pub requirements: Vec<RequirementOutcome>,
}

/// What the refusals of Validate read (B5.5): the loop, the study's current model and Tune section, and the readiness of the
/// isolated simulation (E8.2): the project revision it runs on, or the reason it cannot run.
#[derive(Debug, Clone, Copy)]
pub struct ValidateInputs<'a> {
    /// The loop that is validated.
    pub loop_definition: &'a LoopDefinition,
    /// The study's current model (B3.7), or `None` when the study has none.
    pub model: Option<&'a CurrentModel>,
    /// The study's Tune section, or `None` when the study has none.
    pub tune: Option<&'a TuneSection>,
    /// The answer of the readiness check (E8.2, `ready_project_revision`).
    pub readiness: &'a Result<String, Failure>,
}

/// The refusals that come before Validate starts, in the order of B5.5: a loop without a controller law (C12), no model that is
/// current for the loop, no candidate that is current for that model, a rule of the Rust engine that the loop breaks, and an
/// isolated simulation that is not ready. Each refusal carries the next action that fixes it.
///
/// # Errors
/// The first of the refusals above that holds; for the readiness, the runtime's own failure, with its reason.
pub fn check_before_start(inputs: &ValidateInputs<'_>) -> Result<(), Failure> {
    let loop_definition = inputs.loop_definition;
    loop_definition.require_controller_law()?;
    let current = inputs
        .model
        .filter(|current| current.is_current_for(loop_definition))
        .ok_or_else(no_model)?;
    inputs
        .tune
        .filter(|section| section.is_current_for(&current.model))
        .ok_or_else(no_candidate)?;
    loop_definition.rust_engine_support()?;
    match inputs.readiness {
        Ok(_) => Ok(()),
        Err(failure) => Err(failure.clone()),
    }
}

/// The overall result of a validation (N8.5, the campaign rule): Fail when a scenario's candidate run or the Manual-to-Auto entry case
/// fails; Pass when every one of them passes and there is at least one; Inconclusive otherwise. A run that did not complete, or a
/// scenario without its run, is Inconclusive, so missing evidence never passes (B5.4).
#[must_use]
pub fn overall_status(
    scenarios: &[ScenarioOutcome],
    initialization: Option<&InitializationOutcome>,
) -> RequirementStatus {
    overall_result(scenarios, initialization, None)
}

/// The overall result of a validation with its measured-disturbance case (N8.5, C8): the campaign rule over the candidate runs, the
/// Manual-to-Auto entry case and the disturbance case, so a failed case fails the candidate and ends Apply (B5.4).
#[must_use]
pub fn overall_result(
    scenarios: &[ScenarioOutcome],
    initialization: Option<&InitializationOutcome>,
    disturbance_case: Option<&DisturbanceCaseOutcome>,
) -> RequirementStatus {
    combined_status(
        scenarios
            .iter()
            .map(|scenario| scenario.candidate.status)
            .chain(initialization.map(|entry| entry.status))
            .chain(disturbance_case.map(|case| case.status)),
    )
}

/// The result of a set of statuses under the campaign rule (N8.5): Fail when one fails; Pass when all pass and there is at least one;
/// Inconclusive otherwise.
fn combined_status(statuses: impl Iterator<Item = RequirementStatus>) -> RequirementStatus {
    let statuses: Vec<RequirementStatus> = statuses.collect();
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
    /// The Manual-to-Auto entry case of this Auto commissioning (B5.3), planned and run or refused (`EntryJob`); `None` when this
    /// validation has no such case at all.
    pub entry: Option<EntryJob<'a>>,
}

/// The Manual-to-Auto entry case of an Auto commissioning, as this validation has it (B5.3): planned and run, with the job that the
/// runtime ended for it, or refused because the loop's Auto entry cannot be planned (C1). A refused case is not run; it is Inconclusive.
#[derive(Debug, Clone, Copy)]
pub enum EntryJob<'a> {
    /// The case was planned and its job ended on the plant it ran on.
    Ran {
        /// The campaign case of the entry run, whose requirements judge its first output, its mode, its states and its bounds.
        case: &'a CampaignCase,
        /// The entry run's job, checked against its answer (`IsolatedRunStatus::check`).
        job: &'a IsolatedRunStatus,
        /// The plant the entry run ran against: the configured plant, or the model.
        plant: &'a VerificationPlant,
    },
    /// The case could not be planned, so Auto is blocked (C1): the refusal is the case's one Inconclusive row and the failure that stops
    /// Apply (B5.4, B5.5).
    Refused(&'a Failure),
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
    check_jobs(input)?;
    let nominal = input
        .cases
        .first()
        .ok_or_else(|| Failure::new(JOBS_MISMATCH, Cause::Implementation, NextAction::None))?;
    let model_revision = input.model.identity().revision.clone();
    let campaign = campaign_outcomes(input, &model_revision)?;
    let initialization = input
        .entry
        .as_ref()
        .map(|entry| {
            entry_outcome(
                loop_definition,
                entry,
                &model_revision,
                tune.requirements.automatic_criteria.as_ref(),
            )
        })
        .transpose()?;
    let disturbance = disturbance_case_of(loop_definition, input.model, tune)?;
    let case = disturbance.as_ref();
    let status = overall_result(&campaign.scenarios, initialization.as_ref(), case);
    let refusal = match input.entry {
        Some(EntryJob::Refused(refusal)) => Some(refusal),
        Some(EntryJob::Ran { .. }) | None => None,
    };
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
            campaign.candidate_evaluations.first(),
            Some(Some(evaluation)) if apply_eligible(model_status, evaluation)
        );
    let section = ValidateSection {
        model_revision,
        candidate: tune.candidate,
        project_candidate: tune.project_candidate.clone(),
        requirements: tune.requirements.clone(),
        declared_scenarios: declared_scenarios(loop_definition)?,
        conditions: conditions_of(loop_definition, &nominal.plan),
        model_check: model_check.clone(),
        scenarios: campaign.scenarios.clone(),
        initialization: initialization.clone(),
        disturbance_case: disturbance.clone(),
        entry_refusal: refusal.map(|refusal| refusal.message.clone()),
        status,
        apply_available: eligible,
    };
    let evidence = StageEvidence {
        validation_has_result: true,
        validation_passed: eligible,
        validation_inconclusive: status == RequirementStatus::Inconclusive,
        ..input.before
    };
    let failure = if eligible {
        None
    } else {
        Some(failure_of(
            model_check.as_ref(),
            status,
            &campaign.scenarios,
            &campaign.candidate_evaluations,
            initialization.as_ref(),
            case,
            refusal,
        ))
    };
    Ok(ValidateOutcome {
        section,
        result: ValidateResult {
            model_agreement: model_check.as_ref().map(answer::model_agreement),
            initialization,
            disturbance_case: disturbance,
            model_check,
            scenarios: campaign.scenarios,
            requirements: tune.requirements.clone(),
            status,
            apply_available: eligible,
            failure: failure.clone(),
            plant_reason: input.plant_reason.clone(),
            stages: stage_availability(&evidence),
            stage_evidence: evidence,
            labels: answer::labels(),
            plot: campaign.plot,
        },
        failure,
    })
}

/// The jobs answer the campaign's cases one for one, and every run of the campaign ran on one program revision (B5.3).
///
/// # Errors
/// The jobs do not match the cases, or the runs ran on different program revisions.
fn check_jobs(input: &ValidationInput<'_>) -> Result<(), Failure> {
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
    Ok(())
}

/// What the cases of a campaign produce: each scenario in the campaign's order, the candidate's evaluation of each (`None` when
/// its job did not complete), and the plot of the target-response case (B5.3, B9.6).
struct CampaignOutcomes {
    scenarios: Vec<ScenarioOutcome>,
    candidate_evaluations: Vec<Option<RecordedRunEvaluation>>,
    plot: Option<ValidationPlot>,
}

/// Judges the current and the candidate job of each case against the case's requirements (B5.3, B5.4), in the order of the cases.
///
/// # Errors
/// A completed run is not one the evaluator accepts (its requirements or its record).
fn campaign_outcomes(
    input: &ValidationInput<'_>,
    model_revision: &str,
) -> Result<CampaignOutcomes, Failure> {
    let loop_definition = input.loop_definition;
    let criteria = input.tune.requirements.automatic_criteria.as_ref();
    let mut outcomes = CampaignOutcomes {
        scenarios: Vec::with_capacity(input.cases.len()),
        candidate_evaluations: Vec::with_capacity(input.cases.len()),
        plot: None,
    };
    for (index, (case, jobs)) in input.cases.iter().zip(input.jobs).enumerate() {
        let plant = isolated_plant_name(jobs.plant, model_revision);
        let averaged = case.plan.record_every > 1;
        let current = job_outcome(
            loop_definition,
            case,
            jobs.current,
            &plant,
            averaged,
            criteria,
        )?;
        let candidate = job_outcome(
            loop_definition,
            case,
            jobs.candidate,
            &plant,
            averaged,
            criteria,
        )?;
        // The plot shows the target-response case, the campaign's first (B5.3, B9.6).
        if index == 0 {
            outcomes.plot = answer::plot(
                loop_definition,
                current
                    .evaluation
                    .as_ref()
                    .map(|evaluation| &evaluation.run),
                candidate
                    .evaluation
                    .as_ref()
                    .map(|evaluation| &evaluation.run),
            );
        }
        outcomes.scenarios.push(ScenarioOutcome {
            id: case.id.clone(),
            label: texts::scenario_label(&case.id),
            plant,
            current: current.run,
            candidate: candidate.run,
        });
        outcomes.candidate_evaluations.push(candidate.evaluation);
    }
    Ok(outcomes)
}

/// The evidence that sets the stages' status (A8.4): the Tune stage's evidence, and the Validate result when it is current for
/// the study's model, its Tune candidate, its requirements and the declared scenarios the study holds (`declared`). A passed
/// current result makes Apply available; an inconclusive one needs data, and a failed one has failed.
#[must_use]
pub fn stage_evidence(
    loop_definition: &LoopDefinition,
    model: Option<&CurrentModel>,
    tune: Option<&TuneSection>,
    validate: Option<&ValidateSection>,
    has_recording: bool,
    declared: &[EngineerScenario],
) -> StageEvidence {
    let evidence = tune_stage_evidence(loop_definition, model, tune, has_recording);
    let current = validate.filter(|section| {
        model.zip(tune).is_some_and(|(current, tune)| {
            current.is_current_for(loop_definition)
                && section.is_current(&current.model, tune, declared)
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
    criteria: Option<&AutomaticCriteria>,
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
                .map(|result| {
                    let naming = case
                        .requirements
                        .iter()
                        .find(|requirement| requirement.id == result.id)
                        .map_or_else(
                            || Naming::unknown(&result.id, &result.unit),
                            |requirement| {
                                Naming::of(requirement, loop_definition, criteria, &case.plan)
                            },
                        );
                    // The ranges come from the requirement the run was evaluated against, with any fixed parameter held (B5.4).
                    let ranges = evaluation
                        .specification
                        .iter()
                        .find(|requirement| requirement.id == result.id)
                        .map_or((None, None), |requirement| {
                            answer::bounds_of(requirement, &evaluation.run)
                        });
                    requirement_of(result, naming, plant, ranges)
                })
                .collect(),
        ),
        None => (
            RequirementStatus::Inconclusive,
            not_run_requirements(
                loop_definition,
                case,
                plant,
                &texts::run_not_completed(job_state_word(job.status), job.reason.as_deref()),
                criteria,
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

/// The Manual-to-Auto entry case judged (B5.3, B5.4): its run is judged against the case's requirements as every other case is, and
/// each row is a guard (B9.9), because the engine generated the checks from the loop's declaration. A run that did not complete is
/// Inconclusive with its reason (`job_outcome`).
///
/// # Errors
/// A completed run is not one the evaluator accepts (its requirements or its record).
fn entry_outcome(
    loop_definition: &LoopDefinition,
    entry: &EntryJob<'_>,
    model_revision: &str,
    criteria: Option<&AutomaticCriteria>,
) -> Result<InitializationOutcome, Failure> {
    let (case, job, plant) = match *entry {
        EntryJob::Ran { case, job, plant } => (case, job, plant),
        EntryJob::Refused(refusal) => {
            return Ok(InitializationOutcome {
                label: texts::MANUAL_TO_AUTO_ENTRY.to_owned(),
                status: RequirementStatus::Inconclusive,
                requirements: vec![refused_row(refusal)],
            });
        }
    };
    let plant = isolated_plant_name(plant, model_revision);
    let averaged = case.plan.record_every > 1;
    let outcome = job_outcome(loop_definition, case, job, &plant, averaged, criteria)?;
    Ok(InitializationOutcome {
        label: texts::MANUAL_TO_AUTO_ENTRY.to_owned(),
        status: outcome.run.status,
        requirements: outcome
            .run
            .requirements
            .into_iter()
            .map(|row| RequirementOutcome { guard: true, ..row })
            .collect(),
    })
}

/// The one row of a refused Manual-to-Auto entry case (B5.4, B9.9): Inconclusive, under the case's identity, with the refusal's sentence
/// as its reason. It is a guard, because the engine generated the case from the loop's declaration.
fn refused_row(refusal: &Failure) -> RequirementOutcome {
    RequirementOutcome {
        id: ENTRY_CASE_ID.to_owned(),
        label: texts::MANUAL_TO_AUTO_ENTRY.to_owned(),
        channel: RequirementChannel {
            role: String::new(),
            unit: String::new(),
            symbol: String::new(),
        },
        limit_side: LimitSide::AtMost,
        status: RequirementStatus::Inconclusive,
        value: None,
        limit: None,
        unit: String::new(),
        source: String::new(),
        reason: refusal.message.clone(),
        automatic: false,
        guard: true,
        measured_range: None,
        limit_range: None,
    }
}

/// The measured-disturbance case of a feedforward's Validate (C8), when the loop has a feedforward: the candidate's PI and feedforward values
/// run on the model against a disturbance step, judged against the error allowance of the requirements (`measured_disturbance_case`). A case
/// that cannot be judged is Inconclusive with the sentence of its refusal (B5.4). A loop without a feedforward has none.
///
/// # Errors
/// The Tune section holds no program candidate, the candidate holds no value of the feedforward, or the loop's channels or disturbance step
/// do not fit the model (`shape_of`).
fn disturbance_case_of(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    tune: &TuneSection,
) -> Result<Option<DisturbanceCaseOutcome>, Failure> {
    let Some(seed) = served_feedforward(loop_definition) else {
        return Ok(None);
    };
    let candidate = tune.project_candidate.as_ref().ok_or_else(|| {
        Failure::new(
            NO_FEEDFORWARD_CANDIDATE,
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    let shape = shape_of(loop_definition, model)?;
    let terms = feedforward_terms(seed, candidate)?;
    let period_seconds = loop_definition.controller.task_period_ms / 1_000.0;
    let allowance = tune.requirements.maximum_error;
    let judged = measured_disturbance_case(
        model,
        &shape.channels,
        candidate.gains,
        terms,
        shape.disturbance_step,
        period_seconds,
        &tune.requirements,
    );
    let (status, value, reason) = match judged {
        Ok(outcome) => {
            let largest = outcome.largest_after_settling;
            if outcome.passed {
                (
                    RequirementStatus::Pass,
                    Some(largest),
                    format!(
                        "The measurement stays within the error allowance of {allowance:.3} from the settling time to the end of the disturbance step, with the candidate's PI and feedforward on the model (largest deviation {largest:.3})."
                    ),
                )
            } else {
                (
                    RequirementStatus::Fail,
                    Some(largest),
                    format!(
                        "The measurement leaves the error allowance of {allowance:.3} after the settling time: its largest deviation is {largest:.3} with the candidate's PI and feedforward on the model."
                    ),
                )
            }
        }
        Err(refusal) => (RequirementStatus::Inconclusive, None, refusal.0),
    };
    let process = &loop_definition.signals.process_variable;
    let row = RequirementOutcome {
        id: DISTURBANCE_ROW_ID.to_owned(),
        label: DISTURBANCE_ROW_LABEL.to_owned(),
        channel: RequirementChannel {
            role: "Measurement".to_owned(),
            unit: process.unit.clone(),
            symbol: process.symbol.clone(),
        },
        limit_side: LimitSide::AtMost,
        status,
        value,
        limit: Some(allowance),
        unit: process.unit.clone(),
        source: isolated_plant_name(&VerificationPlant::Model, &model.identity().revision),
        reason,
        automatic: false,
        guard: false,
        measured_range: None,
        limit_range: None,
    };
    Ok(Some(DisturbanceCaseOutcome {
        label: DISTURBANCE_CASE.to_owned(),
        status,
        requirements: vec![row],
    }))
}

/// The feedforward values of a candidate, read by their variables (C8): the gain, and the lead and the lag when the seed names them.
///
/// # Errors
/// The candidate holds no value for one of the variables that the seed names.
fn feedforward_terms(
    seed: &FeedforwardPidSeed,
    candidate: &ProjectCandidate,
) -> Result<FeedforwardTerms, Failure> {
    let value = |symbol: &str| {
        candidate.values.get(symbol).copied().ok_or_else(|| {
            Failure::new(
                FEEDFORWARD_VALUE_MISSING,
                Cause::Implementation,
                NextAction::None,
            )
        })
    };
    let gain = value(&seed.gain_symbol)?;
    let (lead_seconds, lag_seconds) = match &seed.lead_lag {
        Some(lead_lag) => (
            Some(value(&lead_lag.lead_symbol)?),
            Some(value(&lead_lag.lag_symbol)?),
        ),
        None => (None, None),
    };
    Ok(FeedforwardTerms {
        gain,
        lead_seconds,
        lag_seconds,
    })
}

/// The row of a disturbance case must hold the result the case states (N8.5).
///
/// # Errors
/// The case's result does not follow from its row.
fn check_disturbance_case(case: &DisturbanceCaseOutcome) -> Result<(), Failure> {
    if case.status != combined_status(case.requirements.iter().map(|row| row.status)) {
        return Err(invalid(
            "its measured-disturbance result does not follow from its row (N8.5)",
            Cause::ModelOrData,
        ));
    }
    Ok(())
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

/// One requirement's result as the result keeps it, named as the page shows it (B9.9), with the plant the run ran against as its
/// source (B5.4) and the measured range and the declared bounds of its check, when it has them (`answer::bounds_of`).
fn requirement_of(
    result: &RunRequirementResult,
    naming: Naming,
    source: &str,
    ranges: (Option<ValueRange>, Option<ValueRange>),
) -> RequirementOutcome {
    RequirementOutcome {
        id: result.id.clone(),
        label: naming.label,
        channel: naming.channel,
        limit_side: naming.limit_side,
        status: result.status,
        value: result.value,
        limit: result.limit,
        unit: result.unit.clone(),
        source: source.to_owned(),
        reason: result.reason.clone(),
        automatic: naming.automatic,
        guard: naming.guard,
        measured_range: ranges.0,
        limit_range: ranges.1,
    }
}

/// The requirements of a job that did not complete: each is Inconclusive and names the unit of its channel (B5.4).
fn not_run_requirements(
    loop_definition: &LoopDefinition,
    case: &CampaignCase,
    source: &str,
    reason: &str,
    criteria: Option<&AutomaticCriteria>,
) -> Vec<RequirementOutcome> {
    case.requirements
        .iter()
        .map(|requirement| {
            let naming = Naming::of(requirement, loop_definition, criteria, &case.plan);
            RequirementOutcome {
                id: requirement.id.clone(),
                unit: naming.channel.unit.clone(),
                label: naming.label,
                channel: naming.channel,
                limit_side: naming.limit_side,
                status: RequirementStatus::Inconclusive,
                value: None,
                limit: None,
                source: source.to_owned(),
                reason: reason.to_owned(),
                automatic: naming.automatic,
                guard: naming.guard,
                measured_range: None,
                limit_range: None,
            }
        })
        .collect()
}

/// How a requirement reads on the page (B9.9): its label in words, its channel named by its role and unit, which way its
/// limit bounds its value (B5.4), whether Automatic generated its limit (B4.3), and whether it is a guard (B9.9).
struct Naming {
    label: String,
    channel: RequirementChannel,
    limit_side: LimitSide,
    automatic: bool,
    guard: bool,
}

impl Naming {
    /// The naming of a requirement the campaign holds, with the criteria that Automatic generated, if any, and the plan that
    /// generated its guards.
    fn of(
        requirement: &RunRequirement,
        loop_definition: &LoopDefinition,
        criteria: Option<&AutomaticCriteria>,
        plan: &VerificationPlan,
    ) -> Self {
        // A parameter and a controller state are named by their own names, never by a signal's role.
        let by_name = requirement.id.starts_with(FIXED_PARAMETER_PREFIX)
            || matches!(requirement.check, RunCheck::StateBounds { .. });
        let role = if by_name {
            texts::readable_name(&requirement.channel)
        } else {
            signal_role(loop_definition, &requirement.channel)
                .unwrap_or_else(|| texts::readable_name(&requirement.channel))
        };
        Self {
            label: requirement_label(requirement, loop_definition),
            channel: RequirementChannel {
                role,
                unit: observation_unit(loop_definition, &requirement.channel).unwrap_or_default(),
                symbol: requirement.channel.clone(),
            },
            limit_side: if matches!(requirement.check, RunCheck::Dwell { .. }) {
                LimitSide::AtLeast
            } else {
                LimitSide::AtMost
            },
            automatic: answer::generated_limit(&requirement.check, criteria),
            guard: answer::is_guard(requirement, plan),
        }
    }

    /// The naming of a result whose requirement the campaign does not hold: its identity, made readable.
    fn unknown(id: &str, unit: &str) -> Self {
        Self {
            label: texts::readable_name(id),
            channel: RequirementChannel {
                role: texts::readable_name(id),
                unit: unit.to_owned(),
                symbol: String::new(),
            },
            limit_side: LimitSide::AtMost,
            automatic: false,
            guard: false,
        }
    }
}

/// The requirement's label in words (B9.9): the name of its check, with the reference or the direction that its name needs.
fn requirement_label(requirement: &RunRequirement, loop_definition: &LoopDefinition) -> String {
    if let Some(label) = entry_label(&requirement.id) {
        return label.to_owned();
    }
    if requirement.id.starts_with(FIXED_PARAMETER_PREFIX) {
        return texts::PARAMETER_UNCHANGED.to_owned();
    }
    match &requirement.check {
        RunCheck::Settling { .. } => texts::MAXIMUM_SETTLING_TIME.to_owned(),
        RunCheck::Overshoot { .. } => texts::MAXIMUM_OVERSHOOT.to_owned(),
        RunCheck::Error {
            reference: ErrorReference::Channel { channel },
            ..
        } if !is_setpoint(loop_definition, channel) => {
            let from = signal_role(loop_definition, channel)
                .unwrap_or_else(|| texts::readable_name(channel));
            texts::maximum_error_from(&from)
        }
        RunCheck::Error { .. } => texts::MAXIMUM_STEADY_STATE_ERROR.to_owned(),
        RunCheck::Bounds { .. } => texts::OPERATING_RANGE.to_owned(),
        RunCheck::StateBounds { .. } => texts::STATE_RANGE.to_owned(),
        RunCheck::Levels { .. } => texts::ALLOWED_OUTPUT_LEVELS.to_owned(),
        RunCheck::Dwell {
            state: DwellState::High,
            ..
        } => texts::MINIMUM_ON_TIME.to_owned(),
        RunCheck::Dwell {
            state: DwellState::Low,
            ..
        } => texts::MINIMUM_OFF_TIME.to_owned(),
        RunCheck::Rate {
            direction: Some(Movement::Up),
            ..
        } => texts::MAXIMUM_INCREASE_RATE.to_owned(),
        RunCheck::Rate {
            direction: Some(Movement::Down),
            ..
        } => texts::MAXIMUM_DECREASE_RATE.to_owned(),
        RunCheck::Rate {
            direction: None, ..
        } => texts::MAXIMUM_RATE.to_owned(),
        RunCheck::Saturation { .. } => texts::MAXIMUM_SATURATION_TIME.to_owned(),
        RunCheck::Switching { .. } => texts::MAXIMUM_SWITCHING_COUNT.to_owned(),
        RunCheck::Activity { .. } => texts::COMMAND_ACTIVITY.to_owned(),
    }
}

/// The name in words of a row of the Manual-to-Auto entry case (B5.3), by the identity the case gives it.
fn entry_label(id: &str) -> Option<&'static str> {
    match id {
        "transfer-continuity" => Some(initialization_texts::FIRST_OUTPUT_AFTER_ENTRY),
        "auto-mode" => Some(initialization_texts::MODE_AFTER_ENTRY),
        "integral-state" => Some(initialization_texts::INTEGRAL_AT_ENTRY),
        "derivative-state" => Some(initialization_texts::DERIVATIVE_AT_ENTRY),
        _ => None,
    }
}

/// Whether `symbol` is the loop's setpoint.
fn is_setpoint(loop_definition: &LoopDefinition, symbol: &str) -> bool {
    loop_definition
        .signals
        .setpoint
        .as_ref()
        .is_some_and(|signal| signal.symbol == symbol)
}

/// The role of a signal of the loop in words (B9.9): Measurement, Output, Setpoint or Actuator, or an auxiliary signal's id.
/// `None` when the loop declares no signal with that symbol.
fn signal_role(loop_definition: &LoopDefinition, symbol: &str) -> Option<String> {
    let signals = &loop_definition.signals;
    if signals.process_variable.symbol == symbol {
        Some(texts::MEASUREMENT.to_owned())
    } else if signals.manipulated_variable.symbol == symbol {
        Some(texts::OUTPUT.to_owned())
    } else if is_setpoint(loop_definition, symbol) {
        Some(texts::SETPOINT.to_owned())
    } else if signals
        .actuator
        .as_ref()
        .is_some_and(|signal| signal.symbol == symbol)
    {
        Some(texts::ACTUATOR.to_owned())
    } else {
        signals
            .additional
            .iter()
            .flatten()
            .find(|signal| signal.symbol == symbol)
            .map(|signal| signal.id.clone())
    }
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
    initialization: Option<&InitializationOutcome>,
    disturbance_case: Option<&DisturbanceCaseOutcome>,
    refusal: Option<&Failure>,
) -> Failure {
    if let Some(check) = model_check.filter(|check| check.status != RequirementStatus::Pass) {
        return Failure::new(&check.reason, Cause::ModelOrData, NextAction::Identify);
    }
    let entry_rows = initialization.map_or(&[][..], |entry| entry.requirements.as_slice());
    let case_rows = disturbance_case.map_or(&[][..], |case| case.requirements.as_slice());
    if status == RequirementStatus::Fail {
        if let Some(failed) = candidate_requirements(scenarios)
            .find(|requirement| requirement.status == RequirementStatus::Fail)
        {
            return Failure::new(&failed.reason, Cause::Settings, NextAction::Tune);
        }
        // A failed row of the entry case names the first output, the mode, the states or the bounds after Auto entry (B5.3).
        if let Some(failed) = entry_rows
            .iter()
            .find(|row| row.status == RequirementStatus::Fail)
        {
            return Failure::new(
                &failed.reason,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            );
        }
        // A failed row of the measured-disturbance case: the feedforward's gain or its PI does not hold the error allowance (C8, B5.4).
        if let Some(failed) = case_rows
            .iter()
            .find(|row| row.status == RequirementStatus::Fail)
        {
            return Failure::new(&failed.reason, Cause::Settings, NextAction::Tune);
        }
    }
    // An Auto entry that cannot be planned names its own refusal, with the cause and next action that it carries (B5.5, C1).
    if let Some(refusal) = refusal {
        return refusal.clone();
    }
    let reason = candidate_requirements(scenarios)
        .chain(entry_rows.iter())
        .chain(case_rows.iter())
        .find(|requirement| requirement.status != RequirementStatus::Pass)
        .map_or(NO_EVIDENCE, |requirement| requirement.reason.as_str());
    let entry_incomplete =
        initialization.is_some_and(|entry| entry.status == RequirementStatus::Inconclusive);
    let cause = if evaluations.iter().any(Option::is_none) || entry_incomplete {
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

/// A stored Manual-to-Auto entry case is kept only when each of its rows is a guard (B9.9) and its status follows from its rows (N8.5).
fn check_entry(entry: &InitializationOutcome) -> Result<(), Failure> {
    if entry.requirements.iter().any(|row| !row.guard) {
        return Err(invalid(
            "its entry rows are not all guards (B9.9)",
            Cause::ModelOrData,
        ));
    }
    if entry.status != combined_status(entry.requirements.iter().map(|row| row.status)) {
        return Err(invalid(
            "its entry result does not follow from its rows (N8.5)",
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

#[cfg(test)]
mod guard_rows_tests;

#[cfg(test)]
#[path = "validate_entry_tests.rs"]
mod entry_tests;

#[cfg(test)]
#[path = "validate_disturbance_tests.rs"]
mod disturbance_tests;
