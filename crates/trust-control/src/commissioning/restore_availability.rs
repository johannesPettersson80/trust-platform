//! Whether Restore previous settings may proceed (specification 35, B6.4), from `restoreAvailability.ts`.

use super::parameter_status::{same_parameter_history, ParameterStatus};
use super::texts;

/// Restore previous settings (B6.4) returns the settings Apply wrote only while they are still the active ones. Once the
/// program, the active revision or the values changed since Apply, Restore is unavailable and nothing is overwritten.
/// Returns the reason, or `None` when Restore may proceed (`restoreUnavailableReason`).
#[must_use]
pub fn restore_unavailable_reason(
    observed: &ParameterStatus,
    applied: Option<&ParameterStatus>,
) -> Option<String> {
    if applied.is_none() || same_parameter_history(observed, applied) {
        return None;
    }
    Some(texts::restore_unavailable())
}

#[cfg(test)]
#[path = "restore_availability_tests.rs"]
mod tests;
