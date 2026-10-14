//! The Routh–Hurwitz state of one declared loop: the value that was calculated, the saved result and the
//! rules that keep a saved result read-only (B10.2, N7.1). A restored result grants no runtime parameter
//! selection, and a result that belongs to another definition is not restored. The message handling and the
//! worker thread of the editor are not here: they are the engine's wiring.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::analysis::declared::{check_declared, DeclaredControlAnalysis};
use crate::analysis::routh::{calculate_declared_routh, DeclaredRouthResult, IntervalKind};
use crate::failure::{Cause, Failure, NextAction};

use crate::analysis::texts;

/// A calculated Routh–Hurwitz result, kept with the definition it was calculated from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedRouthAnalysis {
    /// The parameter value at which the characteristic polynomial was read.
    pub selected_value: f64,
    /// The result of the calculation.
    pub result: DeclaredRouthResult,
    /// The definition that the result was calculated from.
    pub definition: DeclaredControlAnalysis,
}

/// The Routh–Hurwitz state of one declared loop.
#[derive(Debug, Default)]
pub struct RouthSession {
    selected: Option<f64>,
    saved: Option<SavedRouthAnalysis>,
}

impl RouthSession {
    /// A session with nothing calculated.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculates the result at `selected` for `definition` and keeps it. The parameter selection is granted
    /// for this calculation only. A failed calculation keeps the previous result and grants no selection.
    ///
    /// # Errors
    /// Returns the failure of [`calculate_declared_routh`].
    pub fn run(
        &mut self,
        definition: &DeclaredControlAnalysis,
        selected: f64,
    ) -> Result<(), Failure> {
        self.selected = None;
        let result = calculate_declared_routh(definition, Some(selected))?;
        self.selected = Some(selected);
        self.saved = Some(SavedRouthAnalysis {
            selected_value: selected,
            result,
            definition: definition.clone(),
        });
        Ok(())
    }

    /// The parameter values for a runtime write of the selected value: the runtime name of each parameter
    /// (from `mapping`) with the selected value for the swept parameter and the stored value for the others.
    /// `None` when nothing is selected. A parameter without a runtime name is left out.
    #[must_use]
    pub fn selected_values(
        &self,
        mapping: &BTreeMap<String, String>,
        definition: &DeclaredControlAnalysis,
    ) -> Option<BTreeMap<String, f64>> {
        let selected = self.selected?;
        Some(
            definition
                .controller
                .parameters
                .iter()
                .filter_map(|(name, parameter)| {
                    let runtime = mapping.get(name)?;
                    let value = if *name == definition.root_locus.parameter {
                        selected
                    } else {
                        parameter.value
                    };
                    Some((runtime.clone(), value))
                })
                .collect(),
        )
    }

    /// Clears the selection and the saved result.
    pub fn invalidate(&mut self) {
        self.selected = None;
        self.saved = None;
    }

    /// The saved result, for storing it in the study.
    #[must_use]
    pub fn snapshot(&self) -> Option<SavedRouthAnalysis> {
        self.saved.clone()
    }

    /// Restores a stored result when it is valid and was calculated from `definition`. The restored result is
    /// read-only: it grants no parameter selection. Returns whether it was restored.
    pub fn restore(
        &mut self,
        snapshot: &SavedRouthAnalysis,
        definition: &DeclaredControlAnalysis,
    ) -> bool {
        if check_saved_routh(snapshot).is_err() || snapshot.definition != *definition {
            return false;
        }
        self.saved = Some(snapshot.clone());
        self.selected = None;
        true
    }
}

/// Checks a stored Routh–Hurwitz result: its definition is valid, its selection and parameter agree with the
/// result, and a union interval carries its regions and boundaries, with each open end below the next.
///
/// # Errors
/// Returns the declared-invalid sentence when a rule fails.
pub fn check_saved_routh(saved: &SavedRouthAnalysis) -> Result<(), Failure> {
    check_declared(&saved.definition)?;
    let result = &saved.result;
    let interval = &result.interval;
    let domain = result.allowed_domain;
    let union_complete = interval.kind != IntervalKind::Union
        || (interval.intervals.is_some() && interval.boundaries.is_some());
    let ends_ordered = match (interval.lower, interval.upper) {
        (Some(lower), Some(upper)) => lower < upper,
        _ => true,
    };
    #[allow(clippy::float_cmp)]
    // The saved value is compared with the value it was saved from, so it must be the same number.
    let consistent = result.selected_value == saved.selected_value
        && result.parameter == saved.definition.root_locus.parameter
        && domain.minimum.is_finite()
        && domain.maximum.is_finite()
        && domain.minimum <= domain.maximum
        && union_complete
        && ends_ordered;
    if consistent {
        Ok(())
    } else {
        Err(Failure::new(
            texts::DECLARED_INVALID,
            Cause::ModelOrData,
            NextAction::None,
        ))
    }
}

#[cfg(test)]
#[path = "routh_session_tests.rs"]
mod tests;
