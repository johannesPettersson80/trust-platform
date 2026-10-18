//! The `io.list`, `io.read`, `io.write`, `io.force` and `io.unforce` operations (specification 11,
//! "Live Values"; specification 34 §4.3.1).
//!
//! `io.list` and `io.read` answer from the published portable snapshot. `io.write`, `io.force` and
//! `io.unforce` queue into the platform's [`IoQueuePort`]; a refused request answers its stable
//! error text and changes nothing. None of the five needs the debug surface.

use alloc::format;
use alloc::string::{String, ToString};

use serde::Deserialize;
use serde_json::json;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::validate_process_image_address;
use trust_runtime_core::value::Value;

use super::io_snapshot::IoSnapshot;
use super::queues::RequestQueueFull;
use crate::protocol::ControlResponse;

/// The Live Values queues `io.write`, `io.force` and `io.unforce` queue into.
///
/// The composing platform owns the lock around the queues and reports a refusal with
/// [`RequestQueueFull`]; these operations own the request's parameters, the stable error texts and
/// the reply JSON.
pub trait IoQueuePort {
    /// Queue a one-shot I/O write.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when the queue already holds its capacity; the refused write queues
    /// nothing.
    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull>;

    /// Set the active force on `address`, replacing an earlier force on the same address.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `address` is not forced yet and the queue already holds its
    /// capacity.
    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull>;

    /// Release the active force on `address`, when one is set.
    fn release_io(&self, address: &IoAddress);
}

/// The parameters of `io.write` and `io.force`: an address and the value to apply.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoWriteParams {
    address: String,
    value: String,
}

/// The parameters of `io.unforce`: an address.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IoAddressParams {
    address: String,
}

/// `io.list`: the published I/O snapshot. `None` means no snapshot has been published yet.
#[must_use]
pub fn io_list(id: u64, snapshot: Option<&IoSnapshot>) -> ControlResponse {
    match snapshot {
        Some(snapshot) => ControlResponse::ok(id, snapshot.to_json()),
        None => ControlResponse::error(id, String::from("no snapshot available")),
    }
}

/// `io.read`: the published snapshot under `snapshot`, or `null` when none has been published yet.
#[must_use]
pub fn io_read(id: u64, snapshot: Option<&IoSnapshot>) -> ControlResponse {
    ControlResponse::ok(id, json!({ "snapshot": snapshot.map(IoSnapshot::to_json) }))
}

/// `io.write`: queue a one-shot write of `value` to the process-image `address`.
#[must_use]
pub fn io_write<Q: IoQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params = match parse_write_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let (address, value) = match address_and_value(&params.address, &params.value) {
        Ok(parsed) => parsed,
        Err(error) => return ControlResponse::error(id, error),
    };
    if let Err(error) = queues.enqueue_io_write(address, value) {
        return ControlResponse::error(id, error.to_string());
    }
    ControlResponse::ok(id, json!({"status": "queued"}))
}

/// `io.force`: hold the process-image `address` at `value`.
#[must_use]
pub fn io_force<Q: IoQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params = match parse_write_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let (address, value) = match address_and_value(&params.address, &params.value) {
        Ok(parsed) => parsed,
        Err(error) => return ControlResponse::error(id, error),
    };
    if let Err(error) = queues.force_io(address, value) {
        return ControlResponse::error(id, error.to_string());
    }
    ControlResponse::ok(id, json!({"status": "forced"}))
}

/// `io.unforce`: release the active force on the process-image `address`.
#[must_use]
pub fn io_unforce<Q: IoQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params: IoAddressParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, String::from("missing params")),
    };
    let address = match eligible_address(&params.address) {
        Ok(address) => address,
        Err(error) => return ControlResponse::error(id, error),
    };
    queues.release_io(&address);
    ControlResponse::ok(id, json!({"status": "released"}))
}

/// The parsed `io.write`/`io.force` parameters, or the refusal to answer with.
fn parse_write_params(
    id: u64,
    params: Option<serde_json::Value>,
) -> Result<IoWriteParams, ControlResponse> {
    match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => Ok(parsed),
            Err(err) => Err(ControlResponse::error(id, format!("invalid params: {err}"))),
        },
        None => Err(ControlResponse::error(id, String::from("missing params"))),
    }
}

/// A process-image address that a control request may write, force or release, and the value to
/// apply.
fn address_and_value(address: &str, value: &str) -> Result<(IoAddress, Value), String> {
    let address = eligible_address(address)?;
    let value = parse_value(value)?;
    Ok((address, value))
}

/// Parse `text` and check that it is a concrete process-image address of a writable area.
fn eligible_address(text: &str) -> Result<IoAddress, String> {
    let address = IoAddress::parse(text).map_err(|err| err.to_string())?;
    validate_process_image_address(&address).map_err(|err| err.to_string())?;
    Ok(address)
}

/// Parse the scalar grammar of a control value: a boolean or a signed 64-bit integer.
fn parse_value(text: &str) -> Result<Value, String> {
    let upper = text.trim().to_ascii_uppercase();
    if upper == "TRUE" {
        return Ok(Value::Bool(true));
    }
    if upper == "FALSE" {
        return Ok(Value::Bool(false));
    }
    if let Ok(value) = upper.parse::<i64>() {
        return Ok(Value::LInt(value));
    }
    Err(format!("unsupported value '{text}'"))
}

#[cfg(test)]
mod tests;
