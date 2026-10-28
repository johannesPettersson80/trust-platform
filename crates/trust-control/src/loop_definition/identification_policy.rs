//! The identification policy a loop file declares, checked when the loop is read (specification 35, B3.3 and N3.2). The rules are those of
//! the page's validator, so the engine refuses a policy that the page would refuse. The policy is kept as the file writes it; a stored policy
//! that does not decode is otherwise read as absent, and the loop would open without the limit the file states.

use serde_json::Value;

use crate::identification::policy::decode_identification_policy;

/// The optional fields of a policy. A field that the file names as null is refused: a null is not an absent value.
const OPTIONAL_FIELDS: [&str; 3] = ["maximumNormalizedError", "maximumOrder", "process"];

/// The sentence that refuses a policy whose fields or limits are outside their rules.
const REFUSED: &str = "The identification policy must be an object whose maximum normalised error is above zero and below one, whose maximum order is a whole number from one to eight, whose openLoopPermitted and sustainedHoldPermitted are true or false, whose information is basic, frequency or multiple-modes, and whose process, when it names one, is integrating.";

/// Checks the identification policy a loop file declares: its object form, its optional fields that are not null, and its limits and
/// values (`decode_identification_policy`).
///
/// # Errors
/// The policy is not an object, names a null optional field, or breaks one of its rules.
pub fn check(value: &Value) -> Result<(), String> {
    let Some(fields) = value.as_object() else {
        return Err(REFUSED.to_owned());
    };
    if OPTIONAL_FIELDS
        .iter()
        .any(|field| fields.get(*field).is_some_and(Value::is_null))
    {
        return Err(REFUSED.to_owned());
    }
    decode_identification_policy(value)
        .map(|_| ())
        .ok_or_else(|| REFUSED.to_owned())
}

#[cfg(test)]
#[path = "identification_policy_tests.rs"]
mod tests;
