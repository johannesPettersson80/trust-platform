//! A debugger's write of a compiled program's variable (specification 13, "setVariable and
//! setExpression"), the work of `setVariable`, `setExpression` and the Live Values commands that
//! send `setExpression`: the runtime's `debug.set` finds the variable and admits the value, an IEC
//! literal of the variable's type. While the debugger holds the scan it writes the value into the
//! held scan at once; otherwise the write is queued with `set`, by the path the answer names, for
//! the next cycle start. A force (`force: X`) is written at once at a hold too and is held with
//! `var.force`; a release (`release`) ends it with `var.unforce`. A row of the I/O scope is written,
//! forced and released at its address with the I/O requests.

use serde_json::Value;

use trust_runtime::io::IoAddress;

use crate::protocol::{InvalidatedEventBody, Request, SetExpressionResponseBody};

use super::super::remote::writes::{DebugSetReply, WriteTarget};
use super::super::remote::RemoteSession;
use super::super::{DebugAdapter, DispatchOutcome};
use super::set::SetDirective;

/// The refusal of a queued write or a force of a variable without a path: one of a frame on the
/// held call stack, which exists only while the debugger holds the scan.
const FRAME_ONLY: &str =
    "this variable belongs to a frame of the held call stack: it can be written only while the \
     debugger holds the scan, and never forced";

/// What a write did: the value to answer with, its type, and the I/O address it went to, with
/// whether that address is forced now.
struct Written {
    value: String,
    type_name: Option<String>,
    io: Option<(IoAddress, Option<bool>)>,
}

impl Written {
    fn variable(reply: DebugSetReply) -> Self {
        Self {
            value: reply.value.unwrap_or_default(),
            type_name: reply.type_name,
            io: None,
        }
    }
}

impl DebugAdapter {
    /// Writes, forces or releases the variable `target` names, as `directive` and the value text
    /// `raw` ask, and answers with the value the runtime reports. The answer comes with the events
    /// that make the client read the variables again, and the I/O state for an I/O row.
    pub(in crate::adapter) fn write_variable(
        &mut self,
        request: &Request<Value>,
        target: &WriteTarget,
        directive: &SetDirective,
        raw: &str,
    ) -> DispatchOutcome {
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(request);
        };
        let written = match apply(remote, target, directive, raw) {
            Ok(written) => written,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };
        let mut events = vec![self.event(
            "invalidated",
            Some(InvalidatedEventBody {
                areas: Some(vec!["variables".to_string()]),
                thread_id: None,
                stack_frame_id: None,
            }),
        )];
        if let Some((address, forced)) = &written.io {
            if let Some(forced) = forced {
                self.set_io_forced(address, *forced);
            }
            if let Some(event) = self.io_state_event() {
                events.push(event);
            }
        }
        let body = SetExpressionResponseBody {
            value: written.value,
            r#type: written.type_name,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
        };
        DispatchOutcome {
            responses: vec![self.ok_response(request, Some(body))],
            events,
            should_exit: false,
            stop_gate: None,
        }
    }

    /// The `stIoState` event of the runtime's I/O state now, with this session's force marks.
    pub(in crate::adapter) fn io_state_event(&mut self) -> Option<Value> {
        let mut body = self.remote_session.as_mut()?.io_state().ok()?;
        self.apply_forced_flags(&mut body);
        if let Ok(mut cache) = self.last_io_state.lock() {
            *cache = Some(body.clone());
        }
        let shown = crate::adapter::io_values::display_io_state(&body);
        Some(self.event("stIoState", Some(shown)))
    }
}

/// The requests `directive` makes on `target`: `debug.set`, then the queued write, the force or
/// the release by the variable's path, or the I/O request of an I/O row's address.
fn apply(
    remote: &mut RemoteSession,
    target: &WriteTarget,
    directive: &SetDirective,
    raw: &str,
) -> Result<Written, String> {
    match directive {
        SetDirective::Write => {
            let reply = remote.debug_set(target, Some(raw), false)?;
            if let Some(address) = reply.address.as_deref() {
                remote.io_write(address, raw)?;
                return io_written(address, raw.to_string(), None);
            }
            if !reply.written {
                let path = reply.path.as_deref().ok_or(FRAME_ONLY)?;
                remote.var_set(path, raw)?;
            }
            Ok(Written::variable(reply))
        }
        SetDirective::Force(value) => {
            let reply = remote.debug_set(target, Some(value.as_str()), true)?;
            if let Some(address) = reply.address.as_deref() {
                remote.io_force(address, value)?;
                return io_written(address, value.clone(), Some(true));
            }
            let path = reply.path.as_deref().ok_or(FRAME_ONLY)?;
            remote.var_force(path, value)?;
            Ok(Written::variable(reply))
        }
        SetDirective::Release => {
            let reply = remote.debug_set(target, None, false)?;
            if let Some(address) = reply.address.as_deref() {
                remote.io_unforce(address)?;
                return io_written(address, "released".to_string(), Some(false));
            }
            let path = reply.path.as_deref().ok_or(FRAME_ONLY)?;
            remote.var_unforce(path)?;
            Ok(Written::variable(reply))
        }
    }
}

/// What a write at the I/O address `address` did, answered with `value`.
fn io_written(address: &str, value: String, forced: Option<bool>) -> Result<Written, String> {
    let address = IoAddress::parse(address).map_err(|err| err.to_string())?;
    Ok(Written {
        value,
        type_name: None,
        io: Some((address, forced)),
    })
}
