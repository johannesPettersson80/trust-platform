//! setVariable handling for live runtime.
//! - handle_set_variable: parse args and write values (or delegate to paused)

use serde_json::Value;

use trust_runtime::debug::ForcedVarTarget;
use trust_runtime::harness::coerce_value_to_type;
use trust_runtime::memory::IoArea;
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{
    InvalidatedEventBody, Request, SetVariableArguments, SetVariableResponseBody,
};

use super::super::super::io::{io_type_id, queue_io_request, resolve_io_address, IoRequest};
use super::super::super::{DebugAdapter, DispatchOutcome, VariableHandle};
use super::super::eval::parse_value_expression;
use super::super::format::type_id_for_value;
use super::{parse_set_directive, SetDirective};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_set_variable(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<SetVariableArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid setVariable args")],
                ..DispatchOutcome::default()
            };
        };

        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "setVariable not supported in attach mode")
                ],
                ..DispatchOutcome::default()
            };
        }

        let Some(handle) = self
            .variable_handles
            .get(&args.variables_reference)
            .cloned()
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "unknown variables reference")],
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
        let force_requested = matches!(directive, SetDirective::Force(_));

        let snapshot = self.session.debug_control().snapshot();
        let paused = snapshot.is_some();
        if paused {
            return self.handle_set_variable_paused(
                request,
                args,
                handle,
                directive,
                force_requested,
                snapshot.unwrap(),
            );
        }
        let mut events = Vec::new();
        let runtime_handle = self.session.runtime_handle();
        let mut runtime = match runtime_handle.try_lock() {
            Ok(runtime) => runtime,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "runtime busy")],
                    ..DispatchOutcome::default()
                };
            }
        };

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

        let result = match handle {
            VariableHandle::Locals(_) => {
                // Paused locals are handled against the detached snapshot above.
                // An installed controller never exposes a mutable frame here.
                return DispatchOutcome {
                    responses: vec![self.error_response(
                        &request,
                        "local variables can only be edited when paused",
                    )],
                    ..DispatchOutcome::default()
                };
            }
            VariableHandle::Globals => {
                let current = match runtime.storage().get_global(args.name.as_str()) {
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
                        let value = match parse_value_expression(&runtime, raw, None) {
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
                        if runtime.ads_live_value_writable(args.name.as_str()) == Some(false) {
                            return DispatchOutcome {
                                responses: vec![self.error_response(
                                    &request,
                                    &format!("ADS tag '{}' is read-only. Enable writes for this tag and restart the runtime.", args.name),
                                )],
                                ..DispatchOutcome::default()
                            };
                        }
                        // A force is queued before the write, so a full request queue refuses the
                        // request before the destination changes.
                        let debug = self.session.debug_control();
                        let target = ForcedVarTarget::Global(args.name.as_str().into());
                        let previous = if force_requested {
                            debug.forced_var_value(&target)
                        } else {
                            None
                        };
                        if force_requested
                            && let Err(error) =
                                debug.try_force_global(args.name.clone(), coerced.clone())
                        {
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
                        if let Err(error) =
                            runtime.write_global_and_queue_ads(args.name.as_str(), coerced.clone())
                        {
                            if force_requested {
                                debug.restore_var_force(&target, previous);
                            }
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
                        coerced
                    }
                }
            }
            VariableHandle::Retain => {
                let reference = super::super::retained::reference(
                    self.session.metadata(),
                    runtime.storage(),
                    args.name.as_str(),
                );
                let Some(reference) = reference else {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "unknown retain variable")],
                        ..DispatchOutcome::default()
                    };
                };
                let current = match runtime.storage().materialize_by_ref_ref(&reference) {
                    Some(value) => value,
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
                        if let Err(error) = super::super::retained::force(
                            &self.session.debug_control(),
                            runtime.storage(),
                            &reference,
                            None,
                        ) {
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
                        current
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match parse_value_expression(&runtime, raw, None) {
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
                        // A force is queued before the write, so a full request queue refuses the
                        // request before the destination changes.
                        let debug = self.session.debug_control();
                        let previous = if force_requested {
                            match super::super::retained::force(
                                &debug,
                                runtime.storage(),
                                &reference,
                                Some(coerced.clone()),
                            ) {
                                Ok(previous) => previous,
                                Err(error) => {
                                    return DispatchOutcome {
                                        responses: vec![self.error_response(&request, &error)],
                                        ..DispatchOutcome::default()
                                    };
                                }
                            }
                        } else {
                            None
                        };
                        if let Err(error) = runtime.write_reference(&reference, coerced.clone()) {
                            if force_requested {
                                let _ = super::super::retained::restore_force(
                                    &debug,
                                    runtime.storage(),
                                    &reference,
                                    previous,
                                );
                            }
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
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
                let current = match runtime
                    .storage()
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
                        let value = match parse_value_expression(&runtime, raw, None) {
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
                        let reference = runtime
                            .storage()
                            .ref_for_instance_recursive(instance_id, args.name.as_str());
                        // A force is queued before the write, so a full request queue refuses the
                        // request before the destination changes.
                        let debug = self.session.debug_control();
                        let target =
                            ForcedVarTarget::Instance(instance_id, args.name.as_str().into());
                        let previous = if force_requested {
                            debug.forced_var_value(&target)
                        } else {
                            None
                        };
                        if force_requested
                            && let Err(error) = debug.try_force_instance(
                                instance_id,
                                args.name.clone(),
                                coerced.clone(),
                            )
                        {
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
                        let result = reference
                            .ok_or(trust_runtime::error::RuntimeError::ReferenceLifetime)
                            .and_then(|reference| {
                                runtime.write_reference(&reference, coerced.clone())
                            });
                        if let Err(error) = result {
                            if force_requested {
                                debug.restore_var_force(&target, previous);
                            }
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &error.to_string())],
                                ..DispatchOutcome::default()
                            };
                        }
                        coerced
                    }
                }
            }
            VariableHandle::IoInputs
            | VariableHandle::IoOutputs
            | VariableHandle::IoMemory
            | VariableHandle::IoRoot => {
                let address = match resolve_io_address(&runtime, &args.name) {
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
                        let body = self.update_io_cache_from_runtime(&runtime);
                        events.push(self.event("stIoState", Some(body)));
                        runtime.io().read(&address).unwrap_or(RuntimeValue::Null)
                    }
                    SetDirective::Write(raw) | SetDirective::Force(raw) => {
                        let value = match parse_value_expression(&runtime, raw, None) {
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
                        let io_request = if force_requested {
                            IoRequest::Force
                        } else {
                            IoRequest::Write
                        };
                        if let Err(message) = queue_io_request(
                            &self.session.debug_control(),
                            &mut runtime,
                            &address,
                            &coerced,
                            io_request,
                        ) {
                            return DispatchOutcome {
                                responses: vec![self.error_response(&request, &message)],
                                ..DispatchOutcome::default()
                            };
                        }
                        if force_requested {
                            self.set_io_forced(&address, true);
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

        if paused {
            self.session
                .debug_control()
                .refresh_snapshot_from_storage_with_abi(
                    runtime.storage(),
                    runtime.current_time(),
                    runtime.prepared().storage_abi(),
                );
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

#[cfg(test)]
#[path = "request_queue_refusal_tests.rs"]
mod queue_refusal_tests;
