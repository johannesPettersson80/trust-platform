//! The sentence that names a failed requirement (specification 35, B5.5): the value the run measured and the limit it broke, in the
//! units of the loop's signals. A failed Validate offers this sentence as its failure, so the engineer reads the shortfall where Tune
//! and Requirements are offered, and the table's row holds the same numbers (B9.9).

use crate::identification::independent_validation::RequirementStatus;
use crate::messages::{LimitSide, RequirementOutcome, ValueRange};
use crate::tuning::texts::engineering;
use crate::validation::table_rows::{range_text, unit_suffix};

/// The identity of the error requirement of a controlled output, whose measured value is the recorded target error (N8.3).
const TARGET_ERROR: &str = "target-error";
/// The identity of the settling requirement of a controlled output (N8.3).
const RESPONSE_DEADLINE: &str = "response-deadline";

/// The sentence that says what a failed requirement found (B5.5), or `None` when the requirement did not fail or its result holds no
/// finite number to put in a sentence, so the failure keeps the requirement's own reason (B5.4). The recorded target error and the
/// response that did not settle keep the words the extension used for them; any other requirement is named by its label. The numbers
/// are written as the table writes them (B9.9), with the unit of the requirement.
#[must_use]
pub fn failed_requirement_sentence(requirement: &RequirementOutcome) -> Option<String> {
    if requirement.status != RequirementStatus::Fail {
        return None;
    }
    let suffix = unit_suffix(&requirement.unit);
    // A requirement of declared bounds names the range the channel took. Without a good value in its window there is no range to name,
    // and its value and limit are the size of a violation, not a value against a limit: its own reason says what was found.
    if let Some(bounds) = requirement.limit_range {
        return requirement
            .measured_range
            .map(|measured| bounds_sentence(&requirement.label, measured, bounds, &suffix));
    }
    let limit = requirement.limit.filter(|limit| limit.is_finite())?;
    let value = requirement.value.filter(|value| value.is_finite());
    if is_requirement(&requirement.id, RESPONSE_DEADLINE) {
        let within = format!(
            "The recorded response did not settle within {}.",
            quantity(limit, &suffix)
        );
        return Some(match value {
            Some(value) => format!("{within} It settled after {}.", quantity(value, &suffix)),
            None => within,
        });
    }
    let value = value?;
    if is_requirement(&requirement.id, TARGET_ERROR) {
        return Some(format!(
            "Recorded target error {} exceeds {}.",
            quantity(value, &suffix),
            quantity(limit, &suffix)
        ));
    }
    let label = &requirement.label;
    Some(match requirement.limit_side {
        LimitSide::AtMost => format!(
            "{label}: {} exceeds the limit of {}.",
            quantity(value, &suffix),
            quantity(limit, &suffix)
        ),
        LimitSide::AtLeast => format!(
            "{label}: {} is below the limit of {}.",
            quantity(value, &suffix),
            quantity(limit, &suffix)
        ),
    })
}

/// The sentence of an operating-range or state-range requirement that failed: the range the channel took in its window, and the
/// bounds it had to stay within. Bounds of one value are a fixed value (a parameter that must not change).
fn bounds_sentence(label: &str, measured: ValueRange, bounds: ValueRange, suffix: &str) -> String {
    let found = range_text(measured, suffix);
    if engineering(bounds.minimum, None) == engineering(bounds.maximum, None) {
        format!(
            "{label}: {found} differs from the declared {}.",
            quantity(bounds.minimum, suffix)
        )
    } else {
        format!(
            "{label}: {found} is outside the declared {}.",
            range_text(bounds, suffix)
        )
    }
}

/// Whether the requirement `id` is the requirement `name` of a controlled output. A loop with several outputs puts the output's
/// identity before the name, up to a colon (`P1.VesselC:target-error`).
fn is_requirement(id: &str, name: &str) -> bool {
    id.rsplit(':').next() == Some(name)
}

/// A number with the unit that follows it, as the table writes a value.
fn quantity(value: f64, suffix: &str) -> String {
    format!("{}{suffix}", engineering(value, None))
}

#[cfg(test)]
#[path = "failure_sentence_tests.rs"]
mod tests;
