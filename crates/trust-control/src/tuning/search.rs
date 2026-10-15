//! The finite search of a program's law (specification 35, C5): the grid of a search over the tuning axes of a `controller.project`
//! block, the isolated run request of each trial, the error J of a trial's record, the bounds and the ranking, the budgets (A7), and
//! the answer of a finished search. Nothing here reaches a runtime. The operation that runs the trials is `trust-control-studio`'s
//! `workflow::tune_search`, which calls these rules in their order and keeps the journal.

#![allow(clippy::module_name_repetitions)] // `SearchCandidate` and its siblings name the module's subject.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::recording_import::display_engineering_unit;
use crate::limits::SEARCH_TRIAL_LIMIT;
use crate::loop_definition::{LoopDefinition, ParameterBinding, ProjectAxis, RealType};
use crate::model::{GeneralModel, SimulationOverride};
use crate::stages::{stage_availability, StageAvailability, StageEvidence};
use crate::tuning::settings_pins::single_precision;
use crate::validation::engineer_verification_campaign::CampaignCase;
use crate::validation::isolated_run_protocol::{
    IsolatedParameterOverride, IsolatedParameterType, IsolatedRunEvidence, IsolatedRunRecord,
    IsolatedRunRequest, IsolatedRunStatus,
};
use crate::validation::run_requests::{
    isolated_run_request, validation_run_of, RunRequestInput, RunSettings,
};
use crate::validation::run_validation::evaluate_recorded_run;

/// The case whose record gives J: the nominal settling and error case of the campaign (C5, the `target-response` case).
pub const TARGET_CASE: &str = "target-response";
/// The tolerance of a sample's time against the start of the window, in seconds: sample times are computed in floating point.
const TIME_TOLERANCE_SECONDS: f64 = 1e-9;
/// The refusal of a grid with more trials than the loop's candidate budget or than the limit of A7.
const GRID_BUDGET: &str =
    "The declared tuning grid exceeds its candidate budget; pin settings or narrow the axes.";
/// The refusal of a search whose cases together run more scans than the loop's `maximumCycles`.
const SCAN_BUDGET: &str = "The mandatory tuning campaign exceeds its total-cycle budget.";
/// The refusal of a search in which no trial has a valid plan; the planning refusals follow it.
pub const NO_PLAN: &str =
    "No candidate has a valid initialization/scenario plan inside the stability constraints.";
/// The refusal of a search whose objective names no mandatory requirement of a trial.
pub const OBJECTIVE_MISSING: &str =
    "The tuning objective must name a mandatory observed requirement.";
/// The refusal of a search whose trials lack a declared required scenario.
pub const SCENARIOS_MISSING: &str =
    "The declared controller/component policy requires additional mandatory scenarios before tuning.";
/// The refusal of a loop with no parameter binding, which the run's parameters need.
const NO_BINDING: &str = "The loop declares no parameter binding.";
/// The answer's sentence when no attempted trial qualifies.
const NO_QUALIFYING: &str = "No candidate met every mandatory requirement within the declared tuning budget; the trials show the failing or missing conditions.";
/// The answer's sentence when the loaded program changed during the search.
const PROGRAM_CHANGED: &str = "The loaded program changed during controller tuning.";
/// The answer's sentence when the search's wall-time budget ended.
const BUDGET_ENDED: &str =
    "The wall-time budget of the search ended; the trials not attempted stay marked.";
/// The answer's sentence when the engineer cancelled the search.
const CANCELLED: &str = "The search was cancelled; the trials not attempted stay marked.";

/// The finite search of a program's law, kept unapplied in the study's Tune section (C5): the values of the selected trial by the variable
/// each axis tunes, the index of the selected trial, and every trial of the grid in its listed order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchCandidate {
    /// The values of the selected trial, by the variable of each axis.
    pub parameters: BTreeMap<String, f64>,
    /// The index of the selected trial in `trials`.
    pub selected: usize,
    /// Every trial of the grid, in the listed order: the attempted trials with their evidence, and the others marked not attempted.
    pub trials: Vec<SearchTrial>,
    /// The unit of J (C5): the measurement's unit squared times seconds, as the page shows it (`score_unit_of`).
    pub score_unit: String,
}

/// One combination of the grid (C5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchTrial {
    /// The combination: the value of each axis variable, a `REAL` value rounded to single precision.
    pub parameters: BTreeMap<String, f64>,
    /// Whether the trial's first isolated run was dispatched.
    pub attempted: bool,
    /// The bounds outcome of an attempted trial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<SearchBounds>,
    /// J (C5): the error after the settling time, in error unit squared times seconds, when the trial's record gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Why the trial was never run: its plan was refused before the search started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl SearchTrial {
    /// A trial that has not been run, with the reason its plan was refused when it was.
    #[must_use]
    pub fn planned(parameters: BTreeMap<String, f64>, reason: Option<String>) -> Self {
        Self {
            parameters,
            attempted: false,
            bounds: None,
            score: None,
            reason,
        }
    }
}

/// The bounds outcome of an attempted trial (C5): every mandatory requirement met, one not met, or evidence missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SearchBounds {
    /// Every case of the trial passed its requirements.
    Met,
    /// A case of the trial failed a requirement.
    NotMet,
    /// A case has no complete record, or its record cannot be evaluated: missing evidence never qualifies.
    MissingEvidence,
}

/// What one trial's runs showed (C5): its bounds outcome and J, when its record gives one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrialEvidence {
    /// The bounds outcome.
    pub bounds: SearchBounds,
    /// J, when the target-response record gives one.
    pub score: Option<f64>,
}

/// Why a search ends before its last trial (C5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStop {
    /// The engineer cancelled the search.
    Cancelled,
    /// The search's wall-time budget ended (A7).
    BudgetExhausted,
    /// The program's revision changed: the runtime answers another revision than the search's (C5).
    ProgramChanged,
    /// A run could not be carried to its end; the text is the failure's sentence.
    RunFailed(String),
}

/// How a search ended (the Tune answer's `status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SearchStatus {
    /// Every planned trial ran, and one qualifies.
    Completed,
    /// The wall-time budget ended before every planned trial ran.
    BudgetExhausted,
    /// No attempted trial qualifies.
    NoQualifyingTrial,
    /// The engineer cancelled the search.
    Cancelled,
    /// The program's revision changed during the search.
    ProgramChanged,
    /// A run could not be carried to its end.
    RunFailed,
}

/// The answer of a search, the operation's final update (C5): how it ended, the selected trial's index, every trial in its order, and the
/// unit of J. Once the candidate is kept, the answer also carries the stages the study has then (A8.6), as `controlStudio/open` gives them.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchAnswer {
    /// How the search ended.
    pub status: SearchStatus,
    /// The sentence that says why the search did not complete; absent when it completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The index of the selected trial, when one qualifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
    /// Every trial of the grid, in the listed order.
    pub trials: Vec<SearchTrial>,
    /// The unit of J (C5), as `score_unit_of` gives it.
    pub score_unit: String,
    /// The status of every stage, once the stage evidence is given (`with_stages`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<StageAvailability>>,
    /// The stage evidence the stages are computed from; the page enables Validate from its `has_candidate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_evidence: Option<StageEvidence>,
}

impl SearchAnswer {
    /// The candidate to keep in the study (C5): a search that completed, or ran out of budget with a selected trial, keeps it, and its
    /// trials that were not attempted stay marked. A cancelled, changed or failed search keeps nothing.
    #[must_use]
    pub fn candidate_to_keep(&self) -> Option<SearchCandidate> {
        if !matches!(
            self.status,
            SearchStatus::Completed | SearchStatus::BudgetExhausted
        ) {
            return None;
        }
        candidate_of(self.trials.clone(), self.selected?, self.score_unit.clone())
    }

    /// The answer with the stages of `evidence` (A8.6, B9.4): the stage evidence, and the status of every stage computed from it, as
    /// `controlStudio/open` computes them for a study.
    #[must_use]
    pub fn with_stages(mut self, evidence: StageEvidence) -> Self {
        self.stages = Some(stage_availability(&evidence));
        self.stage_evidence = Some(evidence);
        self
    }
}

/// The unit of J (C5): the measurement's unit squared times seconds, in the words the page shows (the Control Studio display of the
/// unit, squared, then `·s`), from the unit of the loop's process variable.
#[must_use]
pub fn score_unit_of(process_variable_unit: &str) -> String {
    format!("{}²·s", display_engineering_unit(process_variable_unit))
}

impl SearchCandidate {
    /// Whether the candidate holds its rules (B10.4, C5): its selected trial qualifies and carries its values, every trial names the same
    /// variables with finite values, and a trial that was not attempted holds no evidence.
    ///
    /// # Errors
    /// The sentence of the first rule that does not hold.
    pub fn check(&self) -> Result<(), String> {
        let selected = self
            .trials
            .get(self.selected)
            .ok_or("its selected trial is not one of its trials")?;
        if !qualifies(selected) {
            return Err("its selected trial does not qualify".to_owned());
        }
        if selected.parameters != self.parameters {
            return Err("its parameters are not those of its selected trial".to_owned());
        }
        for trial in &self.trials {
            if trial.parameters.keys().ne(self.parameters.keys())
                || !trial.parameters.values().all(|value| value.is_finite())
            {
                return Err(
                    "a trial names other variables, or a value that is not finite".to_owned(),
                );
            }
            if !trial.attempted && (trial.bounds.is_some() || trial.score.is_some()) {
                return Err("a trial that was not attempted holds evidence".to_owned());
            }
            if trial.score.is_some_and(|score| !score.is_finite()) {
                return Err("a trial's score is not finite".to_owned());
            }
        }
        Ok(())
    }
}

/// The number of combinations of the grid (C5): the product of the value lists, with one value for each pinned axis, which is not part of
/// the grid. Saturates, so that an oversized grid is refused without being built.
#[must_use]
pub fn trial_count(axes: &[ProjectAxis], pins: &BTreeMap<String, f64>) -> usize {
    axes.iter().fold(1_usize, |count, axis| {
        let values = if pins.contains_key(&axis.symbol) {
            1
        } else {
            axis.values.len()
        };
        count.saturating_mul(values)
    })
}

/// Refuses a grid with more trials than the loop's candidate budget or than the limit of A7 (64 trials per job).
///
/// # Errors
/// The grid has more trials than either limit.
pub fn check_trial_count(count: usize, maximum_candidates: u32) -> Result<(), Failure> {
    let budget = usize::try_from(maximum_candidates).unwrap_or(usize::MAX);
    let limit = usize::try_from(SEARCH_TRIAL_LIMIT).unwrap_or(usize::MAX);
    if count > budget.min(limit) {
        return Err(Failure::new(
            GRID_BUDGET,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(())
}

/// The grid (C5): the Cartesian product of the value lists, in the order the axes list them, the first axis outermost, so that the last
/// axis varies fastest. A pinned axis has its pinned value only.
#[must_use]
pub fn grid(axes: &[ProjectAxis], pins: &BTreeMap<String, f64>) -> Vec<BTreeMap<String, f64>> {
    let mut combinations: Vec<BTreeMap<String, f64>> = vec![BTreeMap::new()];
    for axis in axes {
        let values = match pins.get(&axis.symbol) {
            Some(pinned) => vec![*pinned],
            None => axis.values.clone(),
        };
        let mut next = Vec::with_capacity(combinations.len() * values.len());
        for combination in &combinations {
            for value in &values {
                let mut extended = combination.clone();
                extended.insert(axis.symbol.clone(), *value);
                next.push(extended);
            }
        }
        combinations = next;
    }
    combinations
}

/// The settings of one trial (C5): every bound parameter once, with the trial's value where the trial sets one, else the pinned value, else
/// the value the runtime holds now. A `REAL` value is rounded to the nearest `REAL` before the run.
///
/// # Errors
/// A bound parameter has neither a trial value, a pinned value nor a value read from the runtime.
pub fn trial_settings(
    binding: &ParameterBinding,
    observed: &BTreeMap<String, f64>,
    pins: &BTreeMap<String, f64>,
    combination: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    binding
        .parameters
        .iter()
        .map(|parameter| -> Result<(String, f64), Failure> {
            let value = combination
                .get(&parameter.symbol)
                .or_else(|| pins.get(&parameter.symbol))
                .or_else(|| observed.get(&parameter.symbol))
                .copied()
                .ok_or_else(|| {
                    Failure::new(
                        &format!(
                            "The current value of {} is not read from the runtime.",
                            parameter.symbol
                        ),
                        Cause::Runtime,
                        NextAction::ConnectRuntime,
                    )
                })?;
            let value = match &parameter.iec_type {
                RealType::Real => single_precision(value),
                RealType::Lreal => value,
            };
            Ok((parameter.symbol.clone(), value))
        })
        .collect()
}

/// What one case's isolated run request is built from (C5).
pub struct TrialRun<'a> {
    /// The loop the trial tests.
    pub loop_definition: &'a LoopDefinition,
    /// The selected model the trial runs against.
    pub model: &'a GeneralModel,
    /// The selected model's inline plant (`model_route`).
    pub simulation: &'a SimulationOverride,
    /// The campaign case the run executes.
    pub case: &'a CampaignCase,
    /// The settings of the trial, by symbol (`trial_settings`).
    pub settings: &'a BTreeMap<String, f64>,
    /// The request's identifier, a new one for each run.
    pub request_id: &'a str,
    /// The project revision the search runs on (E8.2).
    pub expected_project_revision: &'a str,
}

/// The isolated run request of one case of a trial (C5, E8.4): the case's grid, events and initial values, the observed channels, the
/// selected model's inline plant, and every bound parameter as a parameter override holding the trial's value. Every trial and every
/// case shares the loop's seed, so the trials differ only in their settings.
///
/// # Errors
/// The loop has no parameter binding, or the request breaks a rule of E8.4.
pub fn trial_run_request(input: &TrialRun<'_>) -> Result<IsolatedRunRequest, Failure> {
    let mut request = isolated_run_request(&RunRequestInput {
        loop_definition: input.loop_definition,
        model: input.model,
        plan: &input.case.plan,
        case: input.case,
        settings: RunSettings::Current(input.settings),
        simulation: Some(input.simulation),
        request_id: input.request_id,
        expected_project_revision: input.expected_project_revision,
    })?;
    let binding = input
        .loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| Failure::new(NO_BINDING, Cause::Settings, NextAction::OpenLoopSetup))?;
    // The search law has no record of its own, so its overrides are the trial's values of the bound parameters.
    request.parameter_overrides = bound_overrides(binding, input.settings)?;
    request.check()?;
    Ok(request)
}

/// Every bound parameter as a parameter override with the value the settings hold for it (E8.4), typed and bounded as the binding declares.
fn bound_overrides(
    binding: &ParameterBinding,
    settings: &BTreeMap<String, f64>,
) -> Result<Vec<IsolatedParameterOverride>, Failure> {
    binding
        .parameters
        .iter()
        .map(|parameter| -> Result<IsolatedParameterOverride, Failure> {
            let value = settings.get(&parameter.symbol).copied().ok_or_else(|| {
                Failure::new(
                    "A bound parameter holds no value in the trial's settings.",
                    Cause::Implementation,
                    NextAction::None,
                )
            })?;
            Ok(IsolatedParameterOverride {
                symbol: parameter.symbol.clone(),
                iec_type: match &parameter.iec_type {
                    RealType::Real => IsolatedParameterType::Real,
                    RealType::Lreal => IsolatedParameterType::Lreal,
                },
                unit: parameter.unit.clone(),
                convention: parameter.convention.clone(),
                minimum: parameter.minimum,
                maximum: parameter.maximum,
                value,
            })
        })
        .collect()
}

/// The planned scans of one case (C5): its duration over its step, and the first scan (`duration / step + 1`).
#[must_use]
pub fn case_scans(case: &CampaignCase) -> u64 {
    if case.plan.step_ms == 0 {
        return 0;
    }
    case.plan.duration_ms / case.plan.step_ms + 1
}

/// The planned scans of a trial's cases together (A7, the scans of a job).
#[must_use]
pub fn scans_of(cases: &[CampaignCase]) -> u64 {
    cases
        .iter()
        .fold(0_u64, |total, case| total.saturating_add(case_scans(case)))
}

/// Refuses a search whose trials together run more scans than the loop's `maximumCycles` (C5, A7).
///
/// # Errors
/// The planned scans exceed the budget.
pub fn check_scan_budget(scans: u64, maximum_cycles: u64) -> Result<(), Failure> {
    if scans > maximum_cycles {
        return Err(Failure::new(
            SCAN_BUDGET,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(())
}

/// Whether a trial's cases name the objective's requirement as one of their mandatory requirements (C5): the search ranks by it.
#[must_use]
pub fn names_objective(cases: &[CampaignCase], requirement_id: &str) -> bool {
    cases.iter().any(|case| {
        case.requirements
            .iter()
            .any(|requirement| requirement.id == requirement_id)
    })
}

/// Whether a trial's cases include every declared required scenario (C5).
#[must_use]
pub fn has_scenarios(cases: &[CampaignCase], required: &[String]) -> bool {
    required
        .iter()
        .all(|id| cases.iter().any(|case| &case.id == id))
}

/// The verdict of one case's run (C5): the case's requirements checked on the run's complete record. A run without a complete record, or a
/// record that cannot be evaluated, is inconclusive: missing evidence never passes.
#[must_use]
pub fn case_status(case: &CampaignCase, status: &IsolatedRunStatus) -> RequirementStatus {
    let IsolatedRunEvidence::Complete(record) = status.evidence() else {
        return RequirementStatus::Inconclusive;
    };
    let averaged = case.plan.record_every > 1;
    match validation_run_of(record)
        .and_then(|run| evaluate_recorded_run(&run, &case.requirements, averaged))
    {
        Ok(evaluation) => evaluation.status,
        Err(_) => RequirementStatus::Inconclusive,
    }
}

/// The bounds outcome of a trial from its cases' verdicts (C5): one failed case fails the trial, every case passing meets the bounds, and
/// anything else leaves the evidence missing.
#[must_use]
pub fn bounds_of(statuses: &[RequirementStatus]) -> SearchBounds {
    if statuses.contains(&RequirementStatus::Fail) {
        SearchBounds::NotMet
    } else if !statuses.is_empty()
        && statuses
            .iter()
            .all(|status| *status == RequirementStatus::Pass)
    {
        SearchBounds::Met
    } else {
        SearchBounds::MissingEvidence
    }
}

/// J's samples (C5): `(time, reference − feedback)` for each sample of the record from `from_seconds` on. `None` when a sample of that window
/// lacks one of the two values.
#[must_use]
pub fn error_samples(
    record: &IsolatedRunRecord,
    reference: &str,
    feedback: &str,
    from_seconds: f64,
) -> Option<Vec<(f64, f64)>> {
    record
        .samples
        .iter()
        .filter(|sample| sample.time_seconds + TIME_TOLERANCE_SECONDS >= from_seconds)
        .map(|sample| {
            let setpoint = sample.values.get(reference)?.value?;
            let measured = sample.values.get(feedback)?.value?;
            Some((sample.time_seconds, setpoint - measured))
        })
        .collect()
}

/// J (C5): the trapezoidal sampled integral of squared error, `Σₖ ½·(eₖ² + eₖ₊₁²)·Δt`, over `(time, error)` samples in time order, with the
/// actual step `Δt` between two samples. `None` with fewer than two samples, a time that does not increase, or a value that is not finite.
#[must_use]
pub fn squared_error_integral(samples: &[(f64, f64)]) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let mut total = 0.0;
    for pair in samples.windows(2) {
        let (start, first) = pair[0];
        let (end, second) = pair[1];
        let step = end - start;
        if !(step.is_finite() && step > 0.0 && first.is_finite() && second.is_finite()) {
            return None;
        }
        total += f64::midpoint(first * first, second * second) * step;
    }
    total.is_finite().then_some(total)
}

/// The evidence of a finished trial (C5): its bounds outcome from its cases' verdicts, and J over the target-response record from the
/// settling deadline on, for the reference and feedback channels the objective names. J is absent when the record, the channels or a
/// sample of the window is missing.
#[must_use]
pub fn trial_evidence(
    statuses: &[RequirementStatus],
    target: Option<&IsolatedRunRecord>,
    channels: Option<(&str, &str)>,
    deadline_seconds: f64,
) -> TrialEvidence {
    let score = match (target, channels) {
        (Some(record), Some((reference, feedback))) => {
            error_samples(record, reference, feedback, deadline_seconds)
                .and_then(|samples| squared_error_integral(&samples))
        }
        _ => None,
    };
    TrialEvidence {
        bounds: bounds_of(statuses),
        score,
    }
}

/// Whether a trial qualifies (C5): it was attempted, its bounds are met, and its J is a finite number. Missing or bad evidence never
/// qualifies, and a failing bound does not qualify.
#[must_use]
pub fn qualifies(trial: &SearchTrial) -> bool {
    trial.attempted
        && trial.bounds == Some(SearchBounds::Met)
        && trial.score.is_some_and(f64::is_finite)
}

/// The index of the qualifying trial with the smallest J (C5), or `None` when no trial qualifies. Of equal J, the first listed trial
/// is chosen.
#[must_use]
pub fn select(trials: &[SearchTrial]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, trial) in trials.iter().enumerate() {
        let Some(score) = trial.score.filter(|_| qualifies(trial)) else {
            continue;
        };
        if best.is_none_or(|(_, best_score)| score < best_score) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}

/// The candidate of the trial `selected` (C5): its values, its index, every trial and the unit of J. `None` when the index is not a trial.
#[must_use]
pub fn candidate_of(
    trials: Vec<SearchTrial>,
    selected: usize,
    score_unit: String,
) -> Option<SearchCandidate> {
    let parameters = trials.get(selected)?.parameters.clone();
    Some(SearchCandidate {
        parameters,
        selected,
        trials,
        score_unit,
    })
}

/// Runs the trials in their listed order (C5). `stop_before(index)` says whether the search ends before trial `index`, and `run(index)`
/// runs that trial's isolated runs and gives its evidence. A trial whose plan was refused is never run. A trial the search starts is
/// attempted, and a trial it stops during is attempted without evidence, so it does not qualify. Every other trial stays not attempted,
/// and nothing is dropped silently. Returns the stop that ended the search, or `None` when every planned trial ran.
pub fn run_trials(
    trials: &mut [SearchTrial],
    mut stop_before: impl FnMut(usize) -> Option<SearchStop>,
    mut run: impl FnMut(usize) -> Result<TrialEvidence, SearchStop>,
) -> Option<SearchStop> {
    for (index, trial) in trials.iter_mut().enumerate() {
        if trial.reason.is_some() {
            continue;
        }
        if let Some(stop) = stop_before(index) {
            return Some(stop);
        }
        trial.attempted = true;
        match run(index) {
            Ok(evidence) => {
                trial.bounds = Some(evidence.bounds);
                trial.score = evidence.score;
            }
            Err(stop) => {
                trial.bounds = Some(SearchBounds::MissingEvidence);
                return Some(stop);
            }
        }
    }
    None
}

/// The answer of a search that ended with `stop`, or completed when `None`, over its trials (C5).
#[must_use]
pub fn answer_of(
    stop: Option<SearchStop>,
    trials: Vec<SearchTrial>,
    score_unit: String,
) -> SearchAnswer {
    let selected = select(&trials);
    let (status, message) = match stop {
        None if selected.is_some() => (SearchStatus::Completed, None),
        None => (
            SearchStatus::NoQualifyingTrial,
            Some(NO_QUALIFYING.to_owned()),
        ),
        Some(SearchStop::BudgetExhausted) => {
            (SearchStatus::BudgetExhausted, Some(BUDGET_ENDED.to_owned()))
        }
        Some(SearchStop::Cancelled) => (SearchStatus::Cancelled, Some(CANCELLED.to_owned())),
        Some(SearchStop::ProgramChanged) => (
            SearchStatus::ProgramChanged,
            Some(PROGRAM_CHANGED.to_owned()),
        ),
        Some(SearchStop::RunFailed(text)) => (SearchStatus::RunFailed, Some(text)),
    };
    SearchAnswer {
        status,
        message,
        selected,
        trials,
        score_unit,
        stages: None,
        stage_evidence: None,
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
