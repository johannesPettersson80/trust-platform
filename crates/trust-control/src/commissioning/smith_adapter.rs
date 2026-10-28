//! The model values of a Smith predictor that Apply writes (specification 35, C9 and D-18): the model's gain, time constant and delay, and,
//! when the program uses `Control.SmithPredictorFOPDT`, the operating point that the block takes. They are bound parameters of the loop,
//! so they are written in the same transaction as the PI (C9, "Apply writes the PID gains and the model values in one transaction").

use crate::failure::{Cause, Failure, NextAction};
use crate::tuning::project_seed::{SmithPidSeed, SmithTerms};

/// The refusal of a Smith predictor whose candidate holds no model values, so Apply has nothing to write for the predictor.
const SMITH_VALUES_MISSING: &str =
    "A Smith predictor's candidate holds no model values, so Apply has nothing to write for the predictor.";

/// The variables and values that a Smith predictor's Apply writes beyond its PI (C9): the model's gain, time constant and delay, and,
/// when the seed names the library block, its operating input and output (D-18).
///
/// # Errors
/// The candidate holds no model values.
pub fn smith_terms<'a>(
    seed: &'a SmithPidSeed,
    terms: Option<&SmithTerms>,
) -> Result<Vec<(&'a str, f64)>, Failure> {
    let terms = terms
        .ok_or_else(|| Failure::new(SMITH_VALUES_MISSING, Cause::Settings, NextAction::Tune))?;
    let mut written = vec![
        (seed.gain_symbol.as_str(), terms.gain),
        (
            seed.time_constant_symbol.as_str(),
            terms.time_constant_seconds,
        ),
        (seed.delay_symbol.as_str(), terms.delay_seconds),
    ];
    if let Some(block) = &seed.library_block {
        written.push((block.operating_input_symbol.as_str(), terms.operating.0));
        written.push((block.operating_output_symbol.as_str(), terms.operating.1));
    }
    Ok(written)
}

#[cfg(test)]
#[path = "smith_adapter_tests.rs"]
mod tests;
