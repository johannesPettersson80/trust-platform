//! The lead/lag design of `leadLag` (B9.9; N7.14; draft §2.3). The design is the product's (`analysis::lead_lag`); this module checks
//! the inputs the page checked (each refusal names its input, A7) and writes the words of the answer: the compensator's formula and
//! the note that the design changes no controller.

use crate::analysis::lead_lag::{run_lead_lag, LeadLagMode, LeadLagRequest};
use crate::analysis::tool::{input_failure, number};
use crate::failure::Failure;
use crate::messages::{AnalysisInputs, LeadLagAnswer};
use crate::model::GeneralModel;

/// The largest phase a lead may add, in degrees (A7, N7.14; `MAXIMUM_LEAD_DEGREES`).
pub const MAXIMUM_LEAD_DEGREES: f64 = 85.0;

/// The note of every design: it is not applied (the page's words).
pub const DESIGN_DETAILS: &str =
    "This is an unapplied design result; no runtime controller was changed.";

/// The request of a lead/lag design from the read inputs, with each rule checked (A7): the crossover is finite and above zero, a lead
/// adds a phase above 0° and below 85°, and a lag has a ratio above 1. A mode or a value that is missing is named as missing.
///
/// # Errors
/// Returns the input failure that names the input the rule is about.
pub fn checked_request(inputs: &AnalysisInputs) -> Result<LeadLagRequest, Failure> {
    let mode = inputs
        .mode
        .ok_or_else(|| input_failure("Choose the compensator: a lead or a lag."))?;
    let crossover = inputs.crossover_rad_per_second.unwrap_or(f64::NAN);
    if !(crossover.is_finite() && crossover > 0.0) {
        return Err(input_failure(
            "The crossover must be a finite frequency above 0 rad/s.",
        ));
    }
    let value = inputs.value.unwrap_or(f64::NAN);
    let valid = match mode {
        LeadLagMode::Lead => value.is_finite() && value > 0.0 && value < MAXIMUM_LEAD_DEGREES,
        LeadLagMode::Lag => value.is_finite() && value > 1.0,
    };
    if !valid {
        return Err(input_failure(match mode {
            LeadLagMode::Lead => "The phase lead must be above 0° and below 85°.",
            LeadLagMode::Lag => "The lag ratio must be above 1.",
        }));
    }
    Ok(LeadLagRequest {
        mode,
        crossover_rad_per_second: crossover,
        value,
    })
}

/// The answer of `leadLag`: the design from the checked request, with the compensator's formula as its first sentence.
///
/// # Errors
/// Returns the input failure of [`checked_request`], or the refusal of the design (its cause and next action).
pub fn lead_lag_answer(
    model: &GeneralModel,
    inputs: &AnalysisInputs,
) -> Result<LeadLagAnswer, Failure> {
    let request = checked_request(inputs)?;
    let mode = request.mode;
    let design = run_lead_lag(model, request)?.result;
    let word = match mode {
        LeadLagMode::Lead => "Lead",
        LeadLagMode::Lag => "Lag",
    };
    let formula = format!(
        "{word} compensator: C(s) = {} · (1 + s/{}) / (1 + s/{}).",
        number(design.gain),
        number(design.zero_rad_per_second),
        number(design.pole_rad_per_second),
    );
    Ok(LeadLagAnswer {
        design,
        summary: vec![formula],
        details: vec![DESIGN_DETAILS.to_owned()],
    })
}

#[cfg(test)]
#[path = "lead_lag_tool_tests.rs"]
mod tests;
