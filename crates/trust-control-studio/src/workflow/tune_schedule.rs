//! The Tune of a gain schedule in the studio (specification 35, C10 Tune). The table of the schedule is designed from all its operating
//! points by trust-control (`design::gain_schedule::run_schedule`), so this module gathers the other points: each one's loop, its study's
//! current model, its earlier Tune section and the requirements its study holds, read as the own loop's are. A study that is not in the
//! current format, or not of the point's loop revision, is refused by the point's name and is never moved. After the design, the other
//! points' sections are stored, each point's journal and study checked before any write; then `tune_action` stores the own section.

use std::collections::BTreeMap;
use std::path::Path;

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::{GainSchedule, LoopDefinition, SchedulePoint};
use trust_control::study::current_model::{current_model, CurrentModel};
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::design::gain_schedule::{run_schedule, ScheduleMember};
use trust_control::study::tune::{TuneInputs, TuneOutcome, TuneSection};
use trust_control::study::Study;
use trust_control::tuning::requirements::TuningRequirements;

use crate::journal::Journal;
use crate::workflow::tune_search::refuse_while_journaled;
use crate::{loops, storage};

/// The refusal of a loop that is not one of the operating points of its own schedule.
const NOT_A_POINT: &str = "The loop is not one of its schedule's operating points.";
/// The defect of a schedule design whose other points' sections are not in the table's order.
const STORES_OUT_OF_ORDER: &str =
    "The other points' sections do not match the gain schedule's table.";

/// One other operating point of the schedule, read from its loop and its study (C10 Tune), with its index in the table. It is owned, so
/// that the members of the design can borrow it.
struct PointWork {
    /// The point's index in the table.
    index: usize,
    /// The point's name, for the messages.
    name: String,
    /// The point's loop, from its loop file.
    loop_definition: LoopDefinition,
    /// The study's current model of the point, which is current for its loop.
    model: CurrentModel,
    /// The point's Tune section from before, which the design keeps (B4.3).
    previous: Option<TuneSection>,
    /// The requirements the point's study records as given (B4.3).
    held: Option<TuningRequirements>,
}

/// Another point's section, ready to store once every point is checked (C10 Tune).
pub(crate) struct SiblingStore {
    /// The point's name, for the messages.
    name: String,
    /// The point's loop, which names its journal and its study.
    loop_definition: LoopDefinition,
    /// The section that the point's study keeps: the table as its program candidate.
    section: TuneSection,
}

/// What the Tune of a gain schedule gives (C10 Tune): the own point's outcome, which `tune_action` stores, and the other points' sections,
/// which are stored first.
pub(crate) struct ScheduleDesign {
    /// The own point's section and result.
    pub(crate) own: TuneOutcome,
    /// The other points' sections, in the table's order.
    pub(crate) siblings: Vec<SiblingStore>,
}

/// The refusal of a point with no identified model that is current for its loop, by the point's name (C10 Tune, Identify).
fn no_model(name: &str) -> Failure {
    Failure::new(
        &format!("Tune needs an identified model for every point; {name} has none."),
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// The point's study and its current model, read without moving anything (C10 Tune). The study must be in the current format and of the
/// point's loop revision, and it must hold an identified model that is current for the point's loop; otherwise the point is refused by its
/// name.
///
/// # Errors
/// The study cannot be read, or the point has no current identified model.
fn point_study(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    name: &str,
) -> Result<(Study, CurrentModel), Failure> {
    let study =
        storage::current_study(project_root, &loop_definition.id, &loop_definition.revision)?
            .ok_or_else(|| no_model(name))?;
    let model = current_model(&study)?
        .filter(|current| current.is_current_for(loop_definition))
        .ok_or_else(|| no_model(name))?;
    Ok((study, model))
}

/// Reads an other point's loop and its study's sections (B4.3, B4.7, C10 Tune).
///
/// # Errors
/// The loop or its study cannot be read, or the point has no current identified model.
fn read_point(
    project_root: &Path,
    runtime_id: &str,
    index: usize,
    point: &SchedulePoint,
) -> Result<PointWork, Failure> {
    let loop_definition = loops::open(project_root, runtime_id, &point.loop_id)?;
    let (study, model) = point_study(project_root, &loop_definition, &point.name)?;
    let inputs = study.section::<InputsSection>()?;
    Ok(PointWork {
        index,
        name: point.name.clone(),
        previous: study.section::<TuneSection>()?,
        held: inputs.and_then(|inputs| inputs.requirements),
        model,
        loop_definition,
    })
}

/// The members of the table in its order (C10 Tune): each other point from its read, and the own point from the own inputs, which are the
/// same for the own point's requirements and running settings.
fn members_of<'a>(
    count: usize,
    works: &'a [PointWork],
    own: &TuneInputs<'a>,
    held: Option<&'a TuningRequirements>,
    running: Option<&'a BTreeMap<String, f64>>,
) -> Vec<ScheduleMember<'a>> {
    (0..count)
        .map(
            |index| match works.iter().find(|work| work.index == index) {
                Some(work) => ScheduleMember {
                    inputs: TuneInputs {
                        loop_definition: &work.loop_definition,
                        model: Some(&work.model),
                        previous: work.previous.as_ref(),
                        request: own.request,
                        relay: None,
                    },
                    held: work.held.as_ref(),
                    running: None,
                },
                None => ScheduleMember {
                    inputs: *own,
                    held,
                    running,
                },
            },
        )
        .collect()
}

/// The Tune of a gain schedule (C10 Tune): the own point is designed with the own study's requirements and running settings, and every other
/// point is gathered from its loop and its study. The other points take the request of the own loop, as the page sent it with the own
/// study's declared response, so one schedule is designed to one response.
///
/// # Errors
/// The own loop is not one of its schedule's points; a point's loop or study cannot be read; a point has no current identified model; or the
/// design refuses (see `run_schedule`).
pub(crate) fn run(
    project_root: &Path,
    schedule: &GainSchedule,
    own: &TuneInputs<'_>,
    held: Option<&TuningRequirements>,
    running: Option<&BTreeMap<String, f64>>,
) -> Result<ScheduleDesign, Failure> {
    let own_index = schedule
        .points
        .iter()
        .position(|point| point.name == schedule.point)
        .ok_or_else(|| Failure::new(NOT_A_POINT, Cause::Settings, NextAction::OpenLoopSetup))?;
    let works = schedule
        .points
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != own_index)
        .map(|(index, point)| read_point(project_root, &own.request.runtime_id, index, point))
        .collect::<Result<Vec<PointWork>, Failure>>()?;
    let outcome = {
        let members = members_of(schedule.points.len(), &works, own, held, running);
        run_schedule(&members, own_index)?
    };
    let mut siblings = Vec::with_capacity(works.len());
    for (work, sibling) in works.into_iter().zip(outcome.siblings) {
        if work.index != sibling.index {
            return Err(Failure::new(
                STORES_OUT_OF_ORDER,
                Cause::Implementation,
                NextAction::None,
            ));
        }
        siblings.push(SiblingStore {
            name: work.name,
            loop_definition: work.loop_definition,
            section: sibling.section,
        });
    }
    Ok(ScheduleDesign {
        own: outcome.own,
        siblings,
    })
}

/// Stores the other points' sections (C10 Tune), before the own section is stored. Every point is checked before the first write: each
/// point's journal must hold no unfinished operation (the check a search makes before it starts, A8.5), and each point's study must still be
/// current (`point_study`). A failed check refuses the Tune with that check's sentence and writes nothing. The writes then go in the table's
/// order, and a write that fails after another one is reported with the points that were stored.
///
/// # Errors
/// A point's journal holds an unfinished operation; a point's study is not current or has no model; or a write fails.
pub(crate) fn store_siblings(
    project_root: &Path,
    storage_folder: &Path,
    siblings: &[SiblingStore],
) -> Result<(), Failure> {
    for sibling in siblings {
        let journal = Journal::open(
            storage_folder,
            &sibling.loop_definition.project_id,
            &sibling.loop_definition.id,
        )?;
        refuse_while_journaled(&journal)?;
    }
    let mut studies = Vec::with_capacity(siblings.len());
    for sibling in siblings {
        studies.push(point_study(project_root, &sibling.loop_definition, &sibling.name)?.0);
    }
    let mut stored: Vec<&str> = Vec::new();
    for (sibling, mut study) in siblings.iter().zip(studies) {
        let written = study
            .set_section(&sibling.section)
            .and_then(|()| storage::save_study(project_root, &study, &[]));
        if let Err(failure) = written {
            return Err(partial_write(&stored, &sibling.name, &failure));
        }
        stored.push(sibling.name.as_str());
    }
    Ok(())
}

/// The failure of a write that stopped at `name` after the points in `stored` were written (C10 Tune): it says which points hold the table,
/// so that the state is not hidden and the engineer Tunes again.
fn partial_write(stored: &[&str], name: &str, failure: &Failure) -> Failure {
    let reason = failure.message.trim_end_matches('.');
    let text = if stored.is_empty() {
        format!("The table could not be stored in {name}: {reason}. Tune again.")
    } else {
        format!(
            "The table is stored in {} but could not be stored in {name}: {reason}. Tune again before applying.",
            stored.join(", ")
        )
    };
    Failure::new(&text, failure.cause, NextAction::Tune)
}

/// The failure of this point's own section write after the other points' sections were stored (C10 Tune): the points that keep the table
/// are named. Without other points, the failure is the write's own.
pub(crate) fn own_write_failure(siblings: &[SiblingStore], failure: Failure) -> Failure {
    if siblings.is_empty() {
        return failure;
    }
    let names: Vec<&str> = siblings
        .iter()
        .map(|sibling| sibling.name.as_str())
        .collect();
    let reason = failure.message.trim_end_matches('.');
    Failure::new(
        &format!(
            "The table is stored in {} but this point's section could not be stored: {reason}. Tune again before applying.",
            names.join(", ")
        ),
        failure.cause,
        NextAction::Tune,
    )
}
