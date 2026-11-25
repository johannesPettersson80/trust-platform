use alloc::boxed::Box;
use alloc::collections::TryReserveError;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use smol_str::SmolStr;

/// Work the transport runs after it has written a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAfterResponse {
    /// Stop the program; the reply to `shutdown` is written first. The runtime stays reachable in
    /// the Stopped state (specification 34, STATE-08).
    StopResource,
    /// Stop the program and then end the runtime's process; the reply to `shutdown` with
    /// `{"exit": true}` is written first. A platform whose firmware never ends, such as a chip,
    /// stops the program and stays reachable, as for [`Self::StopResource`] (specification 34,
    /// STATE-08).
    ExitRuntime,
}

/// One control response: `{"id", "ok", "result", "error", "error_code", "audit_id"}`, where
/// absent members are omitted.
#[derive(Debug)]
pub struct ControlResponse {
    id: u64,
    /// Whether the request succeeded.
    pub ok: bool,
    /// Result of a successful request.
    pub result: Option<serde_json::Value>,
    /// The JSON text of the result of a request whose handler wrote it without building a value,
    /// as the frequent reads do, written as the result in place of `result`, which is then `None`
    /// (specification 34 PORT-02: the work of a request between scans is bounded).
    result_text: Option<Box<RawValue>>,
    /// Error message of a refused or failed request.
    pub error: Option<String>,
    /// Stable machine-readable code of the error, when it has one.
    pub error_code: Option<Box<str>>,
    audit_id: Option<Box<str>>,
    /// Work to run after the response has been written; never serialized.
    pub after_response: Option<ControlAfterResponse>,
}

impl Serialize for ControlResponse {
    /// `id`, `ok`, then `result`, `error`, `error_code` and `audit_id` when present, as the link
    /// writes them; the work after the response is never written.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut response = serializer.serialize_struct("ControlResponse", 6)?;
        response.serialize_field("id", &self.id)?;
        response.serialize_field("ok", &self.ok)?;
        match (&self.result, &self.result_text) {
            (Some(result), _) => response.serialize_field("result", result)?,
            (None, Some(text)) => response.serialize_field("result", text)?,
            (None, None) => response.skip_field("result")?,
        }
        match &self.error {
            Some(error) => response.serialize_field("error", error)?,
            None => response.skip_field("error")?,
        }
        match &self.error_code {
            Some(code) => response.serialize_field("error_code", code)?,
            None => response.skip_field("error_code")?,
        }
        match &self.audit_id {
            Some(audit_id) => response.serialize_field("audit_id", audit_id)?,
            None => response.skip_field("audit_id")?,
        }
        response.end()
    }
}

/// The bytes of a short read's result text, for which the heap is asked before it is written.
const RESULT_TEXT_BYTES: usize = 512;

/// Takes a block of `len` bytes, or refuses when the heap cannot hold it: how the room for a
/// result's text is asked for. A test passes one that refuses, to see that the reply is refused
/// rather than the runtime aborted.
pub(super) type TextReserve = fn(usize) -> Result<Vec<u8>, TryReserveError>;

/// Takes a block of exactly `len` bytes with `try_reserve_exact`.
pub(super) fn reserve_text(len: usize) -> Result<Vec<u8>, TryReserveError> {
    let mut text = Vec::new();
    text.try_reserve_exact(len)?;
    Ok(text)
}

/// Why a result's text was not written.
#[derive(Debug)]
pub(super) enum TextRefusal {
    /// The heap had no room for the text after `written` bytes of it.
    NoRoom {
        /// The bytes written when the heap had no room.
        written: usize,
    },
    /// The result does not serialize.
    Invalid(String),
}

/// Asks the heap for the blocks a result's text of up to `bytes` bytes holds at its peak, and gives
/// them back. Without `std`, `serde_json` writes a text only into a block of its own, which starts
/// at 128 bytes and doubles as the text grows, and then copies the text into a block of its length.
/// Such a text ends in a block of [`text_block`]`(bytes)`, the power of two at or above `bytes`, and
/// at its peak holds that block beside the half it grew from or the copy, each at most `bytes`. The
/// loop that serves the request makes no other allocation before the text is written, so a heap
/// that gave both has room for it, and one that has none refuses the reply before any of it is
/// written (specification 34 §7.4, "Reply refusals").
pub(super) fn room_for_text(bytes: usize, reserve: TextReserve) -> Result<(), TextRefusal> {
    let first = reserve(text_block(bytes)).map_err(|_| TextRefusal::NoRoom { written: 0 })?;
    let second = reserve(bytes).map_err(|_| TextRefusal::NoRoom { written: 0 })?;
    drop(second);
    drop(first);
    Ok(())
}

/// The block a text of up to `bytes` bytes ends in as `serde_json` writes it: its block starts at
/// 128 bytes and doubles, so the power of two at or above `bytes`, and 128 at least.
pub(super) fn text_block(bytes: usize) -> usize {
    bytes.max(128).next_power_of_two()
}

/// The JSON text of `result`, once the heap has shown room for it ([`room_for_text`]).
pub(super) fn result_text<T: Serialize + ?Sized>(
    result: &T,
    bytes: usize,
    reserve: TextReserve,
) -> Result<Box<RawValue>, TextRefusal> {
    room_for_text(bytes, reserve)?;
    serde_json::value::to_raw_value(result).map_err(|error| TextRefusal::Invalid(error.to_string()))
}

impl ControlResponse {
    /// A successful response carrying `result`.
    #[must_use]
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            result_text: None,
            error: None,
            error_code: None,
            audit_id: None,
            after_response: None,
        }
    }

    /// A successful response whose result is `result` written as JSON text without building a
    /// value: the reply of a frequent read, whose work between scans stays small (specification 34
    /// PORT-02), for a text of up to 512 bytes, as a short read's is ([`Self::ok_serialized_within`]).
    #[must_use]
    pub fn ok_serialized<T: Serialize + ?Sized>(id: u64, result: &T) -> Self {
        Self::ok_serialized_within(id, result, RESULT_TEXT_BYTES)
    }

    /// As [`Self::ok_serialized`], for a result whose text fits `bytes`: the heap is asked first
    /// for the blocks such a text holds at its peak, and a heap that has no room for them answers
    /// `no room for a reply longer than 0 bytes`, never an abort (specification 34 §7.4, "Reply
    /// refusals"). A longer text grows as it is written. A result that does not serialize, which a
    /// reply of plain fields never is, answers the error that says so.
    #[must_use]
    pub fn ok_serialized_within<T: Serialize + ?Sized>(id: u64, result: &T, bytes: usize) -> Self {
        Self::of_text(id, result_text(result, bytes, reserve_text))
    }

    /// The response of a result's text, or of the refusal that it could not be written.
    pub(super) fn of_text(id: u64, text: Result<Box<RawValue>, TextRefusal>) -> Self {
        match text {
            Ok(text) => Self {
                id,
                ok: true,
                result: None,
                result_text: Some(text),
                error: None,
                error_code: None,
                audit_id: None,
                after_response: None,
            },
            Err(TextRefusal::NoRoom { written }) => Self::error(
                id,
                format!("no room for a reply longer than {written} bytes"),
            ),
            Err(TextRefusal::Invalid(error)) => {
                Self::error(id, format!("result does not serialize: {error}"))
            }
        }
    }

    /// The result as a value, whether the handler built it or wrote its text: for a reader that
    /// looks into a result, such as a test. `None` without a result.
    #[must_use]
    pub fn result_value(&self) -> Option<serde_json::Value> {
        match (&self.result, &self.result_text) {
            (Some(result), _) => Some(result.clone()),
            (None, Some(text)) => serde_json::from_str(text.get()).ok(),
            (None, None) => None,
        }
    }

    /// The result as a value, as [`Self::result_value`] gives it, taken out of the response.
    #[must_use]
    pub fn into_result_value(self) -> Option<serde_json::Value> {
        match (self.result, self.result_text) {
            (Some(result), _) => Some(result),
            (None, Some(text)) => serde_json::from_str(text.get()).ok(),
            (None, None) => None,
        }
    }

    /// The reply to `shutdown`: `{"status": "stopping"}`, and the program stops after the reply
    /// has been written.
    #[must_use]
    pub fn shutdown(id: u64) -> Self {
        Self::stopping(id, ControlAfterResponse::StopResource)
    }

    /// The reply to `shutdown` with `{"exit": true}`: the same `{"status": "stopping"}`, and the
    /// program stops and the runtime's process ends after the reply has been written.
    #[must_use]
    pub fn shutdown_and_exit(id: u64) -> Self {
        Self::stopping(id, ControlAfterResponse::ExitRuntime)
    }

    /// The `stopping` reply that defers `after_response` until the reply has been written.
    fn stopping(id: u64, after_response: ControlAfterResponse) -> Self {
        let mut result = serde_json::Map::new();
        result.insert("status".to_string(), serde_json::Value::from("stopping"));
        Self {
            id,
            ok: true,
            result: Some(serde_json::Value::Object(result)),
            result_text: None,
            error: None,
            error_code: None,
            audit_id: None,
            after_response: Some(after_response),
        }
    }

    /// A failed response without an error code.
    #[must_use]
    pub fn error(id: u64, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            result_text: None,
            error: Some(error),
            error_code: None,
            audit_id: None,
            after_response: None,
        }
    }

    /// A failed response with a stable error code.
    #[must_use]
    pub fn error_with_code(id: u64, error: String, error_code: &'static str) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            result_text: None,
            error: Some(error),
            error_code: Some(error_code.into()),
            audit_id: None,
            after_response: None,
        }
    }

    /// A failed response with a stable error code and the result that describes the failure, as a
    /// plant request's error envelope carries it (specification 47 §20.1).
    #[must_use]
    pub fn error_with_code_and_result(
        id: u64,
        error: String,
        error_code: &'static str,
        result: serde_json::Value,
    ) -> Self {
        Self {
            id,
            ok: false,
            result: Some(result),
            result_text: None,
            error: Some(error),
            error_code: Some(error_code.into()),
            audit_id: None,
            after_response: None,
        }
    }

    /// Attaches the id of the audit record written for this request.
    #[must_use]
    pub fn with_audit_id(mut self, audit_id: Option<SmolStr>) -> Self {
        self.audit_id = audit_id.map(|value| value.as_str().into());
        self
    }

    /// The failure `error` in place of this response, for one that cannot be delivered as it is:
    /// the same id, audit record id and work after the response, without a result or error code.
    #[must_use]
    pub fn replaced_by_error(&self, error: String) -> Self {
        Self {
            id: self.id,
            ok: false,
            result: None,
            result_text: None,
            error: Some(error),
            error_code: None,
            audit_id: self.audit_id.clone(),
            after_response: self.after_response,
        }
    }

    /// The success `result` in place of this response, for one whose reply is held and answered with
    /// its head: the same id and audit record id, and no error or error code.
    #[must_use]
    pub(crate) fn replaced_by_result(&self, result: serde_json::Value) -> Self {
        Self {
            id: self.id,
            ok: true,
            result: Some(result),
            result_text: None,
            error: None,
            error_code: None,
            audit_id: self.audit_id.clone(),
            after_response: self.after_response,
        }
    }

    /// Writes the wire text of a successful response to request `id` up to its result:
    /// `{"id":<id>,"ok":true,"result":`. With the result's JSON text after it and then
    /// [`Self::write_ok_end`], it is exactly the text that serializing the response writes.
    pub(crate) fn write_ok_start(out: &mut impl fmt::Write, id: u64) -> fmt::Result {
        write!(out, r#"{{"id":{id},"ok":true,"result":"#)
    }

    /// Writes the wire text that follows the result of this successful response: its audit
    /// record id, when it has one, and the closing brace.
    pub(crate) fn write_ok_end(&self, out: &mut impl fmt::Write) -> fmt::Result {
        write_text_member(out, "audit_id", self.audit_id.as_deref())?;
        out.write_char('}')
    }

    /// Writes the wire text of this response, member by member, exactly as serializing it writes
    /// it: `id`, `ok`, then `result`, `error`, `error_code` and `audit_id` when present. The work
    /// after the response is never written. A writer that takes its text in parts, `ReplyWriter`,
    /// takes a response of any length this way, without a serializer that builds it whole.
    pub(crate) fn write_to(&self, out: &mut impl fmt::Write) -> fmt::Result {
        write!(out, r#"{{"id":{},"ok":{}"#, self.id, self.ok)?;
        if let Some(result) = &self.result {
            write!(out, r#","result":{result}"#)?;
        } else if let Some(text) = &self.result_text {
            out.write_str(r#","result":"#)?;
            out.write_str(text.get())?;
        }
        write_text_member(out, "error", self.error.as_deref())?;
        write_text_member(out, "error_code", self.error_code.as_deref())?;
        write_text_member(out, "audit_id", self.audit_id.as_deref())?;
        out.write_char('}')
    }
}

/// Writes `,"<name>":<text as a JSON string>` when `text` is present, with the escapes that
/// serializing a string writes.
fn write_text_member(out: &mut impl fmt::Write, name: &str, text: Option<&str>) -> fmt::Result {
    match text {
        Some(text) => write!(out, r#","{name}":{}"#, serde_json::Value::from(text)),
        None => Ok(()),
    }
}
