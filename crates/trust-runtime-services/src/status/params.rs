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
