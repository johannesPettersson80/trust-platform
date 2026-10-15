//! The switching (ON/OFF) controller as the commissioning adapters see it (specification 35, C3, A6.4 and B6.1 to B6.4): the candidate's
//! three settings as the gains that an adapter writes, the write set that those settings make through the parameter binding, and the
//! switching limits that the commissioning response is judged by. Apply writes the settings and switches to Auto in one task-boundary
//! transaction; Restore returns the settings and the mode from before it (B6.4). The controller's retained states are declared (C0).

use std::collections::BTreeMap;

use super::adapters::ControllerGains;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, OnOffController, RealType};
use crate::tuning::on_off_candidate::OnOffCandidate;
use crate::tuning::settings_pins::single_precision;
use crate::validation::commissioning_assessment::{CommissioningCriteria, SwitchingCriteria};

/// The refusal of a loop that the switching adapter is asked about without a switching block (C3).
const NO_BLOCK: &str = "The loop declares no switching controller.";

/// The switching controller of a loop, or the refusal that it has none.
fn block_of(loop_definition: &LoopDefinition) -> Result<&OnOffController, Failure> {
    loop_definition
        .controller
        .on_off
        .as_ref()
        .ok_or_else(|| Failure::new(NO_BLOCK, Cause::Implementation, NextAction::OpenLoopSetup))
}

/// The refusal of a setting that the settings do not hold.
fn missing(symbol: &str) -> Failure {
    Failure::new(
        &format!("The switching setting {symbol} has no value."),
        Cause::Settings,
        NextAction::Tune,
    )
}

/// The refusal of a setting that its binding does not hold: outside its bounds, or not representable as its type (C0, A6.4).
fn outside(symbol: &str) -> Failure {
    Failure::new(
        &format!("Switching setting {symbol} is outside its representable binding."),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

/// The gains that an adapter writes for a candidate: its three settings, each under the block's symbol (C3). A setting that the candidate
/// does not hold stays absent, and the adapter refuses it when it writes.
#[must_use]
pub fn candidate_gains(
    loop_definition: &LoopDefinition,
    candidate: &OnOffCandidate,
) -> ControllerGains {
    let Some(block) = loop_definition.controller.on_off.as_ref() else {
        return ControllerGains::default();
    };
    ControllerGains {
        on_off_hysteresis: candidate.values.get(&block.hysteresis_symbol).copied(),
        on_off_minimum_on_seconds: candidate.values.get(&block.minimum_on_time_symbol).copied(),
        on_off_minimum_off_seconds: candidate
            .values
            .get(&block.minimum_off_time_symbol)
            .copied(),
        ..ControllerGains::default()
    }
}

/// The write set of a switching controller's settings (`materialize`, C0, C3): the observed record, with the three settings written under
/// their symbols. Each setting must be given, finite, inside its binding's bounds, and representable as its type, as
/// `materializeBoundParameters` requires.
///
/// # Errors
/// A setting is missing, outside its bounds, or not representable; or the loop has no switching block or no parameter binding.
pub fn materialize(
    loop_definition: &LoopDefinition,
    gains: &ControllerGains,
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let block = block_of(loop_definition)?;
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| Failure::new(NO_BLOCK, Cause::Implementation, NextAction::OpenLoopSetup))?;
    let settings = [
        (block.hysteresis_symbol.as_str(), gains.on_off_hysteresis),
        (
            block.minimum_on_time_symbol.as_str(),
            gains.on_off_minimum_on_seconds,
        ),
        (
            block.minimum_off_time_symbol.as_str(),
            gains.on_off_minimum_off_seconds,
        ),
    ];
    let mut values = observed.clone();
    for (symbol, proposed) in settings {
        let value = proposed.ok_or_else(|| missing(symbol))?;
        let parameter = binding
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .ok_or_else(|| outside(symbol))?;
        let representable = match parameter.iec_type {
            RealType::Real => single_precision(value).to_bits() == value.to_bits(),
            RealType::Lreal => true,
        };
        if !value.is_finite()
            || value < parameter.minimum
            || value > parameter.maximum
            || !representable
        {
            return Err(outside(symbol));
        }
        values.insert(symbol.to_owned(), value);
    }
    Ok(values)
}

/// The switching limits of a run (`switching` of the commissioning criteria, C3): the two output levels of the block, and the minimum ON
/// and OFF times of the reviewed settings. `None` for a loop without a switching block.
///
/// # Errors
/// The reviewed settings hold no value for one of the two minimum times.
pub fn switching_criteria(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
) -> Result<Option<SwitchingCriteria>, Failure> {
    let Some(block) = loop_definition.controller.on_off.as_ref() else {
        return Ok(None);
    };
    let setting = |symbol: &str| values.get(symbol).copied().ok_or_else(|| missing(symbol));
    Ok(Some(SwitchingCriteria {
        low: block.low_output,
        high: block.high_output,
        minimum_on_seconds: setting(&block.minimum_on_time_symbol)?,
        minimum_off_seconds: setting(&block.minimum_off_time_symbol)?,
    }))
}

/// The commissioning criteria of a run with the switching limits set (C3, `pidCommissioning.ts`): the levels and the minimum times that the
/// reviewed settings hold. The criteria of any other loop are kept as they are.
///
/// # Errors
/// As [`switching_criteria`].
pub fn with_switching(
    criteria: &CommissioningCriteria,
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
) -> Result<CommissioningCriteria, Failure> {
    Ok(CommissioningCriteria {
        switching: switching_criteria(loop_definition, values)?,
        ..*criteria
    })
}

#[cfg(test)]
#[path = "on_off_adapter_tests.rs"]
mod tests;
