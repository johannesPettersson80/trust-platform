//! The control request pipeline (specification 11, §6.8.1 and §6.9.5).
//!
//! Each request is read, its credential channel stamped and its role resolved; the operation
//! registry and the debug surface are checked; the shared operations are answered here, the debug
//! operations from the platform's debugger when it has one, and every other operation goes to the
//! platform; and every request that could be read, refused or not, gets one audit record, whose id
//! the response carries. For a link that moves frames of a bounded length (specification 34,
//! PORT-05), the reply is written in parts, and a reply longer than the frame is held by the
//! platform's transfer session.

use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::Display;

use serde_json::json;
use smol_str::SmolStr;

use crate::access::{
    is_debug_request, required_role_for_control_request, stamp_credential_channel_param, AccessRole,
};
use crate::capabilities;
use crate::clock;
use crate::cycle_step;
use crate::debug::{answer as answer_debug, is_debug_operation};
use crate::events::{wall_time, RuntimeEvent};
use crate::live_values::{io_requests, var_requests};
use crate::loading::program_install;
use crate::ports::{AuditRecord, ControlHost};
use crate::protocol::{
    read_line, ControlAfterResponse, ControlRequest, ControlResponse, LineError,
};
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

/// Answers one request line as [`prepare_line`] does, for a link that moves frames of at most
/// `max_len` bytes (specification 34, PORT-05). The reply is written in parts of 4,096 bytes, each
/// taken from the heap as the writing reaches it, and never built whole. A reply that fits the
/// frame is the same bytes that [`prepare_line`] writes. A longer one is held, in its parts, by the
/// platform's transfer session and answered with its head, up to the session's capacity. A reply
/// above the capacity is replaced by `reply longer than <capacity> bytes`, and one the heap has no
/// room for by `no room for a reply longer than <n> bytes`, where `n` is the bytes written when it
/// had none. A platform without a session holds nothing, and its capacity is the frame. A reply is
/// never cut, the replacement carries the request's id and audit record id, and the work after the
/// reply stays.
///
/// `events.tail`, `events` and `faults` write their reply event by event, so a reply is never built
/// longer than the writing has taken parts for.
#[must_use]
pub fn prepare_line_within<H: ControlHost>(
    line: &str,
    host: &H,
    client: Option<&str>,
    max_len: usize,
) -> PreparedLine {
    let admitted = match read_line(line) {
        Ok(request) => admit(request, host, client),
        Err(LineError::Unreadable(err)) => {
            return link_reply::within(unreadable_line(&err), max_len, host);
        }
        Err(LineError::NotARequest(err)) => Err(not_a_request(&err, host, client)),
    };
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(refused) => return link_reply::within(refused, max_len, host),
    };
    match EventRead::of(admitted.request.r#type.as_str()) {
        Some(read) => link_reply::event_read_within(read, &admitted.request, host, client, max_len),
        None => link_reply::within(answer(admitted, host, client), max_len, host),
    }
}

/// The line of `response`, a reply that its platform finished after the request's pass, within
/// `max_len` bytes as [`prepare_line_within`] writes a reply.
pub(crate) fn reply_within<H: ControlHost>(
    response: ControlResponse,
    max_len: usize,
    host: &H,
) -> PreparedLine {
    link_reply::within(response, max_len, host)
}

/// The response to one request line, before it is serialized.
fn respond<H: ControlHost>(line: &str, host: &H, client: Option<&str>) -> ControlResponse {
    match read_line(line) {
        Ok(request) => dispatch_request(request, host, client),
        Err(LineError::Unreadable(err)) => unreadable_line(&err),
        Err(LineError::NotARequest(err)) => not_a_request(&err, host, client),
    }
}

/// The answer to a line that is not JSON: `invalid request` with id 0 and no audit record.
fn unreadable_line(err: &serde_json::Error) -> ControlResponse {
    ControlResponse::error(0, format!("invalid request: {err}"))
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
    serde_json::to_string(response).unwrap_or_else(|err| serialization_failed(&err))
}

/// The response to a response that cannot be serialized: `response serialization failed: <error>`
/// with id 0.
fn serialization_failed(error: &impl Display) -> String {
    json!({
        "id": 0_u64,
        "ok": false,
        "error": format!("response serialization failed: {error}"),
    })
    .to_string()
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
    match serde_json::from_value::<ControlRequest>(value) {
        Ok(request) => dispatch_request(request, host, client),
        Err(err) => not_a_request(&err, host, client),
    }
}

/// Answers one request.
fn dispatch_request<H: ControlHost>(
    request: ControlRequest,
    host: &H,
    client: Option<&str>,
) -> ControlResponse {
    match admit(request, host, client) {
        Ok(admitted) => answer(admitted, host, client),
        Err(refused) => refused,
    }
}

/// The answer to JSON that is not a request: `invalid request: <error>` with id 0 and an audit
/// record of type `invalid`.
fn not_a_request<H: ControlHost>(
    err: &serde_json::Error,
    host: &H,
    client: Option<&str>,
) -> ControlResponse {
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
    ControlResponse::error(0, error).with_audit_id(audit_id)
}

/// A request that may be served: it could be read, its role allows it and its surface is
/// enabled.
struct Admitted {
    request: ControlRequest,
    role: AccessRole,
}

/// Checks the access of `request`. A request that is refused is answered and audited here.
fn admit<H: ControlHost>(
    mut request: ControlRequest,
    host: &H,
    client: Option<&str>,
) -> Result<Admitted, ControlResponse> {
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

/// Serves an admitted request, a `status` with the access capabilities of its role, and audits
/// it.
fn answer<H: ControlHost>(admitted: Admitted, host: &H, client: Option<&str>) -> ControlResponse {
    let Admitted { request, role } = admitted;
    let response = if request.r#type == "status" {
        status::status_for(request.id, host, Some(role))
    } else {
        route(&request, host)
            .unwrap_or_else(|| ControlResponse::error(request.id, "unsupported request".into()))
    };
    audited(host, &request, client, response)
}

/// Answers the shared operations and hands every other one to the platform.
fn route<H: ControlHost>(request: &ControlRequest, host: &H) -> Option<ControlResponse> {
    let id = request.id;
    let response = match request.r#type.as_str() {
        "status" => status::status(id, host),
        "capabilities" => capabilities::capabilities(id, host),
        "clock.set" => clock::set(id, request.params.as_ref(), host),
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
        "cycle.step" => cycle_step::request(id, request.params.as_ref(), host),
        "shutdown" => operations::shutdown(id, request.params.as_ref()),
        "program.install" => program_install::request(id, request.params.as_ref(), host),
        "transfer.begin" | "transfer.part" | "transfer.status" | "transfer.get"
        | "transfer.abort" => transfers::answer_transfer(request, host),
        "program.active" => transfers::program_active(id, host),
        // The debug operations are answered from the platform's debugger when it has one
        // (specification 11 §6.13); otherwise the platform answers them itself.
        kind if is_debug_operation(kind) => match host.debugger() {
            Some(debugger) => answer_debug(request, debugger),
            None => return host.dispatch_extension(request),
        },
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

mod link_reply;
pub(crate) mod transfers;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod event_read_contract_tests;
