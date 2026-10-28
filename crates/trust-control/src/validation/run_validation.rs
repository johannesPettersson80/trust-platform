//! The evaluation of a recorded run against its requirements (specification 35, B5.4 and A11.1; numerics N8.3 to N8.6; the
//! rules of `runValidation.ts`). A requirement measures one channel over its window and is Pass, Fail or Inconclusive, with
//! its value, limit, unit and reason. The run's status aggregates its requirements (N8.5). A comparison ranks two runs of one
//! experiment by a requirement that the engineer chose (N8.6), and Apply needs the model check and the candidate to pass
//! (B5.4). The arithmetic is `trust_control_math::measure`, and every sentence is in `validation::texts`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::measure::{self, Movement as MeasureMovement};
use trust_control_math::sampling::{first_gap, median_spacing, smallest};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::{RunConditions, ValidationRun, ValidationSample};
use crate::identification::independent_validation::RequirementStatus::{Fail, Inconclusive, Pass};
use crate::identification::independent_validation::{RequirementStatus, RunEvaluation};
use crate::limits::{ISOLATED_SIMULATION_CHANNEL_LIMIT, ISOLATED_SIMULATION_SAMPLE_LIMIT};
use crate::recording::channels::ChannelQuality;
use crate::recording::sequence::steady_record_period;
use crate::validation::texts;

pub use super::requirement_record::{
    DwellState, ErrorReference, ErrorStatistic, Movement, RunCheck, RunRequirement,
    RunRequirementResult, Window,
};

/// The tolerance of a time comparison, in seconds, as `runValidation.ts` compares times.
const TIME_TOLERANCE: f64 = 1e-9;
/// The largest integer that JSON carries exactly, 2⁵³ − 1 (E0.3).
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
/// The most levels that a levels requirement names.
const MAX_LEVELS: usize = 16;

/// The evaluation of one recorded run against its requirements (`RunEvaluation` in `runValidation.ts`): the aggregate status,
/// the run as it was evaluated, the requirements as they were checked, one verdict per requirement and the scope sentence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordedRunEvaluation {
    /// The aggregate status (N8.5).
    pub status: RequirementStatus,
    /// The run, as it was evaluated.
    pub run: ValidationRun,
    /// The requirements, as they were checked.
    pub specification: Vec<RunRequirement>,
    /// One verdict per requirement, in the same order.
    pub requirements: Vec<RunRequirementResult>,
    /// The sentence that says what the evaluation covers.
    pub scope: String,
}

impl RunEvaluation for RecordedRunEvaluation {
    fn status(&self) -> RequirementStatus {
        self.status
    }

    fn finding(&self) -> Option<&str> {
        self.requirements
            .iter()
            .find(|result| result.status != Pass)
            .map(|result| result.reason.as_str())
    }
}

/// Which run a comparison prefers (N8.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Preference {
    /// The current settings measured less.
    Current,
    /// The candidate measured less.
    Candidate,
    /// Both measured the same.
    Tie,
}

/// The comparison of a current and a candidate run of one experiment (N8.6): the aggregate status, both evaluations, the
/// preferred run when one can be named, and the reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunComparison {
    /// The aggregate status of the comparison.
    pub status: RequirementStatus,
    /// The evaluation of the current run.
    pub current: RecordedRunEvaluation,
    /// The evaluation of the candidate run.
    pub candidate: RecordedRunEvaluation,
    /// The preferred run, when a ranking was made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred: Option<Preference>,
    /// The sentence that says what the comparison found.
    pub reason: String,
}

/// Whether Apply is available for a candidate (B5.4, N8.5): the independent model check passed, and the candidate's
/// evaluation passed. Every requirement is mandatory, because the `TypeScript` declares no optional one. An evaluation with no
/// requirements is Inconclusive, so it is never eligible.
#[must_use]
pub fn apply_eligible(model_check: RequirementStatus, evaluation: &RecordedRunEvaluation) -> bool {
    model_check == Pass && evaluation.status == Pass
}

/// Checks the run and its requirements, then evaluates each requirement and aggregates the verdicts (`evaluateRecordedRun`).
/// `averaged` is true when the run was recorded at a record period (A4.2), so that its samples sit at the mean times of their
/// executions (E4.4).
///
/// # Errors
/// Returns a refusal when the run does not match the recorded-run contract, when a requirement's identity, window or limits
/// are invalid, or when two requirements share an identifier.
pub fn evaluate_recorded_run(
    run: &ValidationRun,
    requirements: &[RunRequirement],
    averaged: bool,
) -> Result<RecordedRunEvaluation, Failure> {
    check_run(run, averaged)?;
    for requirement in requirements {
        check_requirement(requirement, run)?;
        if !window_on_grid(requirement, run, averaged) {
            return Err(requirement_failure(&texts::invalid_requirement_window()));
        }
    }
    if has_duplicate_ids(requirements) {
        return Err(requirement_failure(&texts::requirement_ids_not_unique()));
    }
    let results: Vec<RunRequirementResult> = requirements
        .iter()
        .map(|requirement| evaluate_observed_requirement(run, requirement, averaged))
        .collect();
    let statuses: Vec<RequirementStatus> = results.iter().map(|result| result.status).collect();
    Ok(RecordedRunEvaluation {
        status: aggregate(&statuses),
        run: run.clone(),
        specification: requirements.to_vec(),
        requirements: results,
        scope: texts::evaluation_scope(),
    })
}

/// Compares a current and a candidate run of one experiment (`compareRecordedRuns`, N8.6). The runs must share their evidence
/// class, immutable conditions, channels and parameter identities, or the comparison is Inconclusive and ranks nothing. When
/// `objective` names a requirement, the run with the smaller measured value is preferred, but only when both pass every
/// requirement. `averaged` is as in [`evaluate_recorded_run`].
///
/// # Errors
/// Returns the refusal of either evaluation, or, when the runs match, the refusal of an objective that names no requirement.
pub fn compare_recorded_runs(
    current: &ValidationRun,
    candidate: &ValidationRun,
    requirements: &[RunRequirement],
    averaged: bool,
    objective: Option<&str>,
) -> Result<RunComparison, Failure> {
    let current_evaluation = evaluate_recorded_run(current, requirements, averaged)?;
    let candidate_evaluation = evaluate_recorded_run(candidate, requirements, averaged)?;
    if !same_experiment(current, candidate) {
        return Ok(RunComparison {
            status: Inconclusive,
            current: current_evaluation,
            candidate: candidate_evaluation,
            preferred: None,
            reason: texts::comparison_conditions_differ(),
        });
    }
    let status = aggregate(&[current_evaluation.status, candidate_evaluation.status]);
    let ranking = match objective {
        None => Ranking {
            status: None,
            preferred: None,
            reason: texts::no_ranking_objective(),
        },
        Some(objective) => rank(objective, &current_evaluation, &candidate_evaluation)?,
    };
    Ok(RunComparison {
        status: ranking.status.unwrap_or(status),
        current: current_evaluation,
        candidate: candidate_evaluation,
        preferred: ranking.preferred,
        reason: ranking.reason,
    })
}

/// The ranking of two evaluations (N8.6): the preferred run when one can be named, and the reason. `status` overrides the
/// comparison's aggregate status only when a ranking was due but a measured value is missing.
struct Ranking {
    status: Option<RequirementStatus>,
    preferred: Option<Preference>,
    reason: String,
}

/// Ranks two evaluations by the requirement `objective` (N8.6). A run ranks only when it passes every requirement; two passing
/// runs are ranked by the smaller measured value of the objective.
///
/// # Errors
/// Returns the refusal of an objective that names no requirement of the evaluations.
fn rank(
    objective: &str,
    current: &RecordedRunEvaluation,
    candidate: &RecordedRunEvaluation,
) -> Result<Ranking, Failure> {
    let (Some(current_row), Some(candidate_row)) = (
        find_result(current, objective),
        find_result(candidate, objective),
    ) else {
        return Err(requirement_failure(&texts::ranking_objective_undeclared()));
    };
    let ranked = |preferred: Option<Preference>, reason: String| Ranking {
        status: None,
        preferred,
        reason,
    };
    match (current.status == Pass, candidate.status == Pass) {
        (true, true) => {}
        (false, false) => return Ok(ranked(None, texts::neither_run_qualifies())),
        (false, true) => {
            let reason = texts::only_candidate_qualifies();
            return Ok(ranked(Some(Preference::Candidate), reason));
        }
        (true, false) => {
            let reason = texts::only_current_qualifies();
            return Ok(ranked(Some(Preference::Current), reason));
        }
    }
    let (Some(current_value), Some(candidate_value)) = (current_row.value, candidate_row.value)
    else {
        return Ok(Ranking {
            status: Some(Inconclusive),
            preferred: None,
            reason: texts::ranking_measurement_unavailable(),
        });
    };
    let preferred = if exactly_equal(current_value, candidate_value) {
        Preference::Tie
    } else if current_value < candidate_value {
        Preference::Current
    } else {
        Preference::Candidate
    };
    Ok(ranked(Some(preferred), texts::both_runs_qualify(objective)))
}

/// The checks of one requirement against the run (`decodeRunRequirement`): its identifier, its channel and its window lie in the
/// run, and its calculation and limits are valid for its kind.
///
/// # Errors
/// Returns a refusal when the identifier, the channel or the window is invalid, or when the calculation or its limits are invalid.
pub fn check_requirement(requirement: &RunRequirement, run: &ValidationRun) -> Result<(), Failure> {
    if !window_and_channel_valid(requirement, run) {
        return Err(requirement_failure(&texts::invalid_requirement_window()));
    }
    if !limits_valid(requirement, run) {
        return Err(requirement_failure(&texts::invalid_requirement_limits()));
    }
    Ok(())
}

/// The verdict on one requirement of a run (`evaluateObservedRequirement`). The window's samples are placed on the declared
/// sample times, each taking the latest recorded scan at or before it (N8.4), and the requirement is judged only when that
/// coverage is complete. Call it on a requirement that [`check_requirement`] accepted. A requirement whose channel the run does
/// not hold is Inconclusive.
#[must_use]
pub fn evaluate_observed_requirement(
    run: &ValidationRun,
    requirement: &RunRequirement,
    averaged: bool,
) -> RunRequirementResult {
    let kind = kind_word(&requirement.check);
    let limit = declared_limit(&requirement.check);
    let Some(channel) = run
        .channels
        .iter()
        .find(|candidate| candidate.id == requirement.channel)
    else {
        let labels = Labels::new(requirement, kind, String::new());
        return inconclusive(&labels, limit, texts::invalid_requirement_window());
    };
    let labels = Labels::new(
        requirement,
        kind,
        result_unit(&requirement.check, &channel.unit),
    );
    let grid = grid_times(run, requirement.window);
    let needed = needed_channels(requirement);
    let scans = match held_on_grid(run, &grid, &needed, requirement.window, averaged) {
        Ok(scans) => scans,
        Err(reason) => return inconclusive(&labels, limit, reason),
    };
    let Some(values) = values_of(&scans, &requirement.channel) else {
        return inconclusive(&labels, limit, texts::incomplete_coverage());
    };
    verdict(&labels, requirement, &grid, &values, &scans)
}

/// The verdict of a requirement's check over its observed samples: `grid` holds the declared sample times of the window,
/// `values` the measured channel at each of them, and `scans` the recorded scans that hold them.
fn verdict(
    labels: &Labels,
    requirement: &RunRequirement,
    grid: &[f64],
    values: &[f64],
    scans: &[&ValidationSample],
) -> RunRequirementResult {
    let window = requirement.window;
    match &requirement.check {
        RunCheck::Error {
            reference,
            statistic,
            maximum,
        } => error_verdict(labels, reference, *statistic, *maximum, values, scans),
        RunCheck::Overshoot {
            initial,
            target,
            maximum,
        } => {
            let direction = (target - initial).signum();
            judge(
                labels,
                measure::overshoot(direction, values, *target),
                *maximum,
            )
        }
        RunCheck::Settling {
            target,
            band,
            hold_seconds,
            deadline_seconds,
        } => match settling_arrival(grid, values, *target, *band, window.end, *hold_seconds) {
            Some(time) => judge(labels, time - window.start, *deadline_seconds),
            None => finish(
                labels,
                Fail,
                None,
                Some(*deadline_seconds),
                texts::no_arrival_in_band(),
            ),
        },
        RunCheck::Bounds { minimum, maximum } | RunCheck::StateBounds { minimum, maximum } => {
            let violation = measure::bound_violation(values, *minimum, *maximum);
            judge(labels, violation, 0.0)
        }
        RunCheck::Levels { allowed, tolerance } => {
            judge(labels, measure::level_distance(values, allowed), *tolerance)
        }
        RunCheck::Dwell {
            threshold,
            state,
            minimum_seconds,
        } => {
            let high = matches!(state, DwellState::High);
            let intervals = measure::dwell_intervals(grid, values, *threshold, high);
            dwell_verdict(labels, &intervals, *state, *minimum_seconds)
        }
        RunCheck::Rate { maximum, direction } => {
            let movement = match direction {
                Some(Movement::Up) => MeasureMovement::Up,
                Some(Movement::Down) => MeasureMovement::Down,
                None => MeasureMovement::Either,
            };
            judge(
                labels,
                measure::adjacent_rate(grid, values, movement),
                *maximum,
            )
        }
        RunCheck::Saturation {
            minimum,
            maximum,
            maximum_seconds,
        } => {
            let saturated = measure::saturation_time(grid, values, *minimum, *maximum);
            judge(labels, saturated, *maximum_seconds)
        }
        RunCheck::Switching { threshold, maximum } => {
            let switches = measure::switching_count(values, *threshold);
            judge(labels, switches, count_limit(*maximum))
        }
        RunCheck::Activity { maximum } => {
            activity_verdict(labels, measure::movement_total(values), *maximum)
        }
    }
}

/// The verdict of an error requirement: the errors `observed − reference`, measured by the chosen statistic and compared with
/// the maximum (N8.3); a signed mean passes on its magnitude (`ErrorStatistic::MeanError`). A reference channel whose coverage
/// is incomplete makes the requirement Inconclusive.
fn error_verdict(
    labels: &Labels,
    reference: &ErrorReference,
    statistic: ErrorStatistic,
    maximum: f64,
    values: &[f64],
    scans: &[&ValidationSample],
) -> RunRequirementResult {
    let Some(reference_samples) = reference_values(reference, scans, values.len()) else {
        return inconclusive(labels, Some(maximum), texts::incomplete_coverage());
    };
    let errors: Vec<f64> = values
        .iter()
        .zip(&reference_samples)
        .map(|(observed, expected)| observed - expected)
        .collect();
    let measured = match statistic {
        ErrorStatistic::Maximum => measure::max_abs(&errors),
        ErrorStatistic::Mean => measure::mean_abs(&errors),
        ErrorStatistic::Rms => measure::rms(&errors),
        ErrorStatistic::MeanError => measure::signed_mean(&errors),
    };
    let value = measured.unwrap_or(f64::NAN);
    match statistic {
        // The signed mean is a bias: its magnitude is compared with the maximum (N8.3, "Mean error").
        ErrorStatistic::MeanError => judge_compared(labels, value, value.abs(), maximum),
        ErrorStatistic::Maximum | ErrorStatistic::Mean | ErrorStatistic::Rms => {
            judge(labels, value, maximum)
        }
    }
}

/// The arrival time of a settling requirement: the first sample of the final run inside the band, when the hold fits in the
/// window from that sample to the window's end. This is the `TypeScript` reading: when the response leaves the band and comes
/// back, it arrives at the final entry, not at the first one.
fn settling_arrival(
    grid: &[f64],
    values: &[f64],
    target: f64,
    band: f64,
    window_end: f64,
    hold: f64,
) -> Option<f64> {
    measure::final_in_band_start(values, target, band)
        .and_then(|index| grid.get(index).copied())
        .filter(|&time| window_end - time >= hold)
}

/// The verdict of a dwell requirement: the shortest completed stay in `state` must last at least `minimum`. The comparison
/// tolerates `1e-9` seconds, scaled by the larger of one and the minimum, as the `TypeScript` does.
fn dwell_verdict(
    labels: &Labels,
    intervals: &[f64],
    state: DwellState,
    minimum: f64,
) -> RunRequirementResult {
    if intervals.is_empty() {
        let word = match state {
            DwellState::High => "high",
            DwellState::Low => "low",
        };
        return inconclusive(labels, Some(minimum), texts::no_dwell_observed(word));
    }
    let shortest = smallest(intervals);
    let (status, reason) = if shortest + 1e-9 * 1.0_f64.max(minimum) >= minimum {
        (Pass, texts::dwell_meets_minimum())
    } else {
        (Fail, texts::dwell_below_minimum())
    };
    finish(labels, status, Some(shortest), Some(minimum), reason)
}

/// The verdict of an activity requirement: its total movement, compared with the maximum when there is one. Without a
/// maximum, a finite movement is reported as a Pass.
fn activity_verdict(labels: &Labels, movement: f64, maximum: Option<f64>) -> RunRequirementResult {
    match maximum {
        Some(limit) => judge(labels, movement, limit),
        None if movement.is_finite() => finish(
            labels,
            Pass,
            Some(movement),
            None,
            texts::movement_measured(),
        ),
        None => finish(
            labels,
            Inconclusive,
            Some(movement),
            None,
            texts::measurement_nonfinite(),
        ),
    }
}

/// The fields that every verdict of one requirement carries (the `base` of `runValidation.ts`).
struct Labels {
    id: String,
    kind: &'static str,
    unit: String,
}

impl Labels {
    fn new(requirement: &RunRequirement, kind: &'static str, unit: String) -> Self {
        Self {
            id: requirement.id.clone(),
            kind,
            unit,
        }
    }
}

/// A verdict with the requirement's labels.
fn finish(
    labels: &Labels,
    status: RequirementStatus,
    value: Option<f64>,
    limit: Option<f64>,
    reason: String,
) -> RunRequirementResult {
    RunRequirementResult {
        id: labels.id.clone(),
        kind: labels.kind.to_owned(),
        status,
        value,
        limit,
        unit: labels.unit.clone(),
        reason,
    }
}

/// An Inconclusive verdict that has no measured value.
fn inconclusive(labels: &Labels, limit: Option<f64>, reason: String) -> RunRequirementResult {
    finish(labels, Inconclusive, None, limit, reason)
}

/// The verdict of a value against its limit: Pass at or below the limit, Fail above it, and Inconclusive when the value is not
/// finite.
fn judge(labels: &Labels, value: f64, limit: f64) -> RunRequirementResult {
    judge_compared(labels, value, value, limit)
}

/// The verdict of a value whose `compared` quantity is tested against the limit. The value is reported as it is, so a signed
/// value keeps its sign. Inconclusive when the compared quantity is not finite.
fn judge_compared(labels: &Labels, value: f64, compared: f64, limit: f64) -> RunRequirementResult {
    if !compared.is_finite() {
        return inconclusive(labels, Some(limit), texts::measurement_nonfinite());
    }
    if compared <= limit {
        finish(
            labels,
            Pass,
            Some(value),
            Some(limit),
            texts::measurement_within_limit(),
        )
    } else {
        finish(
            labels,
            Fail,
            Some(value),
            Some(limit),
            texts::measurement_exceeds_limit(),
        )
    }
}

/// The wire kind of a requirement, in the `TypeScript` words.
fn kind_word(check: &RunCheck) -> &'static str {
    match check {
        RunCheck::Error { .. } => "error",
        RunCheck::Overshoot { .. } => "overshoot",
        RunCheck::Settling { .. } => "settling",
        RunCheck::Bounds { .. } => "bounds",
        RunCheck::StateBounds { .. } => "state-bounds",
        RunCheck::Levels { .. } => "levels",
        RunCheck::Dwell { .. } => "dwell",
        RunCheck::Rate { .. } => "rate",
        RunCheck::Saturation { .. } => "saturation",
        RunCheck::Switching { .. } => "switching",
        RunCheck::Activity { .. } => "activity",
    }
}

/// The unit of a requirement's result (`evaluateObservedRequirement`): seconds for a settling, saturation or dwell, a count for
/// a switching requirement, percentage points for an activity of a percentage channel, and a rate per second.
fn result_unit(check: &RunCheck, channel_unit: &str) -> String {
    let unit = match check {
        RunCheck::Settling { .. } | RunCheck::Saturation { .. } | RunCheck::Dwell { .. } => "s",
        RunCheck::Switching { .. } => "count",
        RunCheck::Activity { .. } if channel_unit == "%" => "pp",
        _ => channel_unit,
    };
    if matches!(check, RunCheck::Rate { .. }) {
        format!("{unit}/s")
    } else {
        unit.to_owned()
    }
}

/// The limit that a requirement's value is compared with (`declaredLimit`): the deadline of a settling, the longest saturation
/// of a saturation, the tolerance of a levels requirement, the minimum of a dwell, zero for bounds, and the maximum of the
/// others. An activity without a maximum has no limit.
fn declared_limit(check: &RunCheck) -> Option<f64> {
    match check {
        RunCheck::Error { maximum, .. }
        | RunCheck::Overshoot { maximum, .. }
        | RunCheck::Rate { maximum, .. } => Some(*maximum),
        RunCheck::Settling {
            deadline_seconds, ..
        } => Some(*deadline_seconds),
        RunCheck::Bounds { .. } | RunCheck::StateBounds { .. } => Some(0.0),
        RunCheck::Levels { tolerance, .. } => Some(*tolerance),
        RunCheck::Dwell {
            minimum_seconds, ..
        } => Some(*minimum_seconds),
        RunCheck::Saturation {
            maximum_seconds, ..
        } => Some(*maximum_seconds),
        RunCheck::Switching { maximum, .. } => Some(count_limit(*maximum)),
        RunCheck::Activity { maximum } => *maximum,
    }
}

/// A switching limit as the number that a switching count is compared with. `check_requirement` keeps the limit a safe integer,
/// so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_limit(maximum: u64) -> f64 {
    maximum as f64
}

/// The declared sample times of the run that lie within the window, inclusive (N8.4).
fn grid_times(run: &ValidationRun, window: Window) -> Vec<f64> {
    run.conditions
        .sample_times_seconds
        .iter()
        .copied()
        .filter(|time| (window.start..=window.end).contains(time))
        .collect()
}

/// The channels that a requirement needs at every sample: its own channel, and a reference channel of an error requirement.
fn needed_channels(requirement: &RunRequirement) -> Vec<&str> {
    let mut needed = vec![requirement.channel.as_str()];
    if let RunCheck::Error {
        reference: ErrorReference::Channel { channel },
        ..
    } = &requirement.check
    {
        needed.push(channel.as_str());
    }
    needed
}

/// The recorded scans that hold the declared sample times of a window (`heldOnGrid`, N8.4). The scans must reach both ends of the
/// window, be ordered, consecutive and good on every needed channel, and leave no interval longer than one and a half grid
/// steps. A run recorded at a record period (`averaged`) takes its first and last scans as covering the window's ends when they
/// lie beyond them (E4.4). Each grid time holds the latest scan at or before it.
fn held_on_grid<'a>(
    run: &'a ValidationRun,
    grid: &[f64],
    needed: &[&str],
    window: Window,
    averaged: bool,
) -> Result<Vec<&'a ValidationSample>, String> {
    let scans = run.samples.as_slice();
    let mut lower = scans
        .iter()
        .take_while(|scan| scan.time_seconds <= window.start + TIME_TOLERANCE)
        .count()
        .checked_sub(1);
    let mut upper = scans
        .iter()
        .position(|scan| scan.time_seconds >= window.end - TIME_TOLERANCE);
    if averaged && !scans.is_empty() {
        lower = lower.or(Some(0));
        upper = upper.or(Some(scans.len() - 1));
    }
    let (Some(lower), Some(upper)) = (lower, upper) else {
        return Err(texts::incomplete_coverage());
    };
    // Scans that end before they start cannot cover the window. The `TypeScript` would read the first of them unchecked.
    if grid.is_empty() || lower > upper {
        return Err(texts::incomplete_coverage());
    }
    let span = &scans[lower..=upper];
    let sequences: Vec<u64> = span.iter().map(|scan| scan.sequence).collect();
    let ordered = span
        .windows(2)
        .all(|pair| pair[1].time_seconds > pair[0].time_seconds);
    let complete = span
        .iter()
        .all(|scan| needed.iter().all(|channel| is_good_value(scan, channel)));
    if !ordered || !complete || !steady_record_period(&sequences, upper == scans.len() - 1) {
        return Err(texts::incomplete_coverage());
    }
    let times: Vec<f64> = span.iter().map(|scan| scan.time_seconds).collect();
    if let Some(gap) = first_gap(&times, median_spacing(grid)) {
        return Err(texts::recording_gap(gap));
    }
    let mut held = lower;
    let mut held_scans = Vec::with_capacity(grid.len());
    for &time in grid {
        while held < upper && scans[held + 1].time_seconds <= time + TIME_TOLERANCE {
            held += 1;
        }
        held_scans.push(&scans[held]);
    }
    Ok(held_scans)
}

/// Whether a scan holds a good, finite value on a channel.
fn is_good_value(scan: &ValidationSample, channel: &str) -> bool {
    scan.values.get(channel).is_some_and(|value| {
        value.quality == ChannelQuality::Good && value.value.is_some_and(f64::is_finite)
    })
}

/// The values of a channel at each held scan, when every scan holds one.
fn values_of(scans: &[&ValidationSample], channel: &str) -> Option<Vec<f64>> {
    scans
        .iter()
        .map(|scan| scan.values.get(channel).and_then(|value| value.value))
        .collect()
}

/// The reference values of an error requirement at each held scan: the constant, repeated, or the reference channel.
fn reference_values(
    reference: &ErrorReference,
    scans: &[&ValidationSample],
    count: usize,
) -> Option<Vec<f64>> {
    match reference {
        ErrorReference::Constant(level) => Some(vec![*level; count]),
        ErrorReference::Channel { channel } => values_of(scans, channel),
    }
}

/// Whether a requirement's window starts and ends on the declared sample grid (N8.3). A run recorded at a record period keeps the
/// `TypeScript` reading, because its samples sit at mean times (E4.4), so its window ends need not be sample times.
fn window_on_grid(requirement: &RunRequirement, run: &ValidationRun, averaged: bool) -> bool {
    averaged
        || [requirement.window.start, requirement.window.end]
            .into_iter()
            .all(|edge| {
                run.conditions
                    .sample_times_seconds
                    .iter()
                    .any(|time| (time - edge).abs() <= TIME_TOLERANCE)
            })
}

/// Whether two requirements share an identifier.
fn has_duplicate_ids(requirements: &[RunRequirement]) -> bool {
    let identities: BTreeSet<&str> = requirements
        .iter()
        .map(|requirement| requirement.id.as_str())
        .collect();
    identities.len() != requirements.len()
}

/// The recorded-run contract (`decodeRun`): the identity and the parameters first, then the experiment conditions, then the
/// channels and the samples. The first rule that fails gives its refusal.
fn check_run(run: &ValidationRun, averaged: bool) -> Result<(), Failure> {
    if !is_text(&run.id) || !named_numbers_valid(&run.parameters) {
        return Err(contract_failure(&texts::invalid_recorded_run()));
    }
    if !conditions_valid(&run.conditions, averaged) {
        return Err(contract_failure(&texts::invalid_experiment_conditions()));
    }
    if !channels_and_samples_valid(run) {
        return Err(contract_failure(&texts::invalid_run_channels()));
    }
    Ok(())
}

/// Whether the experiment conditions of a run are valid (`decodeRun`): the initial state holds finite values, the duration is
/// positive, the revisions hold text, the sample times lie in strictly increasing order and the events fall within the run. A
/// run recorded at a record period (`averaged`) has samples at mean times, so its first sample may start after 0 and its last
/// may end before the duration. Any other run starts at 0 and ends at its duration.
fn conditions_valid(conditions: &RunConditions, averaged: bool) -> bool {
    let times = conditions.sample_times_seconds.as_slice();
    let duration = conditions.duration_seconds;
    let (Some(&first), Some(&last)) = (times.first(), times.last()) else {
        return false;
    };
    let span_valid = if averaged {
        first.is_finite() && first >= 0.0 && last <= duration + TIME_TOLERANCE
    } else {
        exactly_equal(first, 0.0) && exactly_equal(last, duration)
    };
    named_numbers_valid(&conditions.initial_state)
        && duration.is_finite()
        && duration > 0.0
        && is_text(&conditions.model_revision)
        && is_text(&conditions.program_revision)
        && is_text(&conditions.parameter_schema_revision)
        && times.len() >= 2
        && within_count(times.len(), ISOLATED_SIMULATION_SAMPLE_LIMIT)
        && span_valid
        && times.iter().all(|time| time.is_finite() && *time >= 0.0)
        && times.windows(2).all(|pair| pair[1] > pair[0])
        && conditions.events.iter().all(|event| {
            (0.0..=duration).contains(&event.at_seconds)
                && is_text(&event.channel)
                && event.value.is_finite()
        })
}

/// Whether the channels and samples of a run are well formed and within the runtime's limits (`decodeRun`): one to 64 channels
/// with text identities and units and no repeated identity, and at most 100,000 samples with finite times and safe-integer
/// sequences.
fn channels_and_samples_valid(run: &ValidationRun) -> bool {
    let identities: BTreeSet<&str> = run
        .channels
        .iter()
        .map(|channel| channel.id.as_str())
        .collect();
    !run.channels.is_empty()
        && within_count(run.channels.len(), ISOLATED_SIMULATION_CHANNEL_LIMIT)
        && run
            .channels
            .iter()
            .all(|channel| is_text(&channel.id) && is_text(&channel.unit))
        && identities.len() == run.channels.len()
        && within_count(run.samples.len(), ISOLATED_SIMULATION_SAMPLE_LIMIT)
        && run
            .samples
            .iter()
            .all(|sample| sample.time_seconds.is_finite() && sample.sequence <= MAX_SAFE_INTEGER)
}

/// Whether the requirement's identifier and channel are valid, and its window lies within the run: it starts at or after 0, starts
/// before it ends, and ends no later than the duration (`decodeRunRequirement`).
fn window_and_channel_valid(requirement: &RunRequirement, run: &ValidationRun) -> bool {
    let window = requirement.window;
    is_text(&requirement.id)
        && is_text(&requirement.channel)
        && run
            .channels
            .iter()
            .any(|channel| channel.id == requirement.channel)
        && window.end.is_finite()
        && (0.0..window.end).contains(&window.start)
        && window.end <= run.conditions.duration_seconds + TIME_TOLERANCE
}

/// Whether the calculation and its limits are valid for the requirement's kind (`decodeRunRequirement`).
fn limits_valid(requirement: &RunRequirement, run: &ValidationRun) -> bool {
    let length = requirement.window.end - requirement.window.start;
    match &requirement.check {
        RunCheck::Error {
            reference, maximum, ..
        } => nonnegative(*maximum) && reference_valid(reference, &requirement.channel, run),
        RunCheck::Overshoot {
            initial,
            target,
            maximum,
        } => {
            initial.is_finite()
                && target.is_finite()
                && !exactly_equal(*initial, *target)
                && nonnegative(*maximum)
        }
        RunCheck::Settling {
            target,
            band,
            hold_seconds,
            deadline_seconds,
        } => {
            target.is_finite()
                && nonnegative(*band)
                && hold_seconds.is_finite()
                && *hold_seconds > 0.0
                && *hold_seconds <= length
                && nonnegative(*deadline_seconds)
                && *deadline_seconds + *hold_seconds <= length
        }
        RunCheck::Bounds { minimum, maximum } | RunCheck::StateBounds { minimum, maximum } => {
            minimum.is_finite() && maximum.is_finite() && minimum <= maximum
        }
        RunCheck::Levels { allowed, tolerance } => {
            !allowed.is_empty()
                && allowed.len() <= MAX_LEVELS
                && allowed.iter().all(|level| level.is_finite())
                && nonnegative(*tolerance)
        }
        RunCheck::Dwell {
            threshold,
            minimum_seconds,
            ..
        } => threshold.is_finite() && nonnegative(*minimum_seconds),
        RunCheck::Rate { maximum, .. } => nonnegative(*maximum),
        RunCheck::Saturation {
            minimum,
            maximum,
            maximum_seconds,
        } => {
            minimum.is_finite()
                && maximum.is_finite()
                && minimum < maximum
                && nonnegative(*maximum_seconds)
        }
        RunCheck::Switching { threshold, maximum } => {
            nonnegative(*threshold) && *maximum <= MAX_SAFE_INTEGER
        }
        RunCheck::Activity { maximum } => (*maximum).is_none_or(nonnegative),
    }
}

/// Whether an error reference is valid: a finite constant, or a channel of the run whose unit is the requirement's own unit.
fn reference_valid(reference: &ErrorReference, channel: &str, run: &ValidationRun) -> bool {
    match reference {
        ErrorReference::Constant(level) => level.is_finite(),
        ErrorReference::Channel {
            channel: reference_channel,
        } => {
            let own_unit = run
                .channels
                .iter()
                .find(|candidate| candidate.id == channel)
                .map(|candidate| candidate.unit.as_str());
            run.channels.iter().any(|candidate| {
                candidate.id == *reference_channel && Some(candidate.unit.as_str()) == own_unit
            })
        }
    }
}

/// The aggregate status of a set of verdicts (N8.5): Fail when any fails; otherwise Inconclusive when there are none or any is
/// Inconclusive; otherwise Pass.
fn aggregate(statuses: &[RequirementStatus]) -> RequirementStatus {
    if statuses.contains(&Fail) {
        Fail
    } else if statuses.is_empty() || statuses.contains(&Inconclusive) {
        Inconclusive
    } else {
        Pass
    }
}

/// Whether two runs belong to one experiment: the same evidence class, immutable conditions, channels with their units, and
/// parameter identities (N8.6). Parameter values may differ between candidates.
fn same_experiment(current: &ValidationRun, candidate: &ValidationRun) -> bool {
    current.evidence == candidate.evidence
        && current.conditions == candidate.conditions
        && current.channels == candidate.channels
        && current.parameters.keys().eq(candidate.parameters.keys())
}

/// The verdict on the requirement with `id`, when the evaluation holds one.
fn find_result<'a>(
    evaluation: &'a RecordedRunEvaluation,
    id: &str,
) -> Option<&'a RunRequirementResult> {
    evaluation
        .requirements
        .iter()
        .find(|result| result.id == id)
}

/// The refusal of a recorded run that does not match the contract: its data is invalid, and no action of the engineer fixes it
/// from here.
fn contract_failure(text: &str) -> Failure {
    Failure::new(text, Cause::ModelOrData, NextAction::None)
}

/// The refusal of a requirement that the engineer declared wrongly, which the Requirements dialogue corrects.
fn requirement_failure(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenRequirements)
}

/// Whether a text holds something other than white space (`text` in `runValidation.ts`).
fn is_text(value: &str) -> bool {
    !value.trim().is_empty()
}

/// Whether every key holds text and every value is finite (`numbers` in `runValidation.ts`).
fn named_numbers_valid(numbers: &BTreeMap<String, f64>) -> bool {
    numbers
        .iter()
        .all(|(name, value)| is_text(name) && value.is_finite())
}

/// Whether a count is within a limit of the runtime (A7).
fn within_count(count: usize, limit: u64) -> bool {
    u64::try_from(count).is_ok_and(|actual| actual <= limit)
}

/// Whether a number is finite and at least zero.
fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

/// Whether two finite numbers are equal, as the `TypeScript` `===` compares them: their difference is zero. The check is written
/// as a difference, so that the float comparison lint does not apply to a deliberate exact comparison.
fn exactly_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 0.0
}

#[cfg(test)]
#[path = "run_validation_tests.rs"]
mod tests;
