//! Identify's check of the scheduling variable when a start begins (specification 35, C10 Identify, D-21). A gain-scheduled loop is
//! identified at its own operating point, so the scheduling variable must read within that point's tolerance. The rule and its
//! sentence are trust-control's (`tuning::structures::gain_schedule`). The value is read from the loop's scan at the start, the scan
//! that the run's sample comes from: the loop's signals include the scheduling variable (`signals::loop_inspection_bindings`). A loop
//! without a schedule has no operating point to check, and reads nothing.

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::{GainSchedule, LoopDefinition};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::InspectionSample;
use trust_control::tuning::structures::gain_schedule::check_scheduling_value;

use crate::identify_adapters::RuntimeSampler;

/// The sentence of a start whose scheduling variable cannot be read (C10 Identify).
const UNREADABLE: &str =
    "The scheduling variable cannot be read, so Identify cannot check the operating point.";

/// The sentence of a schedule whose loop names no operating point of its own.
const NO_OWN_POINT: &str =
    "The loop's schedule names no operating point of its own, so Identify cannot check it.";

/// The schedule of a loop, when it has one (C10).
fn schedule_of(loop_definition: &LoopDefinition) -> Option<&GainSchedule> {
    loop_definition
        .controller
        .project
        .as_ref()?
        .schedule
        .as_ref()
}

/// The scheduling variable's value in `scan`, the loop's scan at the start (C10 Identify, D-21): `None` for a loop without a schedule,
/// when no scan is available, or when the variable's value cannot be used.
#[must_use]
pub fn value_in_scan(
    loop_definition: &LoopDefinition,
    scan: Option<&InspectionSample>,
) -> Option<f64> {
    let schedule = schedule_of(loop_definition)?;
    scan?.usable_symbol(&schedule.scheduling.symbol)
}

/// The check of a start of Identify on `loop_definition` (C10 Identify, D-21). `recorded` is the scheduling variable's value that the
/// runtime gave when the start began, or `None` when no usable value was read. A loop without a schedule gives `None`: it has no
/// operating point to check, so nothing is recorded. A scheduled loop gives the recorded value, which the identification keeps.
///
/// # Errors
/// No value was read; the schedule names no operating point of its own; or the value is outside the tolerance of that point (D-21).
/// The failure refuses the start, which writes nothing.
pub fn scheduling_start(
    loop_definition: &LoopDefinition,
    recorded: Option<f64>,
) -> Result<Option<f64>, Failure> {
    let Some(schedule) = schedule_of(loop_definition) else {
        return Ok(None);
    };
    let point = schedule.own_point().ok_or_else(|| {
        Failure::new(
            NO_OWN_POINT,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })?;
    let Some(value) = recorded else {
        return Err(Failure::new(
            UNREADABLE,
            Cause::Runtime,
            NextAction::ConnectRuntime,
        ));
    };
    check_scheduling_value(value, point, &schedule.scheduling.unit)
        .map_err(|refusal| Failure::new(&refusal.0, Cause::Settings, NextAction::Identify))?;
    Ok(Some(value))
}

/// The start's check of the scheduling variable for a start that takes no review (the run's identify stage, B8): a scheduled loop reads
/// one scan of its signals, and `scheduling_start` checks the value. A loop without a schedule reads nothing.
///
/// # Errors
/// The runtime cannot be reached or refuses the scan, or the value is refused (see [`scheduling_start`]).
pub fn check_start(
    connection: &RuntimeConnection,
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
) -> Result<Option<f64>, Failure> {
    if schedule_of(loop_definition).is_none() {
        return Ok(None);
    }
    let scan = RuntimeSampler::new(connection, loop_definition, source).scan()?;
    scheduling_start(
        loop_definition,
        value_in_scan(loop_definition, scan.as_ref()),
    )
}

#[cfg(test)]
#[path = "identify_schedule_tests.rs"]
mod tests;
