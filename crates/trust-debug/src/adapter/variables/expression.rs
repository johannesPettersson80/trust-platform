//! setExpression handling.
//! - handle_set_expression: live runtime path
//! - handle_set_expression_paused: snapshot path
//! - lvalue helpers: parse/write debug lvalues

use serde_json::Value;

use trust_runtime::debug::DebugSnapshot;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{coerce_value_to_type, parse_debug_expression, parse_debug_lvalue};
use trust_runtime::memory::{FrameId, IoArea, VariableStorage};
use trust_runtime::program_model::LValue;
use trust_runtime::value::Value as RuntimeValue;
use trust_runtime::Runtime;

use crate::protocol::{
    InvalidatedEventBody, Request, SetExpressionArguments, SetExpressionResponseBody,
};

use super::super::io::{io_type_id, resolve_io_address, resolve_io_address_from_state};
use super::super::{DebugAdapter, DispatchOutcome};
use super::eval::parse_value_expression;
use super::format::type_id_for_value;
use super::set::{parse_set_directive, SetDirective};

#[derive(Clone, Copy)]
enum SymbolicForceDirective {
    Force,
    Release,
}

impl DebugAdapter {
    fn apply_symbolic_force_directive(
        &self,
        runtime: &Runtime,
        target: &LValue,
        directive: SymbolicForceDirective,
        value: Option<RuntimeValue>,
    ) -> Result<(), String> {
        match target {
            LValue::Name(name) => {
                if runtime.storage().get_global(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .force_global(name.clone(), value.ok_or("missing force value")?),
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_global(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                if runtime.storage().get_retain(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .force_retain(name.clone(), value.ok_or("missing force value")?),
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_retain(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                Err(
                    "force/release is only supported for globals, retains, or instance fields"
                        .to_string(),
                )
            }
            LValue::Field { target, field } => {
                let LValue::Name(name) = target.as_ref() else {
                    return Err(
                        "force/release is only supported for globals, retains, or instance fields"
                            .to_string(),
                    );
                };
                let instance_id = match runtime.storage().get_global(name.as_ref()) {
                    Some(RuntimeValue::Instance(id)) => *id,
                    _ => return Err(
                        "force/release is only supported for globals, retains, or instance fields"
                            .to_string(),
                    ),
                };
                match directive {
                    SymbolicForceDirective::Force => self.session.debug_control().force_instance(
                        instance_id,
                        field.clone(),
                        value.ok_or("missing force value")?,
                    ),
                    SymbolicForceDirective::Release => self
                        .session
                        .debug_control()
                        .release_instance(instance_id, field.as_ref()),
                }
                Ok(())
            }
            _ => Err(
                "force/release is only supported for globals, retains, or instance fields"
                    .to_string(),
            ),
        }
    }

    fn apply_symbolic_force_directive_for_storage(
        &self,
        storage: &VariableStorage,
        target: &LValue,
        directive: SymbolicForceDirective,
        value: Option<RuntimeValue>,
    ) -> Result<(), String> {
        match target {
            LValue::Name(name) => {
                if storage.get_global(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .force_global(name.clone(), value.ok_or("missing force value")?),
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_global(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                if storage.get_retain(name.as_ref()).is_some() {
                    match directive {
                        SymbolicForceDirective::Force => self
                            .session
                            .debug_control()
                            .force_retain(name.clone(), value.ok_or("missing force value")?),
                        SymbolicForceDirective::Release => {
                            self.session.debug_control().release_retain(name.as_ref())
                        }
                    }
                    return Ok(());
                }
                Err(
                    "force/release is only supported for globals, retains, or instance fields"
                        .to_string(),
                )
            }
            LValue::Field { target, field } => {
                let LValue::Name(name) = target.as_ref() else {
                    return Err(
                        "force/release is only supported for globals, retains, or instance fields"
                            .to_string(),
                    );
                };
                let instance_id = match storage.get_global(name.as_ref()) {
                    Some(RuntimeValue::Instance(id)) => *id,
                    _ => return Err(
                        "force/release is only supported for globals, retains, or instance fields"
                            .to_string(),
                    ),
                };
                match directive {
                    SymbolicForceDirective::Force => self.session.debug_control().force_instance(
                        instance_id,
                        field.clone(),
                        value.ok_or("missing force value")?,
                    ),
                    SymbolicForceDirective::Release => self
                        .session
                        .debug_control()
                        .release_instance(instance_id, field.as_ref()),
                }
                Ok(())
            }
            _ => Err(
                "force/release is only supported for globals, retains, or instance fields"
                    .to_string(),
            ),
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

        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "setExpression not supported in attach mode")
                ],
                ..DispatchOutcome::default()
            };
        }

        let directive = match parse_set_directive(&args.value) {
            Ok(directive) => directive,
            Err(message) => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, &message)],
                    ..DispatchOutcome::default()
                };
            }
        };
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
                    let value = match parse_value_expression(&mut runtime, raw, frame_id) {
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
                    if matches!(&directive, SetDirective::Force(_)) {
                        self.session
                            .debug_control()
                            .force_io(address.clone(), coerced.clone());
                        self.set_io_forced(&address, true);
                    } else {
                        self.session
                            .debug_control()
                            .enqueue_io_write(address.clone(), coerced.clone());
                    }
                    if let Err(err) = runtime.io_mut().write(&address, coerced.clone()) {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &err.to_string())],
                            ..DispatchOutcome::default()
                        };
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
                    .refresh_snapshot_from_storage(runtime.storage(), runtime.current_time());
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
        let target = match parse_debug_lvalue(expr_text, runtime.registry_mut(), profile, &using) {
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
                    &runtime,
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
                let value = match parse_value_expression(&mut runtime, raw, frame_id) {
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
                if let Err(err) = runtime.write_lvalue(&target, coerced.clone(), frame_id) {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }
                if matches!(directive, SetDirective::Force(_)) {
                    if let Err(message) = self.apply_symbolic_force_directive(
                        &runtime,
                        &target,
                        SymbolicForceDirective::Force,
                        Some(coerced.clone()),
                    ) {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, &message)],
                            ..DispatchOutcome::default()
                        };
                    }
                }
                coerced
            }
        };

        if paused {
            self.session
                .debug_control()
                .refresh_snapshot_from_storage(runtime.storage(), runtime.current_time());
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
                    if let Ok(cache) = self.last_io_state.lock() {
                        if let Some(state) = cache.clone() {
                            events.push(self.event("stIoState", Some(state)));
                        }
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
                    if matches!(&directive, SetDirective::Force(_)) {
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
        let target = match parse_debug_lvalue(expr_text, &mut registry, profile, &using) {
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
                if let Err(message) = self.apply_symbolic_force_directive_for_storage(
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
                let expr = match parse_debug_expression(&raw, &mut registry, profile, &using) {
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
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }

                self.session.debug_control().enqueue_lvalue_write(
                    frame_id,
                    target_clone.clone(),
                    coerced.clone(),
                );
                coerced
            }
            SetDirective::Force(raw) => {
                let expr = match parse_debug_expression(&raw, &mut registry, profile, &using) {
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
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &map_runtime_error(err))],
                        ..DispatchOutcome::default()
                    };
                }

                if let Err(message) = self.apply_symbolic_force_directive_for_storage(
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
