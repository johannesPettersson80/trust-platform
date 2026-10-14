//! The mode a commissioning found a loop in, and the return to it (specification 35, A6.2), from `foundModeReturn.ts`: an
//! operation that changes a loop's mode records the mode it found, and when the operation ends the loop returns to that mode.
//! The transfer that carries the return is transport; this module decides whether it happens and with which Auto entry.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::transfer::{TransferModeBinding, TransferValue};
use super::transfer_tracking::TransferTracking;

/// The mode a loop was found in (`FoundMode`): `auto` or `manual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FoundMode {
    /// The loop was in Auto.
    Auto,
    /// The loop was in Manual.
    Manual,
}

/// The found mode of a review (`foundModeOf`): the mode a reviewed mode value stands for. `None` when the review did not read
/// the mode, so no return to it is claimed.
#[must_use]
pub fn found_mode_of(reviewed_mode: Option<bool>, manual_value: bool) -> Option<FoundMode> {
    reviewed_mode.map(|mode| {
        if mode == manual_value {
            FoundMode::Manual
        } else {
            FoundMode::Auto
        }
    })
}

/// The mode value that a found mode stands for (`reviewedModeOf`), as a recovery reviewed from Apply's report sets it. `None`
/// when no mode is named.
#[must_use]
pub fn reviewed_mode_of(found: Option<FoundMode>, manual_value: bool) -> Option<bool> {
    found.map(|mode| match mode {
        FoundMode::Manual => manual_value,
        FoundMode::Auto => !manual_value,
    })
}

/// The Auto entry that returns a recovered loop to Auto: the states it writes, its transfer tolerance and its tracking (A6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct AutoReentry {
    /// The states the entry writes, by symbol.
    pub values: BTreeMap<String, f64>,
    /// The allowance of the first output after the entry (A6.4).
    pub tolerance: f64,
    /// The tracking the entry carries.
    pub tracking: TransferTracking,
}

/// The re-entry decision of A6.2 (the decision part of `reenterFoundMode`): once the previous settings are confirmed back, a
/// loop found in Auto re-enters Auto with the entry that `entry` builds from the current reading. A loop found in Manual, or of
/// unknown mode, stays in Manual; so does a loop whose mode the runtime does not report in Manual, and a loop whose entry cannot
/// be built (`entry` returns `None`). `entry` runs only when the loop is found in Auto and reads as Manual.
///
/// Returns the entry to apply, or `None` when the loop stays where it is.
#[must_use]
pub fn reenter_found_mode(
    found: Option<FoundMode>,
    mode: &TransferModeBinding,
    current: &BTreeMap<String, TransferValue>,
    entry: impl FnOnce(&BTreeMap<String, TransferValue>) -> Option<AutoReentry>,
) -> Option<AutoReentry> {
    if found != Some(FoundMode::Auto) {
        return None;
    }
    if current.get(&mode.symbol) != Some(&TransferValue::Boolean(mode.manual_value)) {
        return None;
    }
    entry(current)
}

#[cfg(test)]
#[path = "found_mode_tests.rs"]
mod tests;
