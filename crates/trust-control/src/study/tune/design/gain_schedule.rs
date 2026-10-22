//! The Tune of a gain schedule (specification 35, C10 Tune; D-19): each operating point's PI is Automatic's single-loop design on that
//! point's own model, which `run_single` gives for each point. The fixed design is the first point's PI kept at every point. It is
//! recommended only when it meets every point's requirements on that point's model; otherwise each point keeps its own PI. The table of
//! gains is recorded in each point's candidate, so Apply and Validate of any point write every point's gains.
//!
//! The choice and the table are the rules of `tuning::structures::gain_schedule`. This module reads the points and the fixed design's
//! prediction through the same predictor, design context and outcome that the single-loop answer uses. The studio gathers the points'
//! loops, models and study sections and passes them in the table's order (`workflow::tune_schedule`); nothing here reads a file.

use std::collections::BTreeMap;

use trust_control_math::identify::LowOrderModelKind;

use super::{answer, defect, refused, run_single, Design, Evaluation, LoopSetup, NO_MODEL};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{GainSchedule, LoopDefinition, SchedulePoint, SchedulingVariable};
use crate::messages::{
    CandidateOutcome, ScheduleAnswer, ScheduleAnswerPoint, ScheduleAnswerVariable,
    ScheduleChoiceWord, TuneRequest, TuneWords,
};
use crate::study::tune::model::tuning_model;
use crate::study::tune::prediction::{candidate_outcome, current_prediction, TraceNames};
use crate::study::tune::record::{pinned_settings, record_shaping};
use crate::study::tune::{stage_evidence, TuneInputs, TuneOutcome, TuneSection};
use crate::tuning::candidate::LawCandidate;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::mapped_parameters;
use crate::tuning::shortfall::predicted_outcome;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::gain_schedule::{
    choose_schedule, gains_per_point, schedule_candidate, ScheduleChoice,
};
use crate::tuning::typed_pid::TypedDesign;

/// The refusal of a loop with a schedule that the single-loop design is asked for: its points are designed together (C10 Tune).
pub(crate) const SINGLE_LOOP_REFUSAL: &str =
    "A gain schedule is tuned with all its operating points together.";
/// The refusal of a named method for a schedule: each point's PI is Automatic's design (C10 Tune).
const AUTOMATIC_ONLY: &str = "A gain schedule is designed by Automatic. Choose Automatic.";
/// The refusal of pinned settings for a schedule, which its points do not take (C10 Tune).
const NO_PINS: &str =
    "Pinned settings are not served for a gain schedule. Clear the pins and Tune again.";
/// The refusal of a structure other than PI, which is the structure of a schedule's seed (C10 Setup).
const PI_ONLY: &str = "A gain schedule's points are PI. Choose PI or Automatic.";
/// The reason of the fixed design (C10 Tune, D-19).
const FIXED_REASON: &str = "Fixed design: one PI for every operating point meets its requirements.";
/// The defect of a schedule design whose members do not match the table, in its order and its loops.
const MEMBERS_NOT_TABLE: &str = "The operating points do not match the gain schedule's table.";
/// The defect of a schedule design for a loop without a schedule.
const NO_SCHEDULE: &str = "The gain schedule design has no schedule to design.";
/// The defect of a single-loop design that holds no program candidate.
const NO_POINT_PI: &str =
    "The single-loop design of an operating point holds no program candidate.";
/// The defect of a scheduled design in which the fixed design misses no point.
const NO_MISS: &str = "The scheduled design holds no point that the fixed design misses.";

/// One operating point of a schedule as the design reads it (C10 Tune): its inputs for the single-loop design, which name its loop, its
/// model, its earlier Tune section and the request, the requirements its study holds, and the settings of its running controller when the
/// engine read them.
#[derive(Debug, Clone, Copy)]
pub struct ScheduleMember<'a> {
    /// The point's inputs for the single-loop design (B4.1).
    pub inputs: TuneInputs<'a>,
    /// The requirements that the point's study holds (B4.3).
    pub held: Option<&'a TuningRequirements>,
    /// The settings of the point's running controller, when the engine read them (B4.5).
    pub running: Option<&'a BTreeMap<String, f64>>,
}

/// What the Tune of a gain schedule gives (C10 Tune): the own point's outcome, which the caller stores as for any loop, and the section that
/// each other point stores, so that every point's study holds the same table.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleOutcome {
    /// The own point's section and result.
    pub own: TuneOutcome,
    /// The section of each other point, in the table's order.
    pub siblings: Vec<SiblingSection>,
}

/// The section that another point of a schedule stores (C10 Tune): the point's index in the table and its section, with the table as its
/// program candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct SiblingSection {
    /// The point's index in the table.
    pub index: usize,
    /// The point's section, unapplied.
    pub section: TuneSection,
}

/// Designs the table of a gain schedule for the operating point `own` (C10 Tune, D-19): the answer for that point, with the section that each
/// other point stores, unapplied. `members` holds every point in the order of the schedule's table, and `members[own]` is the point that is
/// tuned.
///
/// # Errors
/// The members do not match the schedule's table; the request names a method, pins, or a structure other than PI; a point's single-loop
/// design refuses (see `run_single`); or a point's model cannot be read.
pub fn run_schedule(
    members: &[ScheduleMember<'_>],
    own: usize,
) -> Result<ScheduleOutcome, Failure> {
    let schedule = checked_schedule(members, own)?;
    let requests = point_requests(members)?;
    let inputs: Vec<TuneInputs<'_>> = members
        .iter()
        .zip(&requests)
        .map(|(member, request)| TuneInputs {
            request,
            ..member.inputs
        })
        .collect();
    let singles = members
        .iter()
        .zip(&inputs)
        .map(|(member, input)| run_single(input, member.held, member.running))
        .collect::<Result<Vec<TuneOutcome>, Failure>>()?;
    let designs = designs_of(&singles)?;
    let fixed = designs
        .first()
        .copied()
        .ok_or_else(|| defect(MEMBERS_NOT_TABLE))?;
    let fixed_meets = inputs
        .iter()
        .zip(&singles)
        .map(|(input, single)| fixed_meets_on_point(input, &single.section.requirements, fixed))
        .collect::<Result<Vec<bool>, Failure>>()?;
    let choice = choose_schedule(&fixed_meets);
    let table = gains_per_point(choice, &designs);
    let reason = decision_reason(choice, &fixed_meets, &schedule.points)?;
    let siblings = singles
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != own)
        .map(|(index, single)| sibling_section(single, &schedule.points, &table, index, &reason))
        .collect::<Result<Vec<SiblingSection>, Failure>>()?;
    let own_single = singles.get(own).ok_or_else(|| defect(MEMBERS_NOT_TABLE))?;
    let source = project_candidate_of(own_single)?.source;
    let candidate = schedule_candidate(&schedule.points, &table, own, source)
        .map_err(refused(Cause::Settings, NextAction::OpenLoopSetup))?;
    let own_input = inputs.get(own).ok_or_else(|| defect(MEMBERS_NOT_TABLE))?;
    let own_running = members.get(own).and_then(|member| member.running);
    let single = singles
        .into_iter()
        .nth(own)
        .ok_or_else(|| defect(MEMBERS_NOT_TABLE))?;
    let own_outcome = own_answer(
        own_input,
        own_running,
        single,
        candidate,
        reason,
        schedule,
        &table,
    )?;
    let mut answered = own_outcome;
    let words = TuneWords::of_result(own_input.loop_definition, &answered.result);
    answered.result.words = Some(words);
    Ok(ScheduleOutcome {
        own: answered,
        siblings,
    })
}

/// The answer of a gain schedule (C10 Tune): one row per point for the results page, in the table's order. The choice comes from the table,
/// not from the decision: `fixed` when every point has the same proportional and integral gain, `scheduled` when they differ. Each row gives
/// the point's name, value, two gain variables and the gains that the table gives it. `reason` is the section's reason, which the answer
/// repeats. The callers pass one gain pair for each point.
#[must_use]
pub(crate) fn schedule_answer(
    points: &[SchedulePoint],
    scheduling: &SchedulingVariable,
    table: &[ParallelGains],
    reason: &str,
) -> ScheduleAnswer {
    let choice = if table.windows(2).all(|pair| same_gains(pair[0], pair[1])) {
        ScheduleChoiceWord::Fixed
    } else {
        ScheduleChoiceWord::Scheduled
    };
    ScheduleAnswer {
        choice,
        reason: reason.to_owned(),
        scheduling: ScheduleAnswerVariable {
            symbol: scheduling.symbol.clone(),
            unit: scheduling.unit.clone(),
        },
        points: points
            .iter()
            .zip(table)
            .map(|(point, gains)| ScheduleAnswerPoint {
                name: point.name.clone(),
                value: point.value,
                proportional_symbol: point.proportional_symbol.clone(),
                integral_symbol: point.integral_symbol.clone(),
                proportional_gain: gains.proportional_gain,
                integral_gain: gains.integral_gain,
            })
            .collect(),
    }
}

/// Whether two gains have the same proportional and integral values, compared bit for bit as the design's own gains are.
fn same_gains(left: ParallelGains, right: ParallelGains) -> bool {
    left.proportional_gain.to_bits() == right.proportional_gain.to_bits()
        && left.integral_gain.to_bits() == right.integral_gain.to_bits()
}

/// The schedule that a loop declares, when it declares one (C10).
#[must_use]
pub fn schedule_of(loop_definition: &LoopDefinition) -> Option<&GainSchedule> {
    loop_definition
        .controller
        .project
        .as_ref()
        .and_then(|project| project.schedule.as_ref())
}

/// The schedule of the point `own`, once the members are checked against it: one member for each point of the table, in its order, and
/// each member's loop is that point's loop (C10 Setup).
fn checked_schedule<'a>(
    members: &[ScheduleMember<'a>],
    own: usize,
) -> Result<&'a GainSchedule, Failure> {
    let schedule = members
        .get(own)
        .and_then(|member| schedule_of(member.inputs.loop_definition))
        .ok_or_else(|| defect(NO_SCHEDULE))?;
    let in_table_order = schedule.points.len() == members.len()
        && schedule
            .points
            .iter()
            .zip(members)
            .all(|(point, member)| point.loop_id == member.inputs.loop_definition.id);
    if in_table_order {
        Ok(schedule)
    } else {
        Err(defect(MEMBERS_NOT_TABLE))
    }
}

/// The request of each point for the schedule (C10 Tune): Automatic, with no pins, designing the PI of the schedule's seed. A request that
/// names another method, pins or another structure is refused, because the table holds one PI for each point.
fn point_requests(members: &[ScheduleMember<'_>]) -> Result<Vec<TuneRequest>, Failure> {
    members
        .iter()
        .map(|member| {
            let request = member.inputs.request;
            if request.method.is_some() {
                return Err(Failure::new(
                    AUTOMATIC_ONLY,
                    Cause::Settings,
                    NextAction::ChooseMethod,
                ));
            }
            if request.pins.is_some() {
                return Err(Failure::new(NO_PINS, Cause::Settings, NextAction::None));
            }
            if request
                .structure
                .is_some_and(|structure| structure != Structure::PI)
            {
                return Err(Failure::new(PI_ONLY, Cause::Settings, NextAction::None));
            }
            Ok(TuneRequest {
                structure: Some(Structure::PI),
                ..request.clone()
            })
        })
        .collect()
}

/// The section that the point `index` stores for the table (C10 Tune): its own single-loop section, with the table as its program candidate
/// (`schedule_candidate`), the decision as its reason, and no record of its own.
fn sibling_section(
    single: &TuneOutcome,
    points: &[SchedulePoint],
    table: &[ParallelGains],
    index: usize,
    reason: &str,
) -> Result<SiblingSection, Failure> {
    let source = project_candidate_of(single)?.source;
    let candidate = schedule_candidate(points, table, index, source)
        .map_err(refused(Cause::Settings, NextAction::OpenLoopSetup))?;
    let mut section = single.section.clone();
    section.candidate = None;
    section.project_candidate = Some(candidate);
    reason.clone_into(&mut section.reason);
    Ok(SiblingSection { index, section })
}

/// The program candidate of a single-loop design: the point's own PI (C2).
fn project_candidate_of(single: &TuneOutcome) -> Result<&ProjectCandidate, Failure> {
    single
        .section
        .project_candidate
        .as_ref()
        .ok_or_else(|| defect(NO_POINT_PI))
}

/// The own PI of each point, from its single-loop design, in the order of the table.
fn designs_of(singles: &[TuneOutcome]) -> Result<Vec<ParallelGains>, Failure> {
    singles
        .iter()
        .map(|single| project_candidate_of(single).map(|candidate| candidate.gains))
        .collect()
}

/// Only a predicted response that misses a requirement fails the fixed design (C10 Tune, D-19). An unpredicted outcome is not a failure, so
/// it does not make the schedule scheduled: the fixed design fails when its predicted response breaks a requirement, and without a prediction
/// Validate decides (N5.12).
fn fails(outcome: &CandidateOutcome) -> bool {
    matches!(outcome, CandidateOutcome::Misses { .. })
}

/// Whether the fixed design is not failing on a point's own model (C10 Tune, D-19): its predicted outcome, through the predictor, context and
/// outcome of the point's single-loop answer, is not a failure (`fails`).
fn fixed_meets_on_point(
    inputs: &TuneInputs<'_>,
    requirements: &TuningRequirements,
    fixed: ParallelGains,
) -> Result<bool, Failure> {
    with_evaluation(inputs, None, requirements, |evaluation| {
        let design = TypedDesign {
            structure: Structure::PI,
            gains: fixed,
        };
        let outcome = candidate_outcome(
            predicted_outcome(&design, evaluation.requirements, evaluation.context),
            evaluation.predictor,
            fixed,
            evaluation.requirements,
        );
        !fails(&outcome)
    })
}

/// Runs `body` on the evaluation of a point's loop on its own model, built as the point's single-loop answer builds it (B4.4, B4.5): the
/// model, the predictor, the design context, the trace names and the prediction of the running settings, with the point's requirements.
fn with_evaluation<T>(
    inputs: &TuneInputs<'_>,
    running: Option<&BTreeMap<String, f64>>,
    requirements: &TuningRequirements,
    body: impl FnOnce(&Evaluation<'_>) -> T,
) -> Result<T, Failure> {
    let loop_definition = inputs.loop_definition;
    let current = inputs.model.ok_or_else(|| defect(NO_MODEL))?;
    let model = tuning_model(current).map_err(refused(Cause::ModelOrData, NextAction::Identify))?;
    let mapped = mapped_parameters(loop_definition);
    let shaping = record_shaping(
        loop_definition,
        &pinned_settings(loop_definition, inputs.pins, &mapped)?,
    );
    let setup = LoopSetup::of(
        loop_definition,
        &current.model,
        model.kind == LowOrderModelKind::Integrating,
    );
    let predictor = setup.predictor(&model, shaping);
    let context = setup.context(&predictor);
    let running_prediction = running
        .and_then(|running| current_prediction(&predictor, loop_definition, &mapped, running));
    let evaluation = Evaluation {
        model: &model,
        requirements,
        predictor: &predictor,
        context: &context,
        names: TraceNames::of(loop_definition),
        parameters: &mapped,
        current: running_prediction,
    };
    Ok(body(&evaluation))
}

/// The reason of the table (C10 Tune, D-19): the fixed design when every point meets its requirements, otherwise the scheduled design,
/// named by the first point that the fixed design misses.
fn decision_reason(
    choice: ScheduleChoice,
    meets: &[bool],
    points: &[SchedulePoint],
) -> Result<String, Failure> {
    match choice {
        ScheduleChoice::Fixed => Ok(FIXED_REASON.to_owned()),
        ScheduleChoice::Scheduled => {
            let missed = meets
                .iter()
                .position(|met| !*met)
                .and_then(|index| points.get(index))
                .ok_or_else(|| defect(NO_MISS))?;
            Ok(format!(
                "Scheduled design: the fixed design fails a requirement at operating point {}; each point has its own PI.",
                missed.name
            ))
        }
    }
}

/// The answer of the point `own` once the table is chosen (C10 Tune, B4.1): its section keeps the table as its program candidate, and the
/// decision as its reason. Its result is the single-loop answer rebuilt on the gains that Apply writes for the point (the candidate's own
/// gains), with the point's own attempts and closed-loop time, and its schedule answer, which `schedule_answer` gives from the table.
fn own_answer(
    own: &TuneInputs<'_>,
    running: Option<&BTreeMap<String, f64>>,
    single: TuneOutcome,
    candidate: ProjectCandidate,
    reason: String,
    schedule: &GainSchedule,
    table: &[ParallelGains],
) -> Result<TuneOutcome, Failure> {
    let TuneOutcome {
        mut section,
        result,
    } = single;
    // A program's law keeps its candidate in `project_candidate` alone (B4.1, C2).
    section.candidate = None;
    section.project_candidate = Some(candidate.clone());
    section.reason.clone_from(&reason);
    let has_recording = own.model.is_some_and(|current| current.identify.is_some());
    let evidence = stage_evidence(
        own.loop_definition,
        own.model,
        Some(&section),
        has_recording,
    );
    let designed = Design {
        source: candidate.source,
        structure: candidate.structure,
        gains: candidate.gains,
        objective: result.objective,
        closed_loop_time_seconds: result.closed_loop_time_seconds,
        attempts: result.attempts,
        reason,
        description: None,
    };
    let mut answered = with_evaluation(own, running, &section.requirements, |evaluation| {
        answer(
            None,
            designed,
            LawCandidate::Project(candidate),
            Vec::new(),
            evidence,
            evaluation,
        )
    })?;
    answered.schedule = Some(schedule_answer(
        &schedule.points,
        &schedule.scheduling,
        table,
        &section.reason,
    ));
    Ok(TuneOutcome {
        section,
        result: answered,
    })
}

#[cfg(test)]
#[path = "gain_schedule_tests.rs"]
mod tests;
