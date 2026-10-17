use alloc::string::{String, ToString};
use core::fmt;

use serde::Serialize;
use smol_str::SmolStr;

/// Work the transport runs after it has written a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAfterResponse {
    /// Stop the resource; the reply to `shutdown` is written first.
    StopResource,
}

/// One control response: `{"id", "ok", "result", "error", "error_code", "audit_id"}`, where
/// absent members are omitted.
#[derive(Debug, Serialize)]
pub struct ControlResponse {
    id: u64,
    /// Whether the request succeeded.
    pub ok: bool,
    /// Result of a successful request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message of a refused or failed request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stable machine-readable code of the error, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audit_id: Option<String>,
    /// Work to run after the response has been written; never serialized.
    #[serde(skip)]
    pub after_response: Option<ControlAfterResponse>,
}

impl ControlResponse {
    /// A successful response carrying `result`.
    #[must_use]
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
            error_code: None,
            audit_id: None,
            after_response: None,
        }
    }

    /// The reply to `shutdown`: `{"status": "stopping"}`, and the resource stops after the reply
    /// has been written.
    #[must_use]
    pub fn shutdown(id: u64) -> Self {
        let mut result = serde_json::Map::new();
        result.insert("status".to_string(), serde_json::Value::from("stopping"));
        Self {
            id,
            ok: true,
            result: Some(serde_json::Value::Object(result)),
            error: None,
            error_code: None,
            audit_id: None,
            after_response: Some(ControlAfterResponse::StopResource),
        }
    }

    /// A failed response without an error code.
    #[must_use]
    pub fn error(id: u64, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
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
            error: Some(error),
            error_code: Some(error_code.to_string()),
            audit_id: None,
            after_response: None,
        }
    }

    /// Attaches the id of the audit record written for this request.
    #[must_use]
    pub fn with_audit_id(mut self, audit_id: Option<SmolStr>) -> Self {
        self.audit_id = audit_id.map(|value| value.to_string());
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
            error: Some(error),
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
        if let Some(audit_id) = &self.audit_id {
            let audit_id = serde_json::Value::from(audit_id.as_str());
            write!(out, r#","audit_id":{audit_id}"#)?;
        }
        out.write_char('}')
    }
}
