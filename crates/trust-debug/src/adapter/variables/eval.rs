//! Evaluate request and snapshot helpers.
//! - handle_evaluate: DAP evaluate request
//! - evaluate_with_snapshot: evaluate expressions against a snapshot
//! - parse_value_expression: parse raw expression values

use serde_json::Value;

use trust_hir::types::TypeRegistry;
use trust_runtime::debug::DebugSnapshot;
use trust_runtime::error::RuntimeError;
use trust_runtime::eval::expr::Expr;
use trust_runtime::harness::parse_debug_expression;
use trust_runtime::memory::{FrameId, InstanceId};
use trust_runtime::value::Value as RuntimeValue;

use crate::protocol::{EvaluateArguments, EvaluateResponseBody, Request};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_evaluate(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<EvaluateArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid evaluate args")],
                ..DispatchOutcome::default()
            };
        };

        if let Some(remote) = self.remote_session.as_mut() {
            match remote.evaluate(&args.expression, args.frame_id) {
                Ok(body) => {
                    return DispatchOutcome {
                        responses: vec![self.ok_response(&request, Some(body))],
                        ..DispatchOutcome::default()
                    };
                }
                Err(err) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &err.to_string())],
                        ..DispatchOutcome::default()
                    };
                }
            }
        }

        let expr_source = args.expression.clone();
        let mut frame_id = args.frame_id.map(FrameId);
        let snapshot = self.session.debug_control().snapshot();
        let (expr, value) = if let Some(snapshot) = snapshot.as_ref() {
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
            let using = frame_id
                .and_then(|frame_id| {
                    self.session
                        .metadata()
                        .using_for_frame(&snapshot.storage, frame_id)
                })
                .unwrap_or_default();
            let mut registry = self.session.metadata().registry().clone();
            let expr = match parse_debug_expression(
                &args.expression,
                &mut registry,
                self.session.metadata().profile(),
                &using,
            ) {
                Ok(expr) => expr,
                Err(err) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &err.to_string())],
                        ..DispatchOutcome::default()
                    };
                }
            };
            let value =
                match self.evaluate_with_snapshot(&expr, &registry, frame_id, snapshot, &using) {
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
            (expr, value)
        } else if let Ok(mut runtime) = self.session.runtime_handle().lock() {
            if frame_id == Some(FrameId(0)) && runtime.storage().frames().is_empty() {
                frame_id = None;
            }
            let using = frame_id
                .and_then(|frame_id| runtime.using_for_frame(frame_id))
                .unwrap_or_default();
            let profile = runtime.profile();
            let expr = match parse_debug_expression(
                &args.expression,
                runtime.registry_mut(),
                profile,
                &using,
            ) {
                Ok(expr) => expr,
                Err(err) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &err.to_string())],
                        ..DispatchOutcome::default()
                    };
                }
            };
            let value = match runtime.evaluate_expression(&expr, frame_id) {
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
            (expr, value)
        } else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "runtime busy")],
                ..DispatchOutcome::default()
            };
        };

        let variable = self.variable_from_value("result".to_string(), value, None);
        let body = EvaluateResponseBody {
            result: variable.value,
            r#type: variable.r#type,
            variables_reference: variable.variables_reference,
            named_variables: None,
            indexed_variables: None,
        };

        if matches!(args.context.as_deref(), Some("watch"))
            && !self.watch_cache.contains_key(&expr_source)
        {
            self.watch_cache.insert(expr_source, expr.clone());
            self.session.debug_control().register_watch_expression(expr);
        }

        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
    pub(super) fn evaluate_with_snapshot(
        &self,
        expr: &Expr,
        registry: &TypeRegistry,
        frame_id: Option<FrameId>,
        snapshot: &DebugSnapshot,
        using: &[smol_str::SmolStr],
    ) -> Result<RuntimeValue, RuntimeError> {
        let metadata = self.session.metadata();
        let profile = metadata.profile();
        let now = snapshot.now;
        let functions = metadata.functions();
        let stdlib = metadata.stdlib();
        let function_blocks = metadata.function_blocks();
        let classes = metadata.classes();
        let access = metadata.access_map();

        let mut storage = snapshot.storage.clone();
        let eval = |storage: &mut trust_runtime::memory::VariableStorage,
                    instance_id: Option<InstanceId>|
         -> Result<RuntimeValue, RuntimeError> {
            let mut ctx = trust_runtime::eval::EvalContext {
                storage,
                registry,
                profile,
                now,
                debug: None,
                call_depth: 0,
                functions: Some(functions),
                stdlib: Some(stdlib),
                function_blocks: Some(function_blocks),
                classes: Some(classes),
                using: if using.is_empty() { None } else { Some(using) },
                access: Some(access),
                current_instance: instance_id,
                return_name: None,
                loop_depth: 0,
                pause_requested: false,
                execution_deadline: None,
            };
            trust_runtime::eval::eval_expr(&mut ctx, expr)
        };

        let value = if let Some(frame_id) = frame_id {
            storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(RuntimeError::InvalidFrame(frame_id.0))??
        } else {
            eval(&mut storage, None)?
        };
        Ok(value)
    }

    pub(super) fn parse_value_expression_snapshot(
        &self,
        raw: &str,
        frame_id: Option<FrameId>,
        snapshot: &DebugSnapshot,
    ) -> Result<RuntimeValue, String> {
        let mut frame_id = frame_id;
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
                return Err("unknown frame id".to_string());
            }
        }
        let using = frame_id
            .and_then(|frame_id| {
                self.session
                    .metadata()
                    .using_for_frame(&snapshot.storage, frame_id)
            })
            .unwrap_or_default();
        let mut registry = self.session.metadata().registry().clone();
        let expr = parse_debug_expression(
            raw,
            &mut registry,
            self.session.metadata().profile(),
            &using,
        )
        .map_err(|err| err.to_string())?;
        self.evaluate_with_snapshot(&expr, &registry, frame_id, snapshot, &using)
            .map_err(|err| match err {
                RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
                _ => err.to_string(),
            })
    }

    pub(super) fn with_snapshot_eval<T>(
        &self,
        snapshot: &mut DebugSnapshot,
        frame_id: Option<FrameId>,
        using: &[smol_str::SmolStr],
        registry: &TypeRegistry,
        f: impl FnOnce(&mut trust_runtime::eval::EvalContext<'_>) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let metadata = self.session.metadata();
        let profile = metadata.profile();
        let now = snapshot.now;
        let functions = metadata.functions();
        let stdlib = metadata.stdlib();
        let function_blocks = metadata.function_blocks();
        let classes = metadata.classes();
        let access = metadata.access_map();
        let using = if using.is_empty() { None } else { Some(using) };

        let eval = |storage: &mut trust_runtime::memory::VariableStorage,
                    instance_id: Option<InstanceId>|
         -> Result<T, RuntimeError> {
            let mut ctx = trust_runtime::eval::EvalContext {
                storage,
                registry,
                profile,
                now,
                debug: None,
                call_depth: 0,
                functions: Some(functions),
                stdlib: Some(stdlib),
                function_blocks: Some(function_blocks),
                classes: Some(classes),
                using,
                access: Some(access),
                current_instance: instance_id,
                return_name: None,
                loop_depth: 0,
                pause_requested: false,
                execution_deadline: None,
            };
            f(&mut ctx)
        };

        if let Some(frame_id) = frame_id {
            snapshot
                .storage
                .with_frame(frame_id, |storage| {
                    let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                    eval(storage, instance_id)
                })
                .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut snapshot.storage, None)
        }
    }
}

pub(super) fn parse_value_expression(
    runtime: &mut trust_runtime::Runtime,
    raw: &str,
    frame_id: Option<FrameId>,
) -> Result<RuntimeValue, String> {
    let mut frame_id = frame_id;
    if frame_id == Some(FrameId(0)) && runtime.storage().frames().is_empty() {
        frame_id = None;
    }
    let using = frame_id
        .and_then(|frame_id| runtime.using_for_frame(frame_id))
        .unwrap_or_default();
    let profile = runtime.profile();
    let expr = parse_debug_expression(raw, runtime.registry_mut(), profile, &using)
        .map_err(|err| err.to_string())?;
    runtime
        .evaluate_expression(&expr, frame_id)
        .map_err(|err| match err {
            RuntimeError::InvalidFrame(_) => "unknown frame id".to_string(),
            _ => err.to_string(),
        })
}
