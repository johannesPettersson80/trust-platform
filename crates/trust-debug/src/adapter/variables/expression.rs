//! setExpression handling (specification 13, "setVariable and setExpression"): an I/O address, or
//! the name of an I/O row, written, forced or released at its address on the connected runtime; any
//! other expression is a variable path (`Main.Pumps[2].Speed`), written, forced or released as
//! [`DebugAdapter::write_variable`] does.

use serde_json::Value;

use trust_runtime::io::IoAddress;

use crate::protocol::{Request, SetExpressionArguments, SetExpressionResponseBody};

use super::super::io::resolve_io_address_from_state;
use super::super::remote::writes::WriteTarget;
use super::super::{DebugAdapter, DispatchOutcome};
use super::set::{parse_set_directive, SetDirective};

impl DebugAdapter {
    /// The I/O address `expression` names: an address, or the name of a row of the runtime's I/O
    /// state; `None` for any other expression, which is a variable path.
    fn io_expression(&mut self, expression: &str) -> Option<IoAddress> {
        if let Ok(address) = IoAddress::parse(expression) {
            return Some(address);
        }
        let state = self.remote_session.as_mut()?.io_state().ok()?;
        state
            .inputs
            .iter()
            .chain(state.outputs.iter())
            .chain(state.memory.iter())
            .any(|entry| entry.name.as_deref() == Some(expression))
            .then(|| resolve_io_address_from_state(&state, expression).ok())
            .flatten()
    }

    /// Writes, forces or releases the I/O `address` on the connected runtime.
    fn set_io_expression(
        &mut self,
        request: &Request<Value>,
        address: &IoAddress,
        directive: SetDirective,
        raw: &str,
    ) -> DispatchOutcome {
        let address_text = address.to_string();
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(request);
        };
        let answered = match directive {
            SetDirective::Write => remote
                .io_write(&address_text, raw)
                .map(|()| (raw.to_string(), None)),
            SetDirective::Force(value) => remote
                .io_force(&address_text, &value)
                .map(|()| (value, Some(true))),
            SetDirective::Release => remote
                .io_unforce(&address_text)
                .map(|()| ("released".to_string(), Some(false))),
        };
        let (response_value, forced) = match answered {
            Ok(answered) => answered,
            Err(err) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(request, &err)],
                    ..DispatchOutcome::default()
                }
            }
        };
        if let Some(forced) = forced {
            self.set_io_forced(address, forced);
        }
        let events = self.io_state_event().into_iter().collect();
        let body = SetExpressionResponseBody {
            value: response_value,
            r#type: None,
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

    /// `setExpression`: the value text is an IEC literal of the expression's type, `force: X`
    /// forces it and `release` releases its force; a variable path's first name is looked up among
    /// the variables of the request's frame first while the debugger holds the scan.
    pub(in crate::adapter) fn handle_set_expression(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<SetExpressionArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid setExpression args")],
                ..DispatchOutcome::default()
            };
        };

        let directive = match parse_set_directive(&args.value) {
            Ok(directive) => directive,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };

        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        let expression = args.expression.trim();
        if let Some(address) = self.io_expression(expression) {
            return self.set_io_expression(&request, &address, directive, args.value.trim());
        }
        let target = WriteTarget::Path {
            path: expression.to_string(),
            frame_id: args.frame_id,
        };
        self.write_variable(&request, &target, &directive, args.value.trim())
    }
}
