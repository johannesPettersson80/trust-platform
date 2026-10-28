//! The chosen trial of a finite search, as the runs of Validate write it (specification 35, C5 and B5.3): the trial's values are the run's
//! parameter overrides, one for every bound parameter of the loop. A search's own trials are isolated runs of the study's model
//! (`tuning::search::trial_run_request`); Validate runs the chosen candidate on the plant that B5.3 chooses, with the same overrides.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::tuning::search::SearchCandidate;

/// The refusal of a loop with no parameter binding, which a trial's values are written through (C0).
const NO_BINDING: &str = "The loop declares no parameter binding, so a search's trial has nowhere to be written (C0, C5).";

/// The values that the chosen trial writes (C5): one value for every bound parameter of the loop, so that the run writes the whole trial
/// and no parameter is left to the controller's own record.
///
/// # Errors
/// The loop declares no parameter binding, or a bound parameter has no value in the chosen trial.
pub fn candidate_coefficients<'a>(
    loop_definition: &LoopDefinition,
    search: &'a SearchCandidate,
) -> Result<&'a BTreeMap<String, f64>, Failure> {
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(|| Failure::new(NO_BINDING, Cause::Settings, NextAction::OpenLoopSetup))?;
    match binding
        .parameters
        .iter()
        .find(|parameter| !search.parameters.contains_key(&parameter.symbol))
    {
        Some(parameter) => Err(Failure::new(
            &format!(
                "The chosen trial holds no value for {}, a bound parameter of the loop (C5).",
                parameter.symbol
            ),
            Cause::Settings,
            NextAction::Tune,
        )),
        None => Ok(&search.parameters),
    }
}

#[cfg(test)]
#[path = "search_case_tests.rs"]
mod tests;
