//! The control request pipeline (specification 11, §6.8.1 and §6.9.5).
//!
//! Each request is read, its credential channel stamped and its role resolved; the operation
//! registry and the debug surface are checked; the shared operations are answered here and every
//! other operation goes to the platform; and every request that could be read, refused or not,
//! gets one audit record, whose id the response carries. For a link that moves replies of a
//! bounded length (specification 34, PORT-05), the event reads write their reply event by event
//! within the bound.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::json;
use smol_str::SmolStr;

use crate::access::{
    is_debug_request, required_role_for_control_request, stamp_credential_channel_param, AccessRole,
};
use crate::capabilities;
use crate::events::{wall_time, RuntimeEvent};
use crate::live_values::{io_requests, var_requests};
use crate::ports::{AuditRecord, ControlHost};
use crate::protocol::{BoundedReply, ControlAfterResponse, ControlRequest, ControlResponse};
use crate::resource::operations;
use crate::status::{self, EventRead};

/// One request line's serialized response and the work to run after writing it.
#[derive(Debug)]
pub struct PreparedLine {
    /// The response: one JSON object, without a line end.
    pub response: String,
    /// Work to run after the response has been written.
    pub after_response: Option<ControlAfterResponse>,
}

/// Answers one request line.
///
/// A line that is not JSON gets `invalid request` with id 0 and no audit record; a response
/// that cannot be serialized becomes `response serialization failed` with id 0.
#[must_use]
pub fn prepare_line<H: ControlHost>(line: &str, host: &H, client: Option<&str>) -> PreparedLine {
    let mut response = respond(line, host, client);
    let after_response = response.after_response.take();
    PreparedLine {
        response: serialize_response(&response),
        after_response,
    }
}

/// Answers one request line as [`prepare_line`] does, for a link that moves replies of at most
/// `max_len` bytes (specification 34, PORT-05). A longer reply is never cut: it is replaced by
/// `reply longer than <max_len> bytes`, with the request's id and audit record id, and the work
/// after the reply stays.
///
/// `events.tail`, `events` and `faults` write their reply event by event into one buffer of
/// `max_len` bytes and stop at its end, so their reply is never built longer than the link moves;
/// a reply that fits is the same bytes that [`prepare_line`] writes.
#[must_use]
pub fn prepare_line_within<H: ControlHost>(
    line: &str,
    host: &H,
    client: Option<&str>,
    max_len: usize,
) -> PreparedLine {
    let value = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value,
        Err(err) => return within(unreadable_line(&err), max_len),
    };
    let admitted = match admit(value, host, client) {
        Ok(admitted) => admitted,
        Err(refused) => return within(refused, max_len),
    };
    match EventRead::of(admitted.request.r#type.as_str()) {
        Some(read) => event_read_within(read, &admitted.request, host, client, max_len),
        None => within(answer(admitted, host, client), max_len),
    }
}

/// The response to one request line, before it is serialized.
fn respond<H: ControlHost>(line: &str, host: &H, client: Option<&str>) -> ControlResponse {
    match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => dispatch_value(value, host, client),
        Err(err) => unreadable_line(&err),
    }
}

/// The answer to a line that is not JSON: `invalid request` with id 0 and no audit record.
fn unreadable_line(err: &serde_json::Error) -> ControlResponse {
    ControlResponse::error(0, format!("invalid request: {err}"))
}

/// `response` serialized for a link that moves replies of at most `max_len` bytes. A longer one
/// is replaced by `reply longer than <max_len> bytes` with the same id and audit record id, and
/// its work after the reply stays.
fn within(mut response: ControlResponse, max_len: usize) -> PreparedLine {
    let after_response = response.after_response.take();
    let mut serialized = serialize_response(&response);
    if serialized.len() > max_len {
        serialized = serialize_response(&response.replaced_by_error(reply_too_long(max_len)));
    }
    PreparedLine {
        response: serialized,
        after_response,
    }
}

/// The reply to the admitted event read `request` for a link that moves replies of at most
/// `max_len` bytes, written event by event into one buffer of that size. The read was served, so
/// it is audited as served whether its reply fits or not, as [`within`] keeps the audit record of
/// any reply it replaces.
fn event_read_within<H: ControlHost>(
    read: EventRead,
    request: &ControlRequest,
    host: &H,
    client: Option<&str>,
    max_len: usize,
) -> PreparedLine {
    let mut reply = BoundedReply::ok(request.id, max_len);
    if let Err(refused) =
        status::write_event_read(read, request.id, request.params.as_ref(), host, &mut reply)
    {
        return within(audited(host, request, client, refused), max_len);
    }
    let served = audited(
        host,
        request,
        client,
        ControlResponse::ok(request.id, serde_json::Value::Null),
    );
    let response = reply
        .finish(&served)
        .unwrap_or_else(|| serialize_response(&served.replaced_by_error(reply_too_long(max_len))));
    PreparedLine {
        response,
        after_response: None,
    }
}

/// The error that replaces a reply longer than the link moves.
fn reply_too_long(max_len: usize) -> String {
    format!("reply longer than {max_len} bytes")
}

/// The reply to a frame that cannot be a request line, for example one longer than the link
/// accepts: `invalid request: <error>` with id 0 and no audit record, as [`prepare_line`] answers
/// a line that is not JSON.
#[must_use]
pub fn invalid_line(error: &str) -> PreparedLine {
    let response = ControlResponse::error(0, format!("invalid request: {error}"));
    PreparedLine {
        response: serialize_response(&response),
        after_response: None,
    }
}

/// One JSON object; a response that cannot be serialized becomes `response serialization failed`
/// with id 0.
fn serialize_response(response: &ControlResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|err| {
        json!({
            "id": 0_u64,
            "ok": false,
            "error": format!("response serialization failed: {err}"),
        })
        .to_string()
    })
}

/// Answers one request read as a JSON value; `client` is the transport label of the client.
///
/// A value that is not a request gets `invalid request` with id 0 and an audit record of type
/// `invalid`.
#[must_use]
pub fn dispatch_value<H: ControlHost>(
    value: serde_json::Value,
    host: &H,
    client: Option<&str>,
) -> ControlResponse {
    match admit(value, host, client) {
        Ok(admitted) => answer(admitted, host, client),
        Err(refused) => refused,
    }
}

/// A request that may be served: it could be read, its role allows it and its surface is
/// enabled.
struct Admitted {
    request: ControlRequest,
    role: AccessRole,
}

/// Reads `value` as a request and checks its access. A request that is refused is answered and
/// audited here.
fn admit<H: ControlHost>(
    value: serde_json::Value,
    host: &H,
    client: Option<&str>,
) -> Result<Admitted, ControlResponse> {
    let mut request: ControlRequest = match serde_json::from_value(value) {
        Ok(request) => request,
        Err(err) => {
            let error = format!("invalid request: {err}");
            let audit_id = host.record_audit(AuditRecord {
                request_id: 0,
                request_type: SmolStr::new("invalid"),
                correlation_id: None,
                ok: false,
                error: Some(SmolStr::new(&error)),
                auth_present: false,
                client,
                details: None,
            });
            return Err(ControlResponse::error(0, error).with_audit_id(audit_id));
        }
    };
    stamp_credential_channel_param(&mut request, client);
    let role = match host.request_role(&request, client) {
        Ok(role) => role,
        Err(failure) => {
            let response = ControlResponse::error_with_code(
                request.id,
                failure.message().to_string(),
                failure.code(),
            );
            return Err(audited(host, &request, client, response));
        }
    };
    let required =
        required_role_for_control_request(request.r#type.as_str(), request.params.as_ref());
    if !role.allows(required) {
        let response = ControlResponse::error_with_code(
            request.id,
            format!("forbidden: requires role {}", required.as_str()),
            "insufficient_role",
        );
        return Err(audited(host, &request, client, response));
    }
    if !host.debug_enabled() && is_debug_request(request.r#type.as_str()) {
        host.record_event(RuntimeEvent::FeatureDisabled {
            feature: SmolStr::new("debug"),
            request_type: Some(SmolStr::new(request.r#type.as_str())),
            time: wall_time(host.utc_ms()),
        });
        let response = ControlResponse::error_with_code(
            request.id,
            "debug disabled".into(),
            "feature_disabled",
        );
        return Err(audited(host, &request, client, response));
    }
    Ok(Admitted { request, role })
}

/// Serves an admitted request, adds the access capabilities to a `status` result, and audits it.
fn answer<H: ControlHost>(admitted: Admitted, host: &H, client: Option<&str>) -> ControlResponse {
    let Admitted { request, role } = admitted;
    let mut response = route(&request, host)
        .unwrap_or_else(|| ControlResponse::error(request.id, "unsupported request".into()));
    if request.r#type == "status" && response.ok {
        attach_access_capabilities(&mut response, role);
    }
    audited(host, &request, client, response)
}

/// Answers the shared operations and hands every other one to the platform.
fn route<H: ControlHost>(request: &ControlRequest, host: &H) -> Option<ControlResponse> {
    let id = request.id;
    let response = match request.r#type.as_str() {
        "status" => status::status(id, host),
        "capabilities" => capabilities::capabilities(id, host),
        "health" => status::health(id, host),
        "tasks.stats" => status::task_stats(id, host),
        "events.tail" | "events" => status::events_tail(id, request.params.as_ref(), host),
        "faults" => status::faults(id, request.params.as_ref(), host),
        "io.list" => io_requests::io_list(id, host.io_snapshot()),
        "io.read" => io_requests::io_read(id, host.io_snapshot()),
        "io.write" => io_requests::io_write(id, request.params.clone(), host),
        "io.force" => io_requests::io_force(id, request.params.clone(), host),
        "io.unforce" => io_requests::io_unforce(id, request.params.clone(), host),
        "set" => var_requests::set(id, request.params.clone(), host),
        "var.force" => var_requests::var_force(id, request.params.clone(), host),
        "var.unforce" => var_requests::var_unforce(id, request.params.clone(), host),
        "var.forced" => var_requests::var_forced(id, host),
        "pause" => operations::pause(id, host),
        "resume" => operations::resume(id, host),
        "restart" => operations::restart(id, request.params.clone(), host),
        "shutdown" => operations::shutdown(id),
        _ => return host.dispatch_extension(request),
    };
    Some(response)
}

/// Records the audit record of `request` and its `response`, and attaches the record's id.
fn audited<H: ControlHost>(
    host: &H,
    request: &ControlRequest,
    client: Option<&str>,
    response: ControlResponse,
) -> ControlResponse {
    let audit_id = host.record_audit(AuditRecord {
        request_id: request.id,
        request_type: SmolStr::new(request.r#type.as_str()),
        correlation_id: request.request_id.as_deref(),
        ok: response.ok,
        error: response.error.as_ref().map(SmolStr::new),
        auth_present: request.auth.is_some(),
        client,
        details: host.audit_details(request),
    });
    response.with_audit_id(audit_id)
}

/// Adds what `role` may change to an object result of `status`; other results stay as they are.
fn attach_access_capabilities(response: &mut ControlResponse, role: AccessRole) {
    let Some(result) = response.result.as_mut() else {
        return;
    };
    let Some(object) = result.as_object_mut() else {
        return;
    };
    object.insert("access".to_string(), access_capabilities_json(role));
}

/// What `role` may change: writes, forces and releases of values, and HMI writes, which need
/// `Engineer`; a lower role carries the reason.
fn access_capabilities_json(role: AccessRole) -> serde_json::Value {
    let engineer = role.allows(AccessRole::Engineer);
    let denied_reason = (!engineer).then(|| {
        format!(
            "{} role — connect with an engineer token to write or force.",
            capitalize_role(role.as_str())
        )
    });
    json!({
        "role": role.as_str(),
        "io": {
            "write": engineer,
            "force": engineer,
            "release": engineer,
        },
        "hmi": {
            "write": engineer,
        },
        "reason": denied_reason,
    })
}

fn capitalize_role(role: &str) -> String {
    let mut chars = role.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod event_read_contract_tests;
