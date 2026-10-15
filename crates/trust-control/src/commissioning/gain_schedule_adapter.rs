//! The Apply and Restore of a gain schedule (specification 35, C10 Apply and Restore; B6.1 and B6.4). One transaction writes the proportional
//! and integral gain of every operating point, each from the gains that the table gives that point, and switches the loop to Auto; Restore
//! returns all of them. This module decides which variables that transaction writes, what Restore writes back, and how a candidate's values
//! read back as the gains of each point. The transaction itself is the transfer of `transfer_protocol`, which takes the map as its values.
//! The table's variables are states of the transfer binding (`with_schedule_members`), and the declared tracking names each one with its
//! value (`schedule_tracking_values`): the runtime refuses a binding whose states its tracking does not name.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Parameter, ParameterBinding, SchedulePoint};
use crate::tuning::candidate::LawCandidate;
use crate::tuning::structure::ParallelGains;
use crate::tuning::structures::gain_schedule::table_values;

use super::found_mode::FoundMode;
use super::transfer::{rounded_transfer_value, TransferBinding, TransferMember, TransferValue};

/// The operating points of a loop's schedule (C10), in the order the setup gave them; `None` for a loop that declares no schedule.
#[must_use]
pub fn schedule_points(loop_definition: &LoopDefinition) -> Option<&[SchedulePoint]> {
    loop_definition
        .controller
        .project
        .as_ref()?
        .schedule
        .as_ref()
        .map(|schedule| schedule.points.as_slice())
}

/// The transfer members of a schedule's table (C10 Apply): the proportional and the integral variable of every point, in table order. The
/// loop's record names the gains of its own point, and every point's variable of a role takes that role's type, unit and bounds.
///
/// # Errors
/// The record holds no parameter that is a proportional or an integral variable of the table.
pub fn schedule_members(
    points: &[SchedulePoint],
    binding: &ParameterBinding,
) -> Result<Vec<TransferMember>, Failure> {
    let proportional_symbols: Vec<&str> = points
        .iter()
        .map(|point| point.proportional_symbol.as_str())
        .collect();
    let integral_symbols: Vec<&str> = points
        .iter()
        .map(|point| point.integral_symbol.as_str())
        .collect();
    let proportional = role_parameter(binding, &proportional_symbols)?;
    let integral = role_parameter(binding, &integral_symbols)?;
    Ok(points
        .iter()
        .flat_map(|point| {
            [
                member(&point.proportional_symbol, proportional),
                member(&point.integral_symbol, integral),
            ]
        })
        .collect())
}

/// The transfer binding of a schedule (C10 Apply and Restore): the table's variables become states after the binding's own, so that the
/// transfer writes them, and the loop's own gains leave the guards that the record read them through. Every other member keeps its place.
///
/// # Errors
/// [`schedule_members`] refuses the record, or the binding already states one of the table's variables.
pub fn with_schedule_members(
    points: &[SchedulePoint],
    binding: &ParameterBinding,
    mut transfer: TransferBinding,
) -> Result<TransferBinding, Failure> {
    let members = schedule_members(points, binding)?;
    let in_table = |symbol: &str| members.iter().any(|member| member.symbol == symbol);
    if let Some(state) = transfer
        .states
        .iter()
        .find(|state| in_table(state.symbol.as_str()))
    {
        return Err(defect(&already_stated(&state.symbol)));
    }
    transfer
        .guards
        .retain(|guard| !in_table(guard.symbol.as_str()));
    transfer.states.extend(members);
    Ok(transfer)
}

/// The initialization values that a declared tracking names for the table's variables (C10 Apply and Restore; annex E6.4). At an Auto
/// entry (`entry` given) each variable takes its value in `entry`; otherwise it takes the middle of its range, because a tracking that does
/// not align the output keeps the value the runtime reads. Each value is rounded as its member is typed.
///
/// # Errors
/// The binding does not state one of the table's variables, `entry` holds no value for one, or a value does not fit its member.
pub fn schedule_tracking_values(
    points: &[SchedulePoint],
    binding: &TransferBinding,
    entry: Option<&BTreeMap<String, f64>>,
) -> Result<BTreeMap<String, f64>, Failure> {
    table_symbols(points)
        .map(|symbol| -> Result<(String, f64), Failure> {
            let member = binding
                .states
                .iter()
                .find(|state| state.symbol == symbol)
                .ok_or_else(|| defect(&unstated_variable(symbol)))?;
            let value = match entry {
                Some(values) => value_of(values, symbol)?,
                None => member.minimum / 2.0 + member.maximum / 2.0,
            };
            Ok((symbol.to_owned(), rounded_transfer_value(member, value)?))
        })
        .collect()
}

/// The values that a schedule's Apply writes (C10 Apply): the proportional and integral gain of every point, from the candidate's values,
/// which hold the gains of the whole table. The candidate's other values are not written.
///
/// # Errors
/// The values do not give one pair of gains for each point, or name a variable twice (see [`gains_of_table`] and [`schedule_values`]).
pub fn schedule_write(
    points: &[SchedulePoint],
    values: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    schedule_values(points, &gains_of_table(points, values)?)
}

/// The values that Restore returns the table's variables to (C10 Restore, B6.4): each variable's value before the Apply, as the runtime
/// reported the transfer's states at the review (`observed`).
///
/// # Errors
/// A variable is not reported as a finite number (see [`restore_values`]).
pub fn schedule_origin(
    points: &[SchedulePoint],
    observed: &BTreeMap<String, TransferValue>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let previous: BTreeMap<String, f64> = table_symbols(points)
        .filter_map(|symbol| {
            observed
                .get(symbol)
                .copied()
                .and_then(TransferValue::as_number)
                .filter(|value| value.is_finite())
                .map(|value| (symbol.to_owned(), value))
        })
        .collect();
    restore_values(points, &previous)
}

/// The variables and values that one schedule transaction writes (C10 Apply): the proportional and integral gain of every point, from the
/// gains that the table gives that point. `gains` holds each point's gains in the order of the table.
///
/// # Errors
/// The gains do not give one pair for each point, or a variable is named by two of the gains.
pub fn schedule_values(
    points: &[SchedulePoint],
    gains: &[ParallelGains],
) -> Result<BTreeMap<String, f64>, Failure> {
    table_values(points, gains).map_err(|refusal| defect(&refusal.0))
}

/// The gains of each point, read back from a schedule candidate's values (C10 Apply and Validate): the inverse of [`schedule_values`].
///
/// # Errors
/// The values hold no value for one of the table's variables.
pub fn gains_of_table(
    points: &[SchedulePoint],
    values: &BTreeMap<String, f64>,
) -> Result<Vec<ParallelGains>, Failure> {
    points
        .iter()
        .map(|point| -> Result<ParallelGains, Failure> {
            Ok(ParallelGains {
                proportional_gain: value_of(values, &point.proportional_symbol)?,
                integral_gain: value_of(values, &point.integral_symbol)?,
                derivative_gain: 0.0,
            })
        })
        .collect()
}

/// The values that Restore writes back (C10 Restore, B6.4): every gain variable of every point, each at the value the controller held
/// before the Apply. Other entries of `previous` are not the schedule's and are not written.
///
/// # Errors
/// The previous value of one of the schedule's variables is not known.
pub fn restore_values(
    points: &[SchedulePoint],
    previous: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    points
        .iter()
        .flat_map(|point| [&point.proportional_symbol, &point.integral_symbol])
        .map(|symbol| {
            previous
                .get(symbol)
                .map(|value| (symbol.clone(), *value))
                .ok_or_else(|| {
                    Failure::new(
                        &previous_unknown(symbol),
                        Cause::Runtime,
                        NextAction::ConnectRuntime,
                    )
                })
        })
        .collect()
}

/// Whether `symbol` is one of the table's variables: the proportional or the integral variable of one of the points (C10).
#[must_use]
pub fn is_table_variable(points: &[SchedulePoint], symbol: &str) -> bool {
    table_symbols(points).any(|variable| variable == symbol)
}

/// The table's write set of a schedule's Apply (C10 Apply), read from the candidate that the validation holds: the gains of every point,
/// from the candidate's values. `None` for a loop without a schedule, whose Apply writes no table.
///
/// # Errors
/// The loop has a schedule and its candidate is not a program's own law's candidate, or its values do not give the table (see
/// [`schedule_write`]).
pub fn schedule_write_of(
    points: Option<&[SchedulePoint]>,
    candidate: &LawCandidate,
) -> Result<Option<BTreeMap<String, f64>>, Failure> {
    let Some(points) = points else {
        return Ok(None);
    };
    match candidate {
        LawCandidate::Project(project) => schedule_write(points, &project.values).map(Some),
        _ => Err(defect(NOT_A_PROGRAM_CANDIDATE)),
    }
}

/// The refusal of a Restore into Manual of a schedule loop (C10 Restore). The table is written at an Auto entry only, and a Restore that
/// returns the loop to Manual makes none, so it would leave the other points' gains as Apply wrote them. `schedule_origin` is the
/// checkpoint's table, `None` for a loop without a schedule, and `found_mode` is the mode Apply found (`None`, a checkpoint without one,
/// stays in Manual).
///
/// # Errors
/// The checkpoint is a schedule's, and the loop was not in Auto when Apply found it.
pub fn check_restore_mode(
    schedule_origin: Option<&BTreeMap<String, f64>>,
    found_mode: Option<FoundMode>,
) -> Result<(), Failure> {
    if schedule_origin.is_some() && found_mode != Some(FoundMode::Auto) {
        return Err(Failure::new(
            RESTORE_INTO_AUTO_ONLY,
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Every variable of the table, the proportional and the integral variable of each point, in table order.
fn table_symbols(points: &[SchedulePoint]) -> impl Iterator<Item = &str> {
    points.iter().flat_map(|point| {
        [
            point.proportional_symbol.as_str(),
            point.integral_symbol.as_str(),
        ]
    })
}

/// The record's parameter for one role of the table: the parameter whose symbol is one of the role's symbols.
fn role_parameter<'a>(
    binding: &'a ParameterBinding,
    symbols: &[&str],
) -> Result<&'a Parameter, Failure> {
    binding
        .parameters
        .iter()
        .find(|parameter| symbols.contains(&parameter.symbol.as_str()))
        .ok_or_else(|| {
            Failure::new(
                &missing_parameter(),
                Cause::Settings,
                NextAction::OpenLoopSetup,
            )
        })
}

/// A table variable as a transfer member, with the type, unit and bounds of the record's parameter for its role.
fn member(symbol: &str, parameter: &Parameter) -> TransferMember {
    TransferMember {
        symbol: symbol.to_owned(),
        iec_type: parameter.iec_type.into(),
        unit: parameter.unit.clone(),
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    }
}

/// The value a candidate holds for one of the table's variables.
///
/// # Errors
/// The values hold no value for the symbol.
fn value_of(values: &BTreeMap<String, f64>, symbol: &str) -> Result<f64, Failure> {
    values
        .get(symbol)
        .copied()
        .ok_or_else(|| defect(&missing_value(symbol)))
}

/// The defect of a schedule whose candidate is not a program's own law's: only that law's candidate holds the table.
const NOT_A_PROGRAM_CANDIDATE: &str =
    "A gain schedule's candidate holds the table of a program's own law, so no other law can be applied from it.";

/// The refusal of a Restore into Manual of a schedule (`check_restore_mode`).
const RESTORE_INTO_AUTO_ONLY: &str = "The gains of a gain schedule can be restored only into Auto.";

/// The refusal of a record that holds no parameter for a variable of the schedule's table.
fn missing_parameter() -> String {
    "The parameter record of the loop holds no gain of the schedule's table.".to_owned()
}

/// The defect of a transfer binding that states none of the table's variables (`with_schedule_members` was not applied).
fn unstated_variable(symbol: &str) -> String {
    format!("The transfer binding does not state {symbol}, a variable of the schedule's table.")
}

/// The defect of a transfer binding that already states one of the table's variables.
fn already_stated(symbol: &str) -> String {
    format!("The transfer binding already states {symbol}, a variable of the schedule's table.")
}

/// The refusal of a restore whose previous value of a variable the schedule writes is not known (C10 Restore).
fn previous_unknown(symbol: &str) -> String {
    format!("The value of {symbol} before Apply is not known, so Restore cannot return it.")
}

/// The defect of a candidate that holds no value for a variable of its table.
fn missing_value(symbol: &str) -> String {
    format!("The schedule's candidate holds no value for {symbol}.")
}

/// A defect of the schedule's Apply, Validate or Restore: the checks before them refuse these, so reaching one is an implementation fault.
fn defect(text: &str) -> Failure {
    Failure::new(text, Cause::Implementation, NextAction::None)
}

#[cfg(test)]
#[path = "gain_schedule_adapter_tests.rs"]
mod tests;
