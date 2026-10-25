//! setExpression handling: an I/O address forced or released on the connected runtime.

use serde_json::Value;

use trust_runtime::io::IoAddress;

use crate::protocol::{Request, SetExpressionArguments, SetExpressionResponseBody};

use super::super::io::{format_io_address, resolve_io_address_from_state};
use super::super::{DebugAdapter, DispatchOutcome};
use super::set::{parse_set_directive, SetDirective};

impl DebugAdapter {
    fn handle_remote_set_expression(
        &mut self,
        request: Request<Value>,
        args: SetExpressionArguments,
        directive: SetDirective,
    ) -> DispatchOutcome {
        let expr_text = args.expression.trim();
        let address = if let Ok(address) = IoAddress::parse(expr_text) {
            address
        } else {
            let state = {
                let Some(remote) = self.remote_session.as_mut() else {
                    return DispatchOutcome {
                        responses: vec![
                            self.error_response(&request, "attach session is not connected")
                        ],
                        ..DispatchOutcome::default()
                    };
                };
                match remote.io_state() {
                    Ok(state) => state,
                    Err(err) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err)],
                            ..DispatchOutcome::default()
                        }
                    }
                }
            };
            match resolve_io_address_from_state(&state, expr_text) {
                Ok(address) => address,
                Err(message) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    }
                }
            }
        };
        let address_text = format_io_address(&address);

        let (response_value, forced_state) = {
            let Some(remote) = self.remote_session.as_mut() else {
                return DispatchOutcome {
                    responses: vec![
                        self.error_response(&request, "attach session is not connected")
                    ],
                    ..DispatchOutcome::default()
                };
            };
            match directive {
                SetDirective::Force(raw) => match remote.io_force(&address_text, &raw) {
                    Ok(()) => (raw, Some(true)),
                    Err(err) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err)],
                            ..DispatchOutcome::default()
                        }
                    }
                },
                SetDirective::Release => match remote.io_unforce(&address_text) {
                    Ok(()) => ("released".to_string(), Some(false)),
                    Err(err) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err)],
                            ..DispatchOutcome::default()
                        }
                    }
                },
                SetDirective::Write => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(
                            &request,
                            "setExpression writes are not supported in attach mode; use stIoWrite",
                        )],
                        ..DispatchOutcome::default()
                    }
                }
            }
        };
        if let Some(forced) = forced_state {
            self.set_io_forced(&address, forced);
        }

        let mut events = Vec::new();
        let refreshed = self
            .remote_session
            .as_mut()
            .and_then(|remote| remote.io_state().ok());
        if let Some(mut body) = refreshed {
            self.apply_forced_flags(&mut body);
            if let Ok(mut cache) = self.last_io_state.lock() {
                *cache = Some(body.clone());
            }
            events.push(self.event("stIoState", Some(body)));
        }

        let body = SetExpressionResponseBody {
            value: response_value,
            r#type: None,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
        };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            events,
            should_exit: false,
            stop_gate: None,
        }
    }

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
        self.handle_remote_set_expression(request, args, directive)
    }
}
