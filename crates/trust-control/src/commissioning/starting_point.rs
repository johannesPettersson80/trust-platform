//! The starting point of an Apply (specification 35, B6.2), from `commissioningStartingPoint.ts`: the plant the candidate was
//! validated at. A plant input that a Manual loop holds must lie within the Auto-entry allowance of that point; a loop in Auto
//! does not compare it, because the running controller moves it. An output is waited for until it rests inside its band, and
//! an integrating output takes its present value as its operating point. An inner cascade loop holds its reference while its
//! outer controller is in Manual.
//!
//! The wait's reads, its process-time clock and its progress display are transport, and the caller performs them. This module
//! decides each step.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::recording_import::display_engineering_unit;
use crate::identification::settle::settle_window_seconds;
use crate::loop_definition::LoopDefinition;
use crate::model::conversion::linear_transfer_model;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::found_mode::FoundMode;
use super::texts;
use super::transfer::{TransferBinding, TransferMember, TransferValue, TransferValueType};

/// The reference hold of an inner cascade loop: its outer controller's mode, and the Manual value that holds the inner
/// setpoint (the cascade rule of `commissioningStartingPoint.ts`). The cascade's decoding is stage V7 and is not ported; its
/// rules take this typed input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceHold {
    /// The outer controller's mode variable.
    pub mode_symbol: String,
    /// The mode value that means Manual.
    pub manual_value: bool,
}

/// The plant inputs that Apply compares with the validated operating point (B6.2). A loop found in Manual holds its plant input
/// as a setting, so the input must already be the validated one. In Auto the plant input is the running controller's output: it
/// is not compared, and the transfer starts from wherever it is (`comparedPlantInputs`).
#[must_use]
pub fn compared_plant_inputs(input_symbols: &[String], found: Option<FoundMode>) -> &[String] {
    if found == Some(FoundMode::Auto) {
        &[]
    } else {
        input_symbols
    }
}

/// Holds an inner cascade loop's reference while its outer controller is in Manual: the transfer also guards the outer mode, so
/// a switch of that mode before the task boundary refuses the transfer (`withReferenceHoldGuard`). A guard that names the same
/// mode, in any case, is not added twice.
#[must_use]
pub fn with_reference_hold_guard(
    mut binding: TransferBinding,
    hold: Option<&ReferenceHold>,
) -> TransferBinding {
    let Some(hold) = hold else {
        return binding;
    };
    let mode = hold.mode_symbol.to_uppercase();
    if binding
        .guards
        .iter()
        .any(|guard| guard.symbol.to_uppercase() == mode)
    {
        return binding;
    }
    binding.guards.push(TransferMember {
        symbol: hold.mode_symbol.clone(),
        iec_type: TransferValueType::Bool,
        unit: "1".into(),
        minimum: 0.0,
        maximum: 1.0,
    });
    binding
}

/// Refuses the Apply of an inner cascade loop, before any wait or write, while its outer controller is not in Manual
/// (`assertReferenceHeld`).
///
/// # Errors
/// The outer controller's mode is not the Manual value that holds the reference.
pub fn assert_reference_held(
    hold: Option<&ReferenceHold>,
    values: &BTreeMap<String, TransferValue>,
) -> Result<(), Failure> {
    let Some(hold) = hold else {
        return Ok(());
    };
    if values
        .get(&hold.mode_symbol)
        .is_some_and(|value| reads_as(*value, hold.manual_value))
    {
        return Ok(());
    }
    Err(Failure::new(
        &texts::start_reference_held(&hold.mode_symbol),
        Cause::Settings,
        NextAction::Validate,
    ))
}

/// Whether a reported mode value is the Manual value, as the `TypeScript`'s `Number(value) === Number(manualValue)` reads it: a
/// BOOL is 1 or 0, and a number must be exactly that.
#[allow(clippy::float_cmp)] // The runtime's value is compared exactly, as the `TypeScript` compares it.
fn reads_as(value: TransferValue, manual_value: bool) -> bool {
    match value {
        TransferValue::Number(number) => number == if manual_value { 1.0 } else { 0.0 },
        TransferValue::Boolean(flag) => flag == manual_value,
    }
}

/// The validated point an Apply starts from, and the plant it is checked against (B6.2): the loop, the model whose operating
/// point is the validated one, the requirements (their settling band), the plant inputs and the measured outputs by symbol,
/// and the Auto-entry allowance of the inputs (`inputTolerance`, A6.4).
#[derive(Debug, Clone, Copy)]
pub struct StartingPoint<'a> {
    /// The loop being applied.
    pub loop_definition: &'a LoopDefinition,
    /// The model whose operating point is the validated one.
    pub model: &'a GeneralModel,
    /// The requirements of the study: the settling band and the test duration.
    pub task: &'a EngineerRequirements,
    /// The plant inputs that are compared (see [`compared_plant_inputs`]).
    pub input_symbols: &'a [String],
    /// The measured outputs, which are waited for until they settle.
    pub output_symbols: &'a [String],
    /// The allowance of a plant input's deviation from its point.
    pub input_tolerance: f64,
}

/// One plant input or measured output against its validated point (`StartingPointRow`).
#[derive(Debug, Clone, PartialEq)]
struct PlantRow {
    symbol: String,
    observed: f64,
    expected: f64,
    tolerance: f64,
    input: bool,
    /// An integrating output rests wherever it is (B6.2), so it has no band to hold.
    rests: bool,
    settled: bool,
}

/// What the wait for a starting point does next (`awaitCommissioningStartingPoint`, the decision part).
#[derive(Debug, Clone, PartialEq)]
pub enum WaitStep {
    /// Every output that does not rest has stayed inside its band for the settle dwell: the wait ends, and Apply continues from the
    /// reading it has. Nothing is written while waiting.
    Settled,
    /// An output is outside its band, or has not yet stayed inside its band for the settle dwell: its progress is shown, and the next
    /// reading follows after a pause.
    Waiting {
        /// Each measured output with its validated and its current value.
        progress: String,
        /// The process time since which every output that does not rest has been inside its band without a break, or `None` while
        /// one is outside. The next step is handed it.
        inside_since_ms: Option<f64>,
    },
}

impl StartingPoint<'_> {
    /// Checks every plant input and measured output against its validated point, and refuses Apply at the first one that is not
    /// settled (`assertCommissioningStartingPoint`). A plant input is refused at once; an output is refused until it settles.
    ///
    /// # Errors
    /// A plant input is away from its point, an output has not settled, or a value is not a finite number.
    pub fn assert_settled(&self, values: &BTreeMap<String, TransferValue>) -> Result<(), Failure> {
        match self.rows(values)?.into_iter().find(|row| !row.settled) {
            Some(row) => Err(self.unsettled(&row)),
            None => Ok(()),
        }
    }

    /// One step of the wait for the starting point (`awaitCommissioningStartingPoint`, the decision part). The caller reads the
    /// runtime, measures `process_ms` in process time (B6.2), passes the bound of [`wait_bound_ms`] and hands back the
    /// `inside_since_ms` of the step before (`None` at the first reading). A plant input that is away from its point is refused at
    /// once. The wait ends once every output that does not rest has stayed inside its band for [`settle_dwell_ms`]; until then an
    /// output outside its band, or not yet held for the dwell, is waited for, and the wait ends with a failure once the process-time
    /// bound has passed.
    ///
    /// # Errors
    /// A plant input is away from its point; an output has not settled when the bound has passed; or a value is not a finite
    /// number.
    pub fn wait_step(
        &self,
        values: &BTreeMap<String, TransferValue>,
        process_ms: f64,
        bound_ms: f64,
        inside_since_ms: Option<f64>,
    ) -> Result<WaitStep, Failure> {
        let plant_rows = self.rows(values)?;
        if let Some(refused) = plant_rows.iter().find(|row| row.input && !row.settled) {
            return Err(self.unsettled(refused));
        }
        let outputs: Vec<&PlantRow> = plant_rows.iter().filter(|row| !row.input).collect();
        let held: Vec<&PlantRow> = outputs.iter().copied().filter(|row| !row.rests).collect();
        let Some(&first) = held.first() else {
            return Ok(WaitStep::Settled);
        };
        // While every held output is inside its band, the dwell runs from the first such reading; a reading that leaves a band
        // clears it, so the next run of readings starts the dwell afresh.
        let inside_since = held
            .iter()
            .all(|row| row.settled)
            .then(|| inside_since_ms.unwrap_or(process_ms));
        let dwell_ms = settle_dwell_ms(self.loop_definition, self.task);
        if inside_since.is_some_and(|since| process_ms - since >= dwell_ms) {
            return Ok(WaitStep::Settled);
        }
        if process_ms > bound_ms {
            let waiting = held
                .iter()
                .copied()
                .find(|row| !row.settled)
                .unwrap_or(first);
            return Err(self.settle_timeout(waiting, bound_ms));
        }
        Ok(WaitStep::Waiting {
            progress: self.progress(&outputs),
            inside_since_ms: inside_since,
        })
    }

    /// Every plant input, then every measured output, against its validated point (`startingPointRows`).
    fn rows(&self, values: &BTreeMap<String, TransferValue>) -> Result<Vec<PlantRow>, Failure> {
        let mut rows = Vec::with_capacity(self.input_symbols.len() + self.output_symbols.len());
        for (index, symbol) in self.input_symbols.iter().enumerate() {
            let expected = operating_input(self.model, index);
            rows.push(row(
                values,
                symbol,
                expected,
                self.input_tolerance,
                true,
                false,
            )?);
        }
        // An integrating output rests wherever its inflow balances: only the plant input decides whether it is at rest.
        for (index, symbol) in self.output_symbols.iter().enumerate() {
            let expected = operating_output(self.model, index);
            let rests = integrating_output(self.model, index);
            rows.push(row(
                values,
                symbol,
                expected,
                self.task.settling_band,
                false,
                rests,
            )?);
        }
        Ok(rows)
    }

    /// The refusal of an unsettled plant input or output, with the values involved (`unsettledError`, A11.1).
    fn unsettled(&self, row: &PlantRow) -> Failure {
        let unit = self.unit(&row.symbol);
        if row.input {
            Failure::new(
                &texts::start_input_away(
                    &row.symbol,
                    row.observed,
                    row.expected,
                    row.tolerance,
                    &unit,
                ),
                Cause::Settings,
                NextAction::None,
            )
        } else {
            Failure::new(
                &texts::start_output_unsettled(
                    &row.symbol,
                    row.observed,
                    row.expected,
                    row.tolerance,
                    &unit,
                ),
                Cause::Settings,
                NextAction::Wait,
            )
        }
    }

    /// The refusal of an output that did not settle within the process-time bound (`settleTimeoutError`).
    fn settle_timeout(&self, row: &PlantRow, bound_ms: f64) -> Failure {
        let unit = self.unit(&row.symbol);
        Failure::new(
            &texts::start_settle_timeout(
                &row.symbol,
                row.expected,
                row.tolerance,
                row.observed,
                bound_ms / 1000.0,
                &unit,
            ),
            Cause::Settings,
            NextAction::Validate,
        )
    }

    /// The progress of the wait: each measured output with its validated value and its current one (`progressText`).
    fn progress(&self, outputs: &[&PlantRow]) -> String {
        let lines: Vec<String> = outputs
            .iter()
            .map(|row| {
                let unit = self.unit(&row.symbol);
                if row.settled {
                    texts::start_progress_settled(
                        &row.symbol,
                        row.expected,
                        row.tolerance,
                        row.observed,
                        &unit,
                    )
                } else {
                    texts::start_progress_waiting(
                        &row.symbol,
                        row.expected,
                        row.tolerance,
                        row.observed,
                        &unit,
                    )
                }
            })
            .collect();
        texts::start_progress(&lines.join("; "))
    }

    /// The display unit of a loop signal, which the sentences show after its values (`signalUnit`).
    fn unit(&self, symbol: &str) -> String {
        let signals = &self.loop_definition.signals;
        std::iter::once(&signals.process_variable)
            .chain(std::iter::once(&signals.manipulated_variable))
            .chain(signals.setpoint.as_ref())
            .map(|signal| (signal.symbol.as_str(), signal.unit.as_str()))
            .chain(
                signals
                    .additional
                    .iter()
                    .flatten()
                    .map(|signal| (signal.symbol.as_str(), signal.unit.as_str())),
            )
            .find(|(candidate, _)| *candidate == symbol)
            .map_or_else(String::new, |(_, unit)| display_engineering_unit(unit))
    }
}

/// One row: the finite value the reading holds for `symbol`, against `expected` within `tolerance`. An output that rests where it
/// is (`rests`) is settled wherever it is.
fn row(
    values: &BTreeMap<String, TransferValue>,
    symbol: &str,
    expected: f64,
    tolerance: f64,
    input: bool,
    rests: bool,
) -> Result<PlantRow, Failure> {
    let observed = values
        .get(symbol)
        .copied()
        .and_then(TransferValue::as_number)
        .filter(|observed| observed.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::start_value_missing(symbol),
                Cause::Runtime,
                NextAction::None,
            )
        })?;
    let settled = rests || (observed - expected).abs() <= tolerance;
    Ok(PlantRow {
        symbol: symbol.to_owned(),
        observed,
        expected,
        tolerance,
        input,
        rests,
        settled,
    })
}

/// The validated value of a plant input: the operating point's, or zero when the model has none (`model.operatingPoint?.inputs`).
fn operating_input(model: &GeneralModel, index: usize) -> f64 {
    model
        .identity()
        .operating_point
        .as_ref()
        .and_then(|point| point.inputs.get(index))
        .copied()
        .unwrap_or(0.0)
}

/// The validated value of a measured output: the operating point's, or zero when the model has none.
fn operating_output(model: &GeneralModel, index: usize) -> f64 {
    model
        .identity()
        .operating_point
        .as_ref()
        .and_then(|point| point.outputs.get(index))
        .copied()
        .unwrap_or(0.0)
}

/// Whether a model output integrates its input: a single-input, single-output model whose transfer function has a pole at zero
/// (`integratingOutput`, B6.2). A model that does not convert to a transfer function does not integrate.
fn integrating_output(model: &GeneralModel, index: usize) -> bool {
    let identity = model.identity();
    if index != 0 || identity.inputs.len() != 1 || identity.outputs.len() != 1 {
        return false;
    }
    let Ok(value) = serde_json::to_value(model) else {
        return false;
    };
    let Ok(GeneralModel::TransferFunction { transfer, .. }) = linear_transfer_model(&value) else {
        return false;
    };
    transfer.denominator.len() > 1 && transfer.denominator.last().is_some_and(|last| *last == 0.0)
}

/// The bound of the wait for a starting point, in process time: the longer of 60 s and three times the test duration (B6.2).
#[must_use]
pub fn wait_bound_ms(task: &EngineerRequirements) -> f64 {
    (3000.0 * task.duration_seconds).max(60_000.0)
}

/// The settle dwell of the wait, in ms of process time: how long every output that does not rest must stay inside its band before
/// the wait ends (B6.2). It is the settle window of B3.4 step 4 with the expected settling time as the hold, the longer of twenty
/// samples and a tenth of that time; the loop's task period is the sample spacing.
#[must_use]
pub fn settle_dwell_ms(loop_definition: &LoopDefinition, task: &EngineerRequirements) -> f64 {
    let task_seconds = loop_definition.controller.task_period_ms / 1_000.0;
    1_000.0 * settle_window_seconds(task.settling_time_seconds, task_seconds)
}

/// The refusal of an Apply whose wait the user cancelled: nothing was written (`StartingPointWaitCancelled`).
#[must_use]
pub fn wait_cancelled() -> Failure {
    Failure::new(
        &texts::start_wait_cancelled(),
        Cause::Settings,
        NextAction::Validate,
    )
}

/// Takes each integrating output's present value as its operating point (`anchorIntegratingOutputs`, B6.2). Its model does not
/// depend on the level, so the validated step and the Auto entry start from wherever the process rests. The other outputs keep
/// theirs, and a model without an operating point is returned as it is.
#[must_use]
pub fn anchor_integrating_outputs(
    model: &GeneralModel,
    values: &BTreeMap<String, TransferValue>,
    output_symbols: &[String],
) -> GeneralModel {
    let Some(point) = model.identity().operating_point.as_ref() else {
        return model.clone();
    };
    let outputs: Vec<f64> = point
        .outputs
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let present = output_symbols
                .get(index)
                .and_then(|symbol| values.get(symbol))
                .copied()
                .and_then(TransferValue::as_number)
                .filter(|present| present.is_finite());
            match present {
                Some(present) if integrating_output(model, index) => present,
                _ => *value,
            }
        })
        .collect();
    if outputs == point.outputs {
        model.clone()
    } else {
        model_with_operating_outputs(model, outputs)
    }
}

/// The model with its operating point's outputs replaced, and its dynamics unchanged. A model without an operating point is
/// returned as it is.
#[must_use]
pub fn model_with_operating_outputs(model: &GeneralModel, outputs: Vec<f64>) -> GeneralModel {
    let mut replaced = model.clone();
    let identity = match &mut replaced {
        GeneralModel::TransferFunction { identity, .. }
        | GeneralModel::ContinuousStateSpace { identity, .. }
        | GeneralModel::DiscreteStateSpace { identity, .. } => identity,
    };
    if let Some(point) = identity.operating_point.as_mut() {
        point.outputs = outputs;
    }
    replaced
}

#[cfg(test)]
#[path = "starting_point_tests.rs"]
mod tests;
