//! setExpression handling.
//! - handle_set_expression: live runtime path
//! - handle_set_expression_paused: snapshot path
//! - lvalue helpers: parse/write debug lvalues

use serde_json::Value;

use trust_runtime::debug::DebugSnapshot;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{
    coerce_value_to_type, parse_debug_expression_with_abi, parse_debug_lvalue_with_abi,
};
use trust_runtime::io::IoAddress;
use trust_runtime::memory::{FrameId, IoArea};
use trust_runtime::program_model::LValue;
use trust_runtime::value::Value as RuntimeValue;

use crate::adapter::io::{queue_io_request, IoRequest};
use crate::protocol::{
    InvalidatedEventBody, Request, SetExpressionArguments, SetExpressionResponseBody,
};

use super::super::io::{
    format_io_address, io_type_id, resolve_io_address, resolve_io_address_from_state,
};
use super::super::{DebugAdapter, DispatchOutcome};
use super::eval::parse_value_expression;
use super::format::type_id_for_value;
use super::set::{parse_set_directive, SetDirective};
use super::symbolic_force::SymbolicForceDirective;

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
                SetDirective::Write(_) => {
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

        if self.remote_session.is_some() {
            return self.handle_remote_set_expression(request, args, directive);
        }

        let mut frame_id = args.frame_id.map(FrameId);
        let snapshot = self.session.debug_control().snapshot();
        let paused = snapshot.is_some();
        if paused {
            return self.handle_set_expression_paused(request, args, directive, snapshot.unwrap());
        }
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

        if frame_id == Some(FrameId(0)) && runtime.storage().frames().is_empty() {
            frame_id = None;
        }
        if frame_id.is_some() && !paused {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "setExpression requires a paused frame")
                ],
                ..DispatchOutcome::default()
            };
        }

        let expr_text = args.expression.trim();
        let mut events = Vec::new();

        let refresh_frame = frame_id;

        if let Ok(address) = resolve_io_address(&runtime, expr_text) {
            if address.area != IoArea::Input && matches!(&directive, SetDirective::Write(_)) {
                return DispatchOutcome {
                    responses: vec![
                        self.error_response(&request, "only input addresses can be written once")
                    ],
                    ..DispatchOutcome::default()
                };
            }
            let type_id = io_type_id(&address);
            let result = match &directive {
                SetDirective::Release => {
                    self.session.debug_control().release_io(&address);
                    self.set_io_forced(&address, false);
                    let body = self.update_io_cache_from_runtime(&runtime);
                    events.push(self.event("stIoState", Some(body)));
                    runtime.io().read(&address).unwrap_or(RuntimeValue::Null)
                }
                SetDirective::Write(raw) | SetDirective::Force(raw) => {
                    let value = match parse_value_expression(&runtime, raw, frame_id) {
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
                    let io_request = if matches!(&directive, SetDirective::Force(_)) {
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
                    if io_request == IoRequest::Force {
                        self.set_io_forced(&address, true);
                    }
                    let body = self.update_io_cache_for_write(address, coerced.clone());
                    events.push(self.event("stIoState", Some(body)));
                    coerced
                }
            };

            let variable = self.variable_from_value("result".to_string(), result, None);
            let body = SetExpressionResponseBody {
                value: variable.value,
                r#type: variable.r#type,
                variables_reference: variable.variables_reference,
                named_variables: None,
                indexed_variables: None,
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

            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                events,
                should_exit: false,
                stop_gate: None,
            };
        }

        let using = frame_id
            .and_then(|frame_id| runtime.using_for_frame(frame_id))
            .unwrap_or_default();
        let profile = runtime.profile();
        let mut registry = runtime.registry().clone();
        let target = match parse_debug_lvalue_with_abi(
            expr_text,
            &mut registry,
            profile,
            &using,
            self.session.metadata().storage_abi(),
        )
        .map_err(|err| err.to_string())
        .and_then(|target| {
            runtime
                .resolve_debug_lvalue(target, &registry, frame_id)
                .map_err(|err| err.to_string())
        }) {
            Ok(target) => target,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message.to_string())],
                    ..DispatchOutcome::default()
                };
            }
        };

        let map_runtime_error = |err: RuntimeError| match err {
            RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
            _ => err.to_string(),
        };

        let current = match runtime.read_lvalue(&target, frame_id) {
            Ok(value) => value,
            Err(err) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &map_runtime_error(err))],
                    ..DispatchOutcome::default()
                };
            }
        };
        let Some(type_id) = type_id_for_value(&current) else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "unsupported variable type")],
                ..DispatchOutcome::default()
            };
        };
        let final_value = match directive {
            SetDirective::Release => {
                if let Err(message) = self.apply_symbolic_force_directive(
                    runtime.storage(),
                    &target,
                    SymbolicForceDirective::Release,
                    None,
                ) {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    };
                }
                current
            }
            SetDirective::Write(ref raw) | SetDirective::Force(ref raw) => {
                let value = match parse_value_expression(&runtime, raw, frame_id) {
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
                let ads_point = if let LValue::Name(name) = &target {
                    match runtime.ads_live_value_writable(name.as_ref()) {
                        Some(false) => {
                            return DispatchOutcome {
                                responses: vec![self.error_response(
                                    &request,
                                    &format!(
                                        "ADS tag '{name}' is read-only. Enable writes for this tag and restart the runtime."
                                    ),
                                )],
                                ..DispatchOutcome::default()
                            };
                        }
                        Some(true) => Some(name.clone()),
                        None => None,
                    }
                } else {
                    None
                };
                // A force is queued before the direct write, so a full request queue refuses the
                // request before the destination changes.
                let forcing = matches!(directive, SetDirective::Force(_));
                let previous = if forcing {
                    self.symbolic_force_previous(runtime.storage(), &target)
                } else {
                    None
                };
                if forcing
                    && let Err(message) = self.apply_symbolic_force_directive(
                        runtime.storage(),
                        &target,
                        SymbolicForceDirective::Force,
                        Some(coerced.clone()),
                    )
                {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    };
                }
                let written = match ads_point {
                    Some(name) => {
                        runtime.write_global_and_queue_ads(name.as_ref(), coerced.clone())
                    }
                    None => runtime.write_lvalue(&target, coerced.clone(), frame_id),
                };
                if let Err(err) = written {
                    if forcing {
                        // The write did not land, so the target gets back the force it had before.
                        self.restore_symbolic_force(runtime.storage(), &target, previous);
                    }
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }
                coerced
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

        let variable = self.variable_from_value("result".to_string(), final_value, None);
        let body = SetExpressionResponseBody {
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

    fn handle_set_expression_paused(
        &mut self,
        request: Request<Value>,
        args: SetExpressionArguments,
        directive: SetDirective,
        snapshot: DebugSnapshot,
    ) -> DispatchOutcome {
        let mut frame_id = args.frame_id.map(FrameId);
        if frame_id == Some(FrameId(0)) && snapshot.storage.frames().is_empty() {
            frame_id = None;
        }
        if let Some(frame_id) = frame_id {
            let has_frame = snapshot
                .storage
                .frames()
                .iter()
                .any(|frame| frame.id == frame_id);
            if !has_frame {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "unknown frame id")],
                    ..DispatchOutcome::default()
                };
            }
        }

        let expr_text = args.expression.trim();
        let using = frame_id
            .and_then(|frame_id| {
                self.session
                    .metadata()
                    .using_for_frame(&snapshot.storage, frame_id)
            })
            .unwrap_or_default();
        let mut events = Vec::new();

        if let Ok(address) = resolve_io_address_from_state(&self.build_io_state(), expr_text) {
            if address.area != IoArea::Input && matches!(&directive, SetDirective::Write(_)) {
                return DispatchOutcome {
                    responses: vec![
                        self.error_response(&request, "only input addresses can be written once")
                    ],
                    ..DispatchOutcome::default()
                };
            }
            let type_id = io_type_id(&address);
            let result = match &directive {
                SetDirective::Release => {
                    self.session.debug_control().release_io(&address);
                    self.set_io_forced(&address, false);
                    if let Ok(cache) = self.last_io_state.lock()
                        && let Some(state) = cache.clone()
                    {
                        events.push(self.event("stIoState", Some(state)));
                    }
                    RuntimeValue::Null
                }
                SetDirective::Write(raw) | SetDirective::Force(raw) => {
                    let value = match self.parse_value_expression_snapshot(raw, frame_id, &snapshot)
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
                    let queued = if matches!(&directive, SetDirective::Force(_)) {
                        self.session
                            .debug_control()
                            .try_force_io(address.clone(), coerced.clone())
                    } else {
                        self.session
                            .debug_control()
                            .try_enqueue_io_write(address.clone(), coerced.clone())
                    };
                    if let Err(error) = queued {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &error.to_string())],
                            ..DispatchOutcome::default()
                        };
                    }
                    if matches!(&directive, SetDirective::Force(_)) {
                        self.set_io_forced(&address, true);
                    }
                    let body = self.update_io_cache_for_write(address, coerced.clone());
                    events.push(self.event("stIoState", Some(body)));
                    coerced
                }
            };

            let variable = self.variable_from_value("result".to_string(), result, None);
            let body = SetExpressionResponseBody {
                value: variable.value,
                r#type: variable.r#type,
                variables_reference: variable.variables_reference,
                named_variables: None,
                indexed_variables: None,
            };

            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                events,
                should_exit: false,
                stop_gate: None,
            };
        }

        let profile = self.session.metadata().profile();
        let mut registry = self.session.metadata().registry().clone();
        let target = match parse_debug_lvalue_with_abi(
            expr_text,
            &mut registry,
            profile,
            &using,
            self.session.metadata().storage_abi(),
        ) {
            Ok(target) => target,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message.to_string())],
                    ..DispatchOutcome::default()
                };
            }
        };
        let map_runtime_error = |err: RuntimeError| match err {
            RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
            _ => err.to_string(),
        };

        let target = match self.session.debug_control().with_snapshot(|snapshot| {
            snapshot.resolve_lvalue_expressions(
                target,
                &registry,
                Some(self.session.metadata().stdlib()),
                &profile,
                frame_id,
            )
        }) {
            Some(Ok(target)) => target,
            Some(Err(err)) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &map_runtime_error(err))],
                    ..DispatchOutcome::default()
                };
            }
            None => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "no snapshot available")],
                    ..DispatchOutcome::default()
                };
            }
        };
        let target_clone = target.clone();
        let current =
            match self.session.debug_control().with_snapshot(|snapshot| {
                snapshot.read_lvalue(&target, &registry, &profile, frame_id)
            }) {
                Some(Ok(value)) => value,
                Some(Err(err)) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }
                None => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "no snapshot available")],
                        ..DispatchOutcome::default()
                    };
                }
            };

        let Some(type_id) = type_id_for_value(&current) else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "unsupported variable type")],
                ..DispatchOutcome::default()
            };
        };
        let final_value = match directive {
            SetDirective::Release => {
                if let Err(message) = self.apply_symbolic_force_directive(
                    &snapshot.storage,
                    &target_clone,
                    SymbolicForceDirective::Release,
                    None,
                ) {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    };
                }
                current
            }
            SetDirective::Write(raw) => {
                let expr = match parse_debug_expression_with_abi(
                    &raw,
                    &mut registry,
                    profile,
                    &using,
                    snapshot.storage_abi,
                ) {
                    Ok(expr) => expr,
                    Err(err) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err.to_string())],
                            ..DispatchOutcome::default()
                        };
                    }
                };
                let value = match self
                    .evaluate_with_snapshot(&expr, &registry, frame_id, &snapshot, &using)
                {
                    Ok(value) => value,
                    Err(err) => {
                        let message = match err {
                            RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
                            _ => err.to_string(),
                        };
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

                // A full request queue refuses the write before the paused snapshot changes.
                let queued = self.session.debug_control().try_write_lvalue(
                    frame_id,
                    target_clone.clone(),
                    coerced.clone(),
                    |snapshot| {
                        snapshot.write_lvalue(
                            &target_clone,
                            coerced.clone(),
                            &registry,
                            &profile,
                            frame_id,
                        )
                    },
                );
                let refusal = match queued {
                    Ok(Ok(())) => None,
                    Ok(Err(err)) => Some(map_runtime_error(err)),
                    Err(full) => Some(full.to_string()),
                };
                if let Some(message) = refusal {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    };
                }
                coerced
            }
            SetDirective::Force(raw) => {
                let expr = match parse_debug_expression_with_abi(
                    &raw,
                    &mut registry,
                    profile,
                    &using,
                    snapshot.storage_abi,
                ) {
                    Ok(expr) => expr,
                    Err(err) => {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err.to_string())],
                            ..DispatchOutcome::default()
                        };
                    }
                };
                let value = match self
                    .evaluate_with_snapshot(&expr, &registry, frame_id, &snapshot, &using)
                {
                    Ok(value) => value,
                    Err(err) => {
                        let message = match err {
                            RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
                            _ => err.to_string(),
                        };
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

                // The force is queued before the detached snapshot changes, so a full request queue
                // refuses the request without changing what the debugger shows.
                let previous = self.symbolic_force_previous(&snapshot.storage, &target_clone);
                if let Err(message) = self.apply_symbolic_force_directive(
                    &snapshot.storage,
                    &target_clone,
                    SymbolicForceDirective::Force,
                    Some(coerced.clone()),
                ) {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &message)],
                        ..DispatchOutcome::default()
                    };
                }
                let write_result = self.session.debug_control().with_snapshot(|snapshot| {
                    snapshot.write_lvalue(
                        &target_clone,
                        coerced.clone(),
                        &registry,
                        &profile,
                        frame_id,
                    )
                });
                if let Some(Err(err)) = write_result {
                    self.restore_symbolic_force(&snapshot.storage, &target_clone, previous);
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }
                coerced
            }
        };
        events.push(self.event(
            "invalidated",
            Some(InvalidatedEventBody {
                areas: Some(vec!["variables".to_string()]),
                thread_id: None,
                stack_frame_id: frame_id.map(|id| id.0),
            }),
        ));

        let variable = self.variable_from_value("result".to_string(), final_value, None);
        let body = SetExpressionResponseBody {
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
#[path = "expression_queue_refusal_tests.rs"]
mod queue_refusal_tests;
