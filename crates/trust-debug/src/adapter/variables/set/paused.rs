//! setVariable handling when paused.
//! - handle_set_variable_paused: apply writes to snapshot + emit invalidation

use serde_json::Value;

use trust_runtime::debug::DebugSnapshot;
use trust_runtime::harness::coerce_value_to_type;
use trust_runtime::memory::IoArea;
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{
    InvalidatedEventBody, Request, SetVariableArguments, SetVariableResponseBody,
};

use super::super::super::io::{io_type_id, resolve_io_address_from_state};
use super::super::super::{DebugAdapter, DispatchOutcome, VariableHandle};
use super::super::format::type_id_for_value;
use super::SetDirective;

impl DebugAdapter {
    pub(super) fn handle_set_variable_paused(
        &mut self,
        request: Request<Value>,
        args: SetVariableArguments,
        handle: VariableHandle,
        directive: SetDirective,
        force_requested: bool,
        snapshot: DebugSnapshot,
    ) -> DispatchOutcome {
        let apply_value =
            |value: RuntimeValue, target: &RuntimeValue| -> Result<RuntimeValue, String> {
                let Some(type_id) = type_id_for_value(target) else {
                    return Err("unsupported variable type".to_string());
                };
                coerce_value_to_type(value, type_id).map_err(|err| err.to_string())
            };

        let refresh_frame = match &handle {
            VariableHandle::Locals(frame_id) => Some(*frame_id),
            _ => None,
        };
        let is_io_handle = matches!(
            handle,
            VariableHandle::IoInputs
                | VariableHandle::IoOutputs
                | VariableHandle::IoMemory
                | VariableHandle::IoRoot
        );
        let mut events = Vec::new();

        let result = match handle {
            VariableHandle::Locals(frame_id) => {
                let frame = snapshot
                    .storage
                    .frames()
                    .iter()
                    .find(|frame| frame.id == frame_id)
                    .cloned();
                let Some(frame) = frame else {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "unknown frame id")],
                        ..DispatchOutcome::default()
                    };
                };
                let instance_id = frame.instance_id;
                let local_value = frame.variables.get(args.name.as_str()).cloned();
                let instance_value = instance_id
                    .and_then(|id| snapshot.storage.get_instance_var(id, args.name.as_str()))
                    .cloned();
                if local_value.is_none() && instance_value.is_none() {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "unknown local variable")],
                        ..DispatchOutcome::default()
                    };
                }
                match &directive {
                    SetDirective::Release => {
                        if local_value.is_some() {
                            return DispatchOutcome {
                                responses: vec![self
                                    .error_response(&request, "local variables cannot be forced")],
                                ..DispatchOutcome::default()
                            };
                        }
                        if let Some(instance_id) = instance_id {
                            self.session
                                .debug_control()
                                .release_instance(instance_id, &args.name);
                        }
                        let Some(value) = instance_value else {
                            return DispatchOutcome {
                                responses: vec![
                                    self.error_response(&request, "unknown local variable")
                                ],
                                ..DispatchOutcome::default()
                            };
                        };
                        value
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match self.parse_value_expression_snapshot(
                            raw,
                            Some(frame_id),
                            &snapshot,
                        ) {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        if let Some(current) = local_value.clone() {
                            if force_requested {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(
                                        &request,
                                        "local variables cannot be forced",
                                    )],
                                    ..DispatchOutcome::default()
                                };
                            }
                            let coerced = match apply_value(value, &current) {
                                Ok(value) => value,
                                Err(message) => {
                                    return DispatchOutcome {
                                        responses: vec![self.error_response(&request, &message)],
                                        ..DispatchOutcome::default()
                                    };
                                }
                            };
                            self.session.debug_control().enqueue_local_write(
                                frame_id,
                                args.name.clone(),
                                coerced.clone(),
                            );
                            let _ = self.session.debug_control().with_snapshot(|snapshot| {
                                snapshot.storage.with_frame(frame_id, |storage| {
                                    storage.set_local(args.name.clone(), coerced.clone());
                                })
                            });
                            coerced
                        } else if let Some(current) = instance_value.clone() {
                            let coerced = match apply_value(value, &current) {
                                Ok(value) => value,
                                Err(message) => {
                                    return DispatchOutcome {
                                        responses: vec![self.error_response(&request, &message)],
                                        ..DispatchOutcome::default()
                                    };
                                }
                            };
                            let instance_id = instance_id.unwrap();
                            if force_requested {
                                self.session.debug_control().force_instance(
                                    instance_id,
                                    args.name.clone(),
                                    coerced.clone(),
                                );
                            } else {
                                self.session.debug_control().enqueue_instance_write(
                                    instance_id,
                                    args.name.clone(),
                                    coerced.clone(),
                                );
                            }
                            let _ = self.session.debug_control().with_snapshot(|snapshot| {
                                snapshot.storage.set_instance_var(
                                    instance_id,
                                    args.name.clone(),
                                    coerced.clone(),
                                );
                            });
                            coerced
                        } else {
                            return DispatchOutcome {
                                responses: vec![
                                    self.error_response(&request, "unknown local variable")
                                ],
                                ..DispatchOutcome::default()
                            };
                        }
                    }
                }
            }
            VariableHandle::Globals => {
                let current = match snapshot.storage.get_global(args.name.as_str()) {
                    Some(value) => value.clone(),
                    None => {
                        return DispatchOutcome {
                            responses: vec![
                                self.error_response(&request, "unknown global variable")
                            ],
                            ..DispatchOutcome::default()
                        };
                    }
                };
                match &directive {
                    SetDirective::Release => {
                        self.session.debug_control().release_global(&args.name);
                        current
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match self.parse_value_expression_snapshot(raw, None, &snapshot)
                        {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        let coerced = match apply_value(value, &current) {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        if force_requested {
                            self.session
                                .debug_control()
                                .force_global(args.name.clone(), coerced.clone());
                        } else {
                            self.session
                                .debug_control()
                                .enqueue_global_write(args.name.clone(), coerced.clone());
                        }
                        let _ = self.session.debug_control().with_snapshot(|snapshot| {
                            snapshot
                                .storage
                                .set_global(args.name.clone(), coerced.clone());
                        });
                        coerced
                    }
                }
            }
            VariableHandle::Retain => {
                let current = match snapshot.storage.get_retain(args.name.as_str()) {
                    Some(value) => value.clone(),
                    None => {
                        return DispatchOutcome {
                            responses: vec![
                                self.error_response(&request, "unknown retain variable")
                            ],
                            ..DispatchOutcome::default()
                        };
                    }
                };
                match &directive {
                    SetDirective::Release => {
                        self.session.debug_control().release_retain(&args.name);
                        current
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match self.parse_value_expression_snapshot(raw, None, &snapshot)
                        {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        let coerced = match apply_value(value, &current) {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        if force_requested {
                            self.session
                                .debug_control()
                                .force_retain(args.name.clone(), coerced.clone());
                        } else {
                            self.session
                                .debug_control()
                                .enqueue_retain_write(args.name.clone(), coerced.clone());
                        }
                        let _ = self.session.debug_control().with_snapshot(|snapshot| {
                            snapshot
                                .storage
                                .set_retain(args.name.clone(), coerced.clone());
                        });
                        coerced
                    }
                }
            }
            VariableHandle::Instance(instance_id) => {
                if args.name == "parent" {
                    return DispatchOutcome {
                        responses: vec![
                            self.error_response(&request, "parent instance is read-only")
                        ],
                        ..DispatchOutcome::default()
                    };
                }
                let current = match snapshot
                    .storage
                    .get_instance_var(instance_id, args.name.as_str())
                {
                    Some(value) => value.clone(),
                    None => {
                        return DispatchOutcome {
                            responses: vec![
                                self.error_response(&request, "unknown instance variable")
                            ],
                            ..DispatchOutcome::default()
                        };
                    }
                };
                match &directive {
                    SetDirective::Release => {
                        self.session
                            .debug_control()
                            .release_instance(instance_id, &args.name);
                        current
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match self.parse_value_expression_snapshot(raw, None, &snapshot)
                        {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        let coerced = match apply_value(value, &current) {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        if force_requested {
                            self.session.debug_control().force_instance(
                                instance_id,
                                args.name.clone(),
                                coerced.clone(),
                            );
                        } else {
                            self.session.debug_control().enqueue_instance_write(
                                instance_id,
                                args.name.clone(),
                                coerced.clone(),
                            );
                        }
                        let _ = self.session.debug_control().with_snapshot(|snapshot| {
                            snapshot.storage.set_instance_var(
                                instance_id,
                                args.name.clone(),
                                coerced.clone(),
                            );
                        });
                        coerced
                    }
                }
            }
            VariableHandle::IoInputs
            | VariableHandle::IoOutputs
            | VariableHandle::IoMemory
            | VariableHandle::IoRoot => {
                let state = self.build_io_state();
                let address = match resolve_io_address_from_state(&state, &args.name) {
                    Ok(address) => address,
                    Err(message) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &message)],
                            ..DispatchOutcome::default()
                        }
                    }
                };
                if address.area != IoArea::Input && matches!(&directive, SetDirective::Write(_)) {
                    return DispatchOutcome {
                        responses: vec![self
                            .error_response(&request, "only input addresses can be written once")],
                        ..DispatchOutcome::default()
                    };
                }
                let type_id = io_type_id(&address);
                match &directive {
                    SetDirective::Release => {
                        self.session.debug_control().release_io(&address);
                        self.set_io_forced(&address, false);
                        RuntimeValue::Null
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match self.parse_value_expression_snapshot(raw, None, &snapshot)
                        {
                            Ok(value) => value,
                            Err(message) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &message)],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        let coerced = match coerce_value_to_type(value, type_id) {
                            Ok(value) => value,
                            Err(err) => {
                                return DispatchOutcome {
                                    responses: vec![self.error_response(&request, &err.to_string())],
                                    ..DispatchOutcome::default()
                                };
                            }
                        };
                        if force_requested {
                            self.session
                                .debug_control()
                                .force_io(address.clone(), coerced.clone());
                            self.set_io_forced(&address, true);
                        } else {
                            self.session
                                .debug_control()
                                .enqueue_io_write(address.clone(), coerced.clone());
                        }
                        let body = self.update_io_cache_for_write(address, coerced.clone());
                        events.push(self.event("stIoState", Some(body)));
                        coerced
                    }
                }
            }
            VariableHandle::Struct(_)
            | VariableHandle::Array(_)
            | VariableHandle::Reference(_)
            | VariableHandle::Instances => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "this variable cannot be edited")],
                    ..DispatchOutcome::default()
                };
            }
        };

        if !is_io_handle {
            events.push(self.event(
                "invalidated",
                Some(InvalidatedEventBody {
                    areas: Some(vec!["variables".to_string()]),
                    thread_id: None,
                    stack_frame_id: refresh_frame.map(|id| id.0),
                }),
            ));
        }

        let variable = self.variable_from_value("result".to_string(), result, None);
        let body = SetVariableResponseBody {
            value: variable.value,
            r#type: variable.r#type,
            variables_reference: variable.variables_reference,
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
}
