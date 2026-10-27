//! Parameter checks and the stable refusals of the read operations.

use alloc::format;

use crate::protocol::ControlResponse;

/// The `limit` parameter of a read operation: `default` when absent, otherwise 1 to `maximum`.
///
/// `params`, when present, must be an object whose fields are all in `allowed_fields` and whose
/// `limit` is not null.
///
/// # Errors
///
/// The stable text of the parameter problem.
pub fn parse_bounded_limit(
    params: Option<&serde_json::Value>,
    default: usize,
    maximum: usize,
    allowed_fields: &[&str],
) -> Result<usize, &'static str> {
    let Some(params) = params else {
        return Ok(default);
    };
    validate_object_fields(params, allowed_fields, &["limit"])?;
    let Some(value) = params.get("limit") else {
        return Ok(default);
    };
    let Some(limit) = value.as_u64().and_then(|value| usize::try_from(value).ok()) else {
        return Err("limit must be an integer");
    };
    if !(1..=maximum).contains(&limit) {
        return Err("limit is outside the supported range");
    }
    Ok(limit)
}

/// Checks that `value` is an object whose fields are all in `allowed_fields` and whose
/// `non_null_fields` are not null.
///
/// # Errors
///
/// The stable text of the parameter problem.
pub fn validate_object_fields(
    value: &serde_json::Value,
    allowed_fields: &[&str],
    non_null_fields: &[&str],
) -> Result<(), &'static str> {
    let Some(object) = value.as_object() else {
        return Err("parameters must be an object");
    };
    if object
        .keys()
        .any(|key| !allowed_fields.contains(&key.as_str()))
    {
        return Err("unknown parameter field");
    }
    if non_null_fields
        .iter()
        .any(|field| object.get(*field).is_some_and(serde_json::Value::is_null))
    {
        return Err("parameter fields must not be null");
    }
    Ok(())
}

/// The refusal `invalid params: <error>`.
#[must_use]
pub fn invalid_params(id: u64, error: &str) -> ControlResponse {
    ControlResponse::error(id, format!("invalid params: {error}"))
}

/// The refusal `<component> unavailable`, for a value that cannot be read.
#[must_use]
pub fn unavailable(id: u64, component: &str) -> ControlResponse {
    ControlResponse::error(id, format!("{component} unavailable"))
}

/// How an operation reads one member of its parameters, for [`check_number_members`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// A string member: a JSON number is refused.
    Text,
    /// A whole-number member: a JSON number that is not a non-negative integer is refused.
    WholeNumber,
}

/// Refuses a JSON number that serde would read with the platform's number handling: `params` itself
/// when it is a number that is not a whole one, and a JSON number in a member of `params` that the
/// operation reads, before serde reads them.
///
/// `serde_json` reads a fractional number differently when its `arbitrary_precision` feature is on,
/// as it is on a computer (through opcua) and off on a chip, so the message serde gives for such a
/// number would differ between the two. The number is refused here instead, with one text on every
/// runtime (specification 11, `invalid params`). A member that is absent, or holds another JSON
/// type, is left to serde, whose text does not depend on that feature.
///
/// # Errors
///
/// `invalid params: parameters must be an object` for a `params` that is a number other than a whole
/// one, `invalid params: <member> must be a string` for a number in a string member, and
/// `invalid params: <member> must be a whole number` for a number in a whole-number member that is
/// not a non-negative integer.
pub fn check_number_members(
    id: u64,
    params: &serde_json::Value,
    members: &[(&str, MemberKind)],
) -> Result<(), ControlResponse> {
    if params.is_number() && params.as_u64().is_none() {
        return Err(invalid_params(id, "parameters must be an object"));
    }
    for &(name, kind) in members {
        let Some(value) = params.get(name).filter(|value| value.is_number()) else {
            continue;
        };
        let (refused, expected) = match kind {
            MemberKind::Text => (true, "a string"),
            MemberKind::WholeNumber => (value.as_u64().is_none(), "a whole number"),
        };
        if refused {
            return Err(invalid_params(id, &format!("{name} must be {expected}")));
        }
    }
    Ok(())
}
