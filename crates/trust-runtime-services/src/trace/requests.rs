//! The control requests of cycle traces (specification 11 §6.8.1, "Cycle traces"; specification 48
//! RU-59), which every runtime answers alike (specification 34 STATE-08):
//!
//! - `trace.record` starts a recording at the resource's next cycle ([`ResourcePort::trace_record`]);
//! - `trace.read` gives a trace's state, and once it has ended its file in parts of
//!   [`PART_BYTES`] bytes ([`ResourcePort::trace_state`], [`ResourcePort::trace_part`]);
//! - `trace.stop` ends a recording at once ([`ResourcePort::trace_stop`]).

#[cfg(test)]
mod tests;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value as JsonValue};
use trust_runtime_core::crc32;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::trace::TraceEnd;
use trust_runtime_core::value::Duration;

use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;
use crate::resource::replies::resource_not_running;
use crate::status::{invalid_params, validate_object_fields};
use crate::transfer::wire::hex_text;

/// The most bytes of a trace's file one `trace.read` part carries, as a `transfer.part` carries an
/// upload's (specification 34 PORT-05).
pub const PART_BYTES: usize = 4096;
/// The error code of a recording whose memory the runtime cannot give.
pub const INSUFFICIENT_MEMORY: &str = "insufficient_memory";
/// The error code of a recording asked for while another runs.
pub const TRACE_BUSY: &str = "trace_busy";
/// The error code of a trace the runtime does not hold.
pub const UNKNOWN_TRACE: &str = "unknown_trace";

/// What `trace.record` records until: the resource's cycles or logical time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    /// That many cycles, or that many runs of the request's task.
    Cycles(u64),
    /// The cycles within that much logical time from the first.
    Duration(Duration),
}

/// A `trace.record` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRequest {
    /// What the recording records until.
    pub until: Until,
    /// The task whose runs [`Until::Cycles`] counts, by name.
    pub task: Option<String>,
}

/// A recording that started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceStarted {
    /// Its id.
    pub trace: u64,
    /// The bytes it reserved.
    pub reserved_bytes: usize,
}

/// Why a runtime starts no recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceRefusal {
    /// The recording's memory cannot be given: the bytes needed, and the bytes free when known.
    InsufficientMemory {
        /// The bytes the recording needs.
        needed: usize,
        /// The bytes the runtime has free, when it knows them.
        free: Option<usize>,
    },
    /// Another recording runs.
    Busy {
        /// The running recording's id.
        running: u64,
    },
    /// The resource does not run.
    NotRunning(ResourceState),
    /// The program has no task of this name.
    UnknownTask(String),
    /// The runtime records no trace, or its scan thread did not answer: the text says which.
    Unavailable(String),
}

/// A trace as `trace.read` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceName {
    /// A recording on demand, by its id.
    Id(u64),
    /// The flight recorder's trace of the last fault.
    Fault,
}

/// A trace's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceState {
    /// The runtime holds no trace of that name.
    Unknown,
    /// No fault has given the flight recorder's trace yet.
    None,
    /// The recording runs and holds this many cycles.
    Recording {
        /// The cycles recorded so far.
        cycles: u64,
    },
    /// The recording has ended; its file can be read.
    Ended {
        /// Why it ended.
        reason: TraceEnd,
        /// The cycles it holds.
        cycles: u64,
        /// The bytes of its file.
        bytes: usize,
        /// The SHA-256 of its file.
        sha256: [u8; 32],
    },
}

/// Answers `trace.record`: `{"trace": <id>, "reserved_bytes": b}`.
#[must_use]
pub fn record<H: ResourcePort>(id: u64, params: Option<&JsonValue>, host: &H) -> ControlResponse {
    let request = match parse_record(params) {
        Ok(request) => request,
        Err(reason) => return invalid_params(id, &reason),
    };
    match host.trace_record(&request) {
        Ok(started) => ControlResponse::ok(
            id,
            json!({ "trace": started.trace, "reserved_bytes": started.reserved_bytes }),
        ),
        Err(refusal) => refused(id, refusal),
    }
}

/// Answers `trace.read`: the trace's state, or with `offset` the part of its file from there.
#[must_use]
pub fn read<H: ResourcePort>(id: u64, params: Option<&JsonValue>, host: &H) -> ControlResponse {
    let (name, offset) = match parse_read(params) {
        Ok(read) => read,
        Err(reason) => return invalid_params(id, &reason),
    };
    let state = host.trace_state(name);
    let Some(offset) = offset else {
        return state_answer(id, name, state);
    };
    let TraceState::Ended { bytes, .. } = state else {
        return match state {
            TraceState::Unknown => unknown_trace(id, name),
            _ => invalid_params(id, "the trace has not ended: read its parts once it has"),
        };
    };
    if offset >= bytes {
        return invalid_params(
            id,
            &format!("offset {offset} is past the trace's {bytes} bytes"),
        );
    }
    let mut part = vec![0; PART_BYTES.min(bytes - offset)];
    match host.trace_part(name, offset, &mut part) {
        Some(read) if read == part.len() => ControlResponse::ok(id, part_json(offset, &part)),
        _ => unknown_trace(id, name),
    }
}

/// Answers `trace.stop`: the recording ends at once, answered as `trace.read` answers its state.
#[must_use]
pub fn stop<H: ResourcePort>(id: u64, params: Option<&JsonValue>, host: &H) -> ControlResponse {
    let trace = match parse_stop(params) {
        Ok(trace) => trace,
        Err(reason) => return invalid_params(id, &reason),
    };
    let name = TraceName::Id(trace);
    state_answer(id, name, host.trace_stop(trace))
}

/// A trace's state as `trace.read` answers it without an offset.
fn state_answer(id: u64, name: TraceName, state: TraceState) -> ControlResponse {
    match state {
        TraceState::Unknown => unknown_trace(id, name),
        state => ControlResponse::ok(id, state_json(&state)),
    }
}

/// `{"state": "none"}`, `{"state": "recording", "cycles": k}` or `{"state": "ended", "reason",
/// "cycles", "bytes", "sha256"}`.
fn state_json(state: &TraceState) -> JsonValue {
    match state {
        TraceState::Unknown | TraceState::None => json!({ "state": "none" }),
        TraceState::Recording { cycles } => json!({ "state": "recording", "cycles": cycles }),
        TraceState::Ended {
            reason,
            cycles,
            bytes,
            sha256,
        } => json!({
            "state": "ended",
            "reason": reason.name(),
            "cycles": cycles,
            "bytes": bytes,
            "sha256": hex_text(sha256),
        }),
    }
}

/// A part of a trace's file: `{"offset", "data", "crc32", "next_offset"}`.
fn part_json(offset: usize, part: &[u8]) -> JsonValue {
    json!({
        "offset": offset,
        "data": STANDARD.encode(part),
        "crc32": format!("{:08x}", crc32::checksum(part)),
        "next_offset": offset + part.len(),
    })
}

fn refused(id: u64, refusal: TraceRefusal) -> ControlResponse {
    match refusal {
        TraceRefusal::InsufficientMemory { needed, free } => ControlResponse::error_with_code(
            id,
            free.map_or_else(
                || format!("the recording needs {needed} bytes, which this runtime cannot reserve"),
                |free| format!("the recording needs {needed} bytes, and {free} are free"),
            ),
            INSUFFICIENT_MEMORY,
        ),
        TraceRefusal::Busy { running } => ControlResponse::error_with_code(
            id,
            format!("a recording runs: trace {running}; stop it or wait for its end"),
            TRACE_BUSY,
        ),
        TraceRefusal::NotRunning(state) => resource_not_running(id, state),
        TraceRefusal::UnknownTask(task) => {
            invalid_params(id, &format!("the program has no task {task}"))
        }
        TraceRefusal::Unavailable(reason) => ControlResponse::error(id, reason),
    }
}

fn unknown_trace(id: u64, name: TraceName) -> ControlResponse {
    let named = match name {
        TraceName::Id(trace) => format!("no trace {trace}"),
        TraceName::Fault => "no fault trace".to_string(),
    };
    ControlResponse::error_with_code(id, named, UNKNOWN_TRACE)
}

/// `trace.record`'s parameters: exactly one of `cycles` (at least 1) and `duration_ns` (at least
/// 1 ns), and optionally `task`, a task's name.
fn parse_record(params: Option<&JsonValue>) -> Result<RecordRequest, String> {
    let params = params.ok_or("missing params")?;
    validate_object_fields(params, &["cycles", "duration_ns", "task"], &[])?;
    let until = match (params.get("cycles"), params.get("duration_ns")) {
        (Some(cycles), None) => cycles
            .as_u64()
            .filter(|cycles| *cycles > 0)
            .map(Until::Cycles)
            .ok_or_else(|| "cycles must be a whole number of at least 1".to_string())?,
        (None, Some(duration)) => duration
            .as_u64()
            .filter(|nanos| *nanos > 0)
            .and_then(|nanos| i64::try_from(nanos).ok())
            .map(|nanos| Until::Duration(Duration::from_nanos(nanos)))
            .ok_or_else(|| {
                "duration_ns must be a whole number of nanoseconds, at least 1".to_string()
            })?,
        _ => return Err("give exactly one of cycles and duration_ns".to_string()),
    };
    let task = match params.get("task") {
        None => None,
        Some(task) => Some(
            task.as_str()
                .filter(|task| !task.trim().is_empty())
                .map(|task| task.trim().to_string())
                .ok_or_else(|| "task must be a task's name".to_string())?,
        ),
    };
    if task.is_some() && matches!(until, Until::Duration(_)) {
        return Err("task counts cycles: give cycles with it, not duration_ns".to_string());
    }
    Ok(RecordRequest { until, task })
}

/// `trace.read`'s parameters: `trace`, an id or `"fault"`, and optionally `offset`.
fn parse_read(params: Option<&JsonValue>) -> Result<(TraceName, Option<usize>), String> {
    let params = params.ok_or("missing params")?;
    validate_object_fields(params, &["trace", "offset"], &["trace"])?;
    let name = match params.get("trace") {
        Some(JsonValue::String(fault)) if fault == "fault" => TraceName::Fault,
        Some(trace) => TraceName::Id(
            trace
                .as_u64()
                .ok_or_else(|| "trace must be a trace's id or \"fault\"".to_string())?,
        ),
        None => return Err("missing trace".to_string()),
    };
    let offset = match params.get("offset") {
        None => None,
        Some(offset) => Some(
            offset
                .as_u64()
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or_else(|| "offset must be a whole number of bytes".to_string())?,
        ),
    };
    Ok((name, offset))
}

/// `trace.stop`'s parameters: `trace`, a recording's id.
fn parse_stop(params: Option<&JsonValue>) -> Result<u64, String> {
    let params = params.ok_or("missing params")?;
    validate_object_fields(params, &["trace"], &["trace"])?;
    params
        .get("trace")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| "trace must be a recording's id".to_string())
}
