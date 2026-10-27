//! The `io.list`, `io.read`, `io.write`, `io.force` and `io.unforce` operations (specification 11,
//! "Live Values"; specification 34 §4.3.1).
//!
//! `io.list` and `io.read` answer from the published portable snapshot. `io.write`, `io.force` and
//! `io.unforce` queue into the platform's [`IoQueuePort`]; a refused request answers its stable
//! error text and changes nothing. None of the five needs the debug surface.
//!
//! The value of `io.write` and `io.force` at an address a variable is bound to is an IEC 61131-3
//! literal of that variable's declared type, in the image's form of the variable
//! ([`address_value`]), when the platform's program is at hand
//! ([`IoQueuePort::with_program_image`]): a Live Values row writes the variable it shows. At an
//! address no variable is bound to, and for a text of the untyped grammar that the declared type
//! refuses, the untyped grammar ([`parse_value`]) applies and the boundary types and refuses it, as
//! it always has (specification 11, "Live Values").

use alloc::format;
use alloc::string::{String, ToString};

use serde::{Deserialize, Serialize};
use serde_json::json;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::validate_process_image_address;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::value::Value;

use super::io_snapshot::IoSnapshot;
use super::queues::RequestQueueFull;
use super::value_grammar::{address_value, parse_value};
use crate::protocol::ControlResponse;

/// The published I/O snapshot a platform's `io.list` and `io.read` answer from.
///
/// It distinguishes a platform that has published no snapshot yet from one whose snapshot slot
/// cannot be read, so each answers its own stable text.
#[derive(Debug, Clone)]
pub enum IoSnapshotRead {
    /// The snapshot the last completed scan published.
    Published(IoSnapshot),
    /// The platform has not published a snapshot yet.
    Missing,
    /// The platform's snapshot slot could not be read.
    Unavailable,
}

/// The Live Values I/O surface `io.list`, `io.read`, `io.write`, `io.force` and `io.unforce` use:
/// the published snapshot and the queues.
///
/// The composing platform owns the snapshot slot's lock and the lock around the queues and reports
/// a refusal with [`RequestQueueFull`]; these operations own the request's parameters, the stable
/// error texts and the reply JSON.
pub trait IoQueuePort {
    /// The published I/O snapshot, or the state of the platform's snapshot slot.
    fn io_snapshot(&self) -> IoSnapshotRead;

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

    /// Runs `read` on the image of the program the requests write, when the platform has one at
    /// hand, and returns whether it did: `io.write` and `io.force` take an IEC literal of the type
    /// of the variable bound at their address through its tables (specification 11, "Live
    /// Values"). A platform without one leaves the value to the untyped grammar.
    fn with_program_image(&self, _read: &mut dyn FnMut(&Image<'_>)) -> bool {
        false
    }
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

/// `io.list`: the published I/O snapshot. `Missing` means no snapshot has been published yet and
/// `Unavailable` that the platform's snapshot slot could not be read.
#[must_use]
pub fn io_list(id: u64, snapshot: IoSnapshotRead) -> ControlResponse {
    match snapshot {
        IoSnapshotRead::Published(snapshot) => ControlResponse::ok_serialized(id, &snapshot),
        IoSnapshotRead::Missing => {
            ControlResponse::error(id, String::from("no snapshot available"))
        }
        IoSnapshotRead::Unavailable => {
            ControlResponse::error(id, String::from("I/O snapshot unavailable"))
        }
    }
}

/// The result of `io.read`: the published snapshot, or `null`.
#[derive(Serialize)]
struct SnapshotReply<'a> {
    snapshot: Option<&'a IoSnapshot>,
}

/// `io.read`: the published snapshot, or `null` when none has been published yet.
#[must_use]
pub fn io_read(id: u64, snapshot: IoSnapshotRead) -> ControlResponse {
    match snapshot {
        IoSnapshotRead::Published(snapshot) => ControlResponse::ok_serialized(
            id,
            &SnapshotReply {
                snapshot: Some(&snapshot),
            },
        ),
        IoSnapshotRead::Missing => {
            ControlResponse::ok_serialized(id, &SnapshotReply { snapshot: None })
        }
        IoSnapshotRead::Unavailable => {
            ControlResponse::error(id, String::from("I/O snapshot unavailable"))
        }
    }
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
    let (address, value) = match address_and_value(queues, &params.address, &params.value) {
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
    let (address, value) = match address_and_value(queues, &params.address, &params.value) {
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
/// apply: an IEC literal of the type of the variable bound at the address, in the image's form of
/// that variable, when the platform's program binds one there; otherwise, and for a text of the
/// untyped grammar that the bound type refuses, the untyped grammar, which the boundary types.
fn address_and_value<Q: IoQueuePort>(
    queues: &Q,
    address: &str,
    value: &str,
) -> Result<(IoAddress, Value), String> {
    let address = eligible_address(address)?;
    let mut typed = None;
    queues.with_program_image(&mut |image| typed = address_value(image, &address, value));
    let value = match typed {
        Some(Ok(value)) => value,
        Some(Err(reason)) => parse_value(value).map_err(|_| reason)?,
        None => parse_value(value)?,
    };
    Ok((address, value))
}

/// Parse `text` and check that it is a concrete process-image address of a writable area.
fn eligible_address(text: &str) -> Result<IoAddress, String> {
    let address = IoAddress::parse(text).map_err(|err| err.to_string())?;
    validate_process_image_address(&address).map_err(|err| err.to_string())?;
    Ok(address)
}

#[cfg(test)]
mod tests;
