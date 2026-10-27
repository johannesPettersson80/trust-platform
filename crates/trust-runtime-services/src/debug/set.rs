//! `debug.set`: a debugger's write of a variable (specification 13, "setVariable and
//! setExpression"; specification 44 §13.2; specification 11 §6.13.4).
//!
//! The request names its variable as a listing showed it, by the listing's handle and the name it
//! showed, or by a path (`Main.Pumps[2].Speed`), whose first name is looked up among the variables
//! of the held frame `frame_id` before the program's names. With a `value`, an IEC 61131-3 literal
//! of the variable's type ([`typed_value`]), the write goes into the scan at once while the
//! debugger holds one, so that the statements after the stop see it and the next read shows it.
//! Between scans nothing is written: the value is checked against the program's types, and the
//! client queues it with `set`, or forces it with `var.force`, by the path the answer carries, for
//! the next cycle start (specification 11, "Live Values"). Without a value the variable is only
//! read, which gives a client the path to release a force by. A variable of a frame on the held
//! call stack has no path, so no force can hold it, and `force` refuses it. A row of the I/O scope
//! is answered with its address, which the client writes with the I/O requests.
//!
//! The operation is planned, written and answered in three steps, so that a platform whose scan
//! waits on another thread hands the write to that thread between them ([`plan`], [`finish`]); a
//! platform that serves its requests on the held scan's own thread writes through
//! [`Debugger::write_held`].

use alloc::format;
use alloc::string::{String, ToString};

use serde::Deserialize;
use serde_json::json;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::memory::{read_place, ProgramMemory};
use trust_runtime_core::native::reflect::Located as VariableAt;
use trust_runtime_core::value::Value;

use super::compiled::{held_frame, listed, named, WriteTarget};
use super::projection::{format_value, value_type_name};
use super::replies;
use super::requests::{failed, parse, Debugger};
use crate::live_values::io_requests::IoSnapshotRead;
use crate::live_values::value_grammar::typed_value;
use crate::protocol::{ControlRequest, ControlResponse};

/// The kind of the operation.
pub const OPERATION: &str = "debug.set";

/// The parameters of `debug.set`: the variable, by a listing's handle and the name it showed or by
/// a path; the value, absent for a read; and whether the client forces the variable.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    #[serde(default)]
    variables_reference: Option<u32>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    frame_id: Option<u32>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    force: bool,
}

/// A `debug.set` whose variable is found and whose value is admitted, waiting for its write.
#[derive(Debug, Clone)]
pub struct SetPlan {
    id: u64,
    at: VariableAt,
    path: Option<String>,
    value: Option<Value>,
}

impl SetPlan {
    /// Where the value goes and the value, when the request writes one.
    #[must_use]
    pub fn write(&self) -> Option<(&VariableAt, &Value)> {
        self.value.as_ref().map(|value| (&self.at, value))
    }
}

/// What `debug.set` asks of the platform once it is planned.
pub enum Planned {
    /// The answer, complete: a refusal, or the address of an I/O row.
    Answer(ControlResponse),
    /// The variable found and the value admitted: the platform writes it into the held scan when
    /// it holds one, and the operation then answers with [`finish`].
    Write(SetPlan),
}

/// Plans `request`, a `debug.set`, over the memory `debugger` reads: the variable found, checked
/// as a write and with its value admitted, or the answer that refuses it.
#[must_use]
pub fn plan(request: &ControlRequest, debugger: &dyn Debugger) -> Planned {
    let id = request.id;
    let params: SetParams = match parse(request.params.as_ref()) {
        Ok(params) => params,
        Err(error) => return Planned::Answer(failed(id, error)),
    };
    let Some(memory) = debugger.program_memory() else {
        return Planned::Answer(failed(id, replies::NO_SNAPSHOT));
    };
    let target = match (&params.variables_reference, &params.name, &params.path) {
        (Some(reference), Some(name), None) => listed_target(debugger, memory, *reference, name),
        (None, None, Some(path)) => {
            // Between scans no frame is held, and a path names the program's variables.
            let frame = match params.frame_id {
                Some(frame_id) if !memory.frames().is_empty() => {
                    match held_frame(memory, frame_id) {
                        Ok(index) => Some(index),
                        Err(error) => return Planned::Answer(failed(id, error)),
                    }
                }
                _ => None,
            };
            named(memory, path, frame)
        }
        _ => Err(String::from(
            "debug.set names its variable by variables_reference and name, or by path",
        )),
    };
    let (at, path) = match target {
        Ok(WriteTarget::Variable { at, path }) => (at, path),
        Ok(WriteTarget::Io { address }) => {
            return Planned::Answer(ControlResponse::ok(
                id,
                json!({
                    "address": address.to_string(),
                    "path": null,
                    "written": false,
                }),
            ));
        }
        Err(reason) => return Planned::Answer(failed(id, reason)),
    };
    if params.force && path.is_none() {
        return Planned::Answer(failed(
            id,
            "a variable of a frame on the held call stack cannot be forced: write it instead",
        ));
    }
    let value = match params.value.as_deref() {
        Some(text) => match typed_value(&memory.image(), at.type_index, text) {
            Ok(value) => Some(value),
            Err(reason) => return Planned::Answer(failed(id, reason)),
        },
        None => None,
    };
    Planned::Write(SetPlan {
        id,
        at,
        path,
        value,
    })
}

/// The variable named `name` that the listing of the handle `reference` shows.
fn listed_target(
    debugger: &dyn Debugger,
    memory: &dyn ProgramMemory,
    reference: u32,
    name: &str,
) -> Result<WriteTarget, String> {
    let mut handle = None;
    debugger.with_handles(&mut |handles| handle = handles.get(reference).cloned());
    let handle = handle.ok_or_else(|| format!("unknown variables reference {reference}"))?;
    let io = match debugger.io_snapshot() {
        IoSnapshotRead::Published(io) => Some(io),
        IoSnapshotRead::Missing | IoSnapshotRead::Unavailable => None,
    };
    listed(memory, io.as_ref(), &handle, name)
}

/// Answers the planned `debug.set` once the platform has written it into the held scan
/// (`written`), or has not because no scan is held (`None`): the variable's path, type and value,
/// the value it holds now after a write, the admitted value otherwise, and whether it was written.
#[must_use]
pub fn finish(
    plan: &SetPlan,
    written: Option<Result<(), RuntimeError>>,
    debugger: &dyn Debugger,
) -> ControlResponse {
    let applied = matches!(written, Some(Ok(())));
    let value = match (written, &plan.value) {
        (Some(Err(error)), _) => return failed(plan.id, format!("the write was refused: {error}")),
        (Some(Ok(())), _) | (None, None) => {
            let Some(memory) = debugger.program_memory() else {
                return failed(plan.id, replies::NO_SNAPSHOT);
            };
            match read_place(memory, plan.at.place, plan.at.type_index) {
                Ok(value) => value,
                Err(error) => return failed(plan.id, format!("{error}")),
            }
        }
        (None, Some(value)) => value.clone(),
    };
    ControlResponse::ok(
        plan.id,
        json!({
            "path": plan.path,
            "type": value_type_name(&value),
            "value": format_value(&value),
            "written": applied,
        }),
    )
}

/// Answers `debug.set` on a platform that serves its requests on the held scan's own thread.
pub(super) fn answer(request: &ControlRequest, debugger: &dyn Debugger) -> ControlResponse {
    match plan(request, debugger) {
        Planned::Answer(response) => response,
        Planned::Write(plan) => {
            let written = plan
                .write()
                .and_then(|(at, value)| debugger.write_held(at, value));
            finish(&plan, written, debugger)
        }
    }
}
