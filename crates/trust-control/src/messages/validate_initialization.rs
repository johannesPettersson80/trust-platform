//! The Manual-to-Auto entry case of an Auto commissioning in the Validate answer (specification 35, B5.3 and B5.4): the result the
//! answer carries when the validation ran the case, and the study keeps it with the Validate section. `messages.rs` re-exports it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::validate_answer::RequirementOutcome;
use crate::identification::independent_validation::RequirementStatus;

/// The Manual-to-Auto entry case of an Auto commissioning (B5.3): the actual-ST run that starts the controller at the selected
/// model's operating point and enters Auto at the transfer, judged on its first output, its mode, its gains, its states and its
/// signal bounds. Its rows are guards (B9.9): the engine generated them from the loop's declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitializationOutcome {
    /// The words the table names the case with (B9.9): "Manual-to-Auto entry". Each row keeps its own label.
    pub label: String,
    /// The case's result (N8.5): Fail when a row fails, Pass when every row passes, and Inconclusive otherwise, so a case that did not
    /// complete never passes (B5.4).
    pub status: RequirementStatus,
    /// Each row of the case in its order, each with `guard` set.
    pub requirements: Vec<RequirementOutcome>,
}
