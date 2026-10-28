//! The declared analysis tools of `controlStudio/analyze` (`rootLocus` and `routh`, B9.9; N7.4 to N7.6): the pole paths and the
//! Routh–Hurwitz reading of the loop's declared analysis, at the selected value that a request gives.

use crate::analysis::budget::Budget;
use crate::analysis::declared::{
    calculate_declared_root_locus_bounded, DeclaredControlAnalysis, DeclaredRootLocusResult,
};
use crate::analysis::routh::DeclaredRouthResult;
use crate::analysis::routh_interval::calculate_declared_routh;
use crate::analysis::tool::input_failure;
use crate::failure::Failure;
use crate::messages::AnalysisInputs;

const NOT_FINITE: &str = "selectedValue must be a finite number.";
const OUTSIDE_SWEEP: &str = "selectedValue must be one of the declared sweep's values (N7.6).";

/// The root locus of the declared analysis at the request's selected value (`rootLocus`; N7.5, N7.6): the pole paths at each
/// gain of the sweep and across the declared range, with the sentences of the result. The selected value must be one of the
/// sweep's values. `budget` is checked before the roots of each gain (A7).
///
/// # Errors
/// Returns the input failure of a selected value outside the sweep, the declared-invalid sentence for an invalid analysis, or
/// the budget's or the solver's failure.
pub fn root_locus_answer(
    declared: &DeclaredControlAnalysis,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<DeclaredRootLocusResult, Failure> {
    let mut analysis = declared.clone();
    if let Some(selected) = inputs.selected_value {
        if !analysis.root_locus.values.contains(&selected) {
            return Err(input_failure(OUTSIDE_SWEEP));
        }
        analysis.root_locus.selected_value = selected;
    }
    calculate_declared_root_locus_bounded(&analysis, &|| budget.step())
}

/// The Routh–Hurwitz reading of the declared characteristic polynomial at the request's selected value (`routh`; N7.1 to
/// N7.4): the table, the stable interval of the parameter and the sentences of the result. Any finite value is read, because
/// N7.4 keeps the allowed domain apart from the mathematics. `budget` is checked before the reading (A7).
///
/// # Errors
/// Returns the input failure of a value that is not finite, the declared-invalid sentence for an invalid analysis, or the
/// budget's failure.
pub fn routh_answer(
    declared: &DeclaredControlAnalysis,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<DeclaredRouthResult, Failure> {
    let selected = inputs
        .selected_value
        .unwrap_or(declared.root_locus.selected_value);
    if !selected.is_finite() {
        return Err(input_failure(NOT_FINITE));
    }
    budget.step()?;
    calculate_declared_routh(declared, Some(selected))
}

#[cfg(test)]
#[path = "declared_tools_tests.rs"]
mod tests;
