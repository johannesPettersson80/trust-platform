//! Evaluator entry point.

#![allow(missing_docs)]

use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;

use crate::error::RuntimeError;
use crate::instance::{create_class_instance, create_fb_instance};
use crate::io::IoAddress;
use crate::memory::{InstanceId, VariableStorage};
use crate::stdlib::{fbs, StandardLibrary};
use crate::value::{default_value_for_type_id, DateTimeProfile, Duration, Value};

pub mod expr;
pub mod ops;
pub mod stmt;

/// Evaluation context shared across expression and statement execution.
pub struct EvalContext<'a> {
    pub storage: &'a mut VariableStorage,
    pub registry: &'a TypeRegistry,
    pub profile: DateTimeProfile,
    pub now: Duration,
    pub debug: Option<&'a mut dyn crate::debug::DebugHook>,
    pub call_depth: u32,
    pub functions: Option<&'a IndexMap<SmolStr, FunctionDef>>,
    pub stdlib: Option<&'a StandardLibrary>,
    pub function_blocks: Option<&'a IndexMap<SmolStr, FunctionBlockDef>>,
    pub classes: Option<&'a IndexMap<SmolStr, ClassDef>>,
    pub using: Option<&'a [SmolStr]>,
    pub access: Option<&'a crate::memory::AccessMap>,
    pub current_instance: Option<InstanceId>,
    pub return_name: Option<SmolStr>,
    pub loop_depth: u32,
    pub pause_requested: bool,
    pub execution_deadline: Option<std::time::Instant>,
}

/// Parameter declaration for POUs.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub direction: ParamDirection,
    pub address: Option<IoAddress>,
    pub default: Option<expr::Expr>,
}

/// Variable declaration with optional initializer.
#[derive(Debug, Clone)]
pub struct VarDef {
    pub name: SmolStr,
    pub type_id: TypeId,
    pub initializer: Option<expr::Expr>,
    pub retain: crate::RetainPolicy,
    pub external: bool,
    pub constant: bool,
    pub address: Option<IoAddress>,
}

/// Function definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: SmolStr,
    pub return_type: TypeId,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Base type for a function block.
#[derive(Debug, Clone)]
pub enum FunctionBlockBase {
    FunctionBlock(SmolStr),
    Class(SmolStr),
}

/// Function block definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct FunctionBlockDef {
    pub name: SmolStr,
    pub base: Option<FunctionBlockBase>,
    pub params: Vec<Param>,
    pub vars: Vec<VarDef>,
    pub temps: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
    pub body: Vec<stmt::Stmt>,
}

/// Method definition for classes and function blocks.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: SmolStr,
    pub return_type: Option<TypeId>,
    pub params: Vec<Param>,
    pub locals: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub body: Vec<stmt::Stmt>,
}

/// Class definition (used by tests and runtime).
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub vars: Vec<VarDef>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

/// Interface definition (used for metadata and bytecode emission).
#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub name: SmolStr,
    pub base: Option<SmolStr>,
    pub using: Vec<SmolStr>,
    pub methods: Vec<MethodDef>,
}

/// Call argument value.
#[derive(Debug, Clone)]
pub enum ArgValue {
    Expr(expr::Expr),
    Target(expr::LValue),
}

/// Named call argument.
#[derive(Debug, Clone)]
pub struct CallArg {
    pub name: Option<SmolStr>,
    pub value: ArgValue,
}

#[derive(Debug, Clone)]
enum OutputBinding {
    Param {
        param: SmolStr,
        target: expr::LValue,
    },
    Value {
        target: expr::LValue,
        value: Value,
    },
}

struct PreparedBindings {
    should_execute: bool,
    param_values: Vec<(SmolStr, Value)>,
    out_targets: Vec<OutputBinding>,
}

#[derive(Debug, Clone, Copy)]
enum BindingMode {
    Function,
    FunctionBlock,
}

/// Evaluate an expression.
pub fn eval_expr(ctx: &mut EvalContext<'_>, expr: &expr::Expr) -> Result<Value, RuntimeError> {
    expr::eval_expr(ctx, expr)
}

/// Execute a statement.
pub fn exec_stmt(
    ctx: &mut EvalContext<'_>,
    stmt: &stmt::Stmt,
) -> Result<stmt::StmtResult, RuntimeError> {
    stmt::exec_stmt(ctx, stmt)
}

/// Execute a list of statements.
pub fn exec_block(
    ctx: &mut EvalContext<'_>,
    stmts: &[stmt::Stmt],
) -> Result<stmt::StmtResult, RuntimeError> {
    stmt::exec_block(ctx, stmts)
}

/// Call a function definition.
pub fn call_function<'a>(
    ctx: &mut EvalContext<'a>,
    func: &'a FunctionDef,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let saved_using = ctx.using;
    let saved_return = ctx.return_name.clone();
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(ctx, &func.params, args, BindingMode::Function) {
        Ok(value) => value,
        Err(err) => {
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    ctx.using = Some(&func.using);
    ctx.storage.push_frame(func.name.clone());
    ctx.return_name = Some(func.name.clone());
    let return_default = default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
        .unwrap_or(Value::Null);
    ctx.storage.set_local(func.name.clone(), return_default);
    for (name, value) in param_values {
        ctx.storage.set_local(name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        write_output_values(ctx, output_values)?;
        return Ok(
            default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
                .unwrap_or(Value::Null),
        );
    }

    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);

    if let Err(err) = init_locals(ctx, &func.locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        return Err(err);
    }
    let result = match exec_block(ctx, &func.body) {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    let return_value = match result {
        stmt::StmtResult::Return(Some(value)) => value,
        _ => ctx
            .storage
            .current_frame()
            .and_then(|frame| frame.return_value.clone())
            .unwrap_or_else(|| {
                default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
                    .unwrap_or(Value::Null)
            }),
    };

    let output_values = match collect_outputs(ctx, &out_targets) {
        Ok(values) => values,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };
    ctx.storage.pop_frame();
    ctx.return_name = saved_return;
    ctx.using = saved_using;
    if let Err(err) = write_output_values(ctx, output_values) {
        ctx.call_depth = saved_call_depth;
        return Err(err);
    }
    ctx.call_depth = saved_call_depth;

    Ok(return_value)
}

/// Call a method definition on a specific instance.
pub fn call_method(
    ctx: &mut EvalContext<'_>,
    method: &MethodDef,
    instance_id: InstanceId,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let saved_using = ctx.using;
    let saved_instance = ctx.current_instance;
    let saved_return = ctx.return_name.clone();
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(ctx, &method.params, args, BindingMode::Function) {
        Ok(value) => value,
        Err(err) => {
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };
    ctx.current_instance = Some(instance_id);
    ctx.storage
        .push_frame_with_instance(method.name.clone(), instance_id);
    ctx.return_name = method.return_type.map(|_| method.name.clone());
    if let Some(return_type) = method.return_type {
        let return_default = default_value_for_type_id(return_type, ctx.registry, &ctx.profile)
            .unwrap_or(Value::Null);
        ctx.storage.set_local(method.name.clone(), return_default);
    }
    for (name, value) in param_values {
        ctx.storage.set_local(name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        ctx.current_instance = saved_instance;
        write_output_values(ctx, output_values)?;
        return Ok(method
            .return_type
            .and_then(|ty| default_value_for_type_id(ty, ctx.registry, &ctx.profile).ok())
            .unwrap_or(Value::Null));
    }

    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);

    if let Err(err) = init_locals(ctx, &method.locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        ctx.current_instance = saved_instance;
        return Err(err);
    }
    let result = match exec_block(ctx, &method.body) {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };

    let return_value = if let Some(return_type) = method.return_type {
        match result {
            stmt::StmtResult::Return(Some(value)) => value,
            _ => ctx
                .storage
                .current_frame()
                .and_then(|frame| frame.return_value.clone())
                .unwrap_or_else(|| {
                    default_value_for_type_id(return_type, ctx.registry, &ctx.profile)
                        .unwrap_or(Value::Null)
                }),
        }
    } else {
        Value::Null
    };
    let output_values = match collect_outputs(ctx, &out_targets) {
        Ok(values) => values,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };
    ctx.storage.pop_frame();
    ctx.return_name = saved_return;
    ctx.using = saved_using;
    ctx.current_instance = saved_instance;
    if let Err(err) = write_output_values(ctx, output_values) {
        ctx.call_depth = saved_call_depth;
        return Err(err);
    }
    ctx.call_depth = saved_call_depth;

    Ok(return_value)
}

/// Call a function block definition on a specific instance.
pub fn call_function_block<'a>(
    ctx: &mut EvalContext<'a>,
    fb: &'a FunctionBlockDef,
    instance_id: InstanceId,
    args: &[CallArg],
) -> Result<(), RuntimeError> {
    let saved_using = ctx.using;
    let saved_instance = ctx.current_instance;
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(ctx, &fb.params, args, BindingMode::FunctionBlock) {
        Ok(value) => value,
        Err(err) => {
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
    };
    ctx.using = Some(&fb.using);
    ctx.current_instance = Some(instance_id);
    ctx.storage
        .push_frame_with_instance(fb.name.clone(), instance_id);
    for (name, value) in param_values {
        ctx.storage.set_instance_var(instance_id, name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.current_instance = saved_instance;
        ctx.using = saved_using;
        write_output_values(ctx, output_values)?;
        return Ok(());
    }
    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);
    let builtin_kind = fbs::builtin_kind(fb.name.as_ref());
    let result = if let Some(kind) = builtin_kind {
        fbs::execute_builtin(ctx, instance_id, kind).map(|_| stmt::StmtResult::Continue)
    } else {
        if let Err(err) = init_locals_in_frame(ctx, &fb.temps) {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
        exec_block(ctx, &fb.body)
    };
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    match result {
        stmt::StmtResult::Return(_) | stmt::StmtResult::Continue => {
            let output_values = match collect_outputs(ctx, &out_targets) {
                Ok(values) => values,
                Err(err) => {
                    ctx.call_depth = saved_call_depth;
                    ctx.storage.pop_frame();
                    ctx.current_instance = saved_instance;
                    ctx.using = saved_using;
                    return Err(err);
                }
            };
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            if let Err(err) = write_output_values(ctx, output_values) {
                ctx.call_depth = saved_call_depth;
                return Err(err);
            }
            ctx.call_depth = saved_call_depth;
            Ok(())
        }
        stmt::StmtResult::Exit | stmt::StmtResult::LoopContinue | stmt::StmtResult::Jump(_) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            Err(RuntimeError::InvalidControlFlow)
        }
    }
}

fn prepare_bindings(
    ctx: &mut EvalContext<'_>,
    params: &[Param],
    args: &[CallArg],
    mode: BindingMode,
) -> Result<PreparedBindings, RuntimeError> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_iter = if positional { Some(args.iter()) } else { None };
    if positional {
        let expected = params.iter().filter(|param| !is_en_eno(param)).count();
        if args.len() != expected {
            return Err(RuntimeError::InvalidArgumentCount {
                expected,
                got: args.len(),
            });
        }
    }

    let mut param_values = Vec::new();
    let mut out_targets = Vec::new();

    for param in params {
        if param.name.eq_ignore_ascii_case("EN") && matches!(param.direction, ParamDirection::In) {
            let en_value = if positional {
                Value::Bool(true)
            } else {
                find_arg_value(args, &param.name)
                    .map(|arg| eval_arg_expr(ctx, arg))
                    .transpose()?
                    .unwrap_or(Value::Bool(true))
            };
            param_values.push((param.name.clone(), en_value.clone()));
            if let Value::Bool(false) = en_value {
                let eno_param = params.iter().find(|p| {
                    p.name.eq_ignore_ascii_case("ENO") && matches!(p.direction, ParamDirection::Out)
                });
                if let Some(eno_param) = eno_param {
                    if let Some(arg) = find_arg_target(args, &eno_param.name) {
                        out_targets.push(OutputBinding::Value {
                            target: arg.clone(),
                            value: Value::Bool(false),
                        });
                    }
                }
                return Ok(PreparedBindings {
                    should_execute: false,
                    param_values,
                    out_targets,
                });
            }
            continue;
        }

        if positional
            && param.name.eq_ignore_ascii_case("ENO")
            && matches!(param.direction, ParamDirection::Out)
        {
            let value = default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                .unwrap_or(Value::Null);
            param_values.push((param.name.clone(), value));
            continue;
        }

        let arg = if positional {
            positional_iter.as_mut().and_then(|iter| iter.next())
        } else {
            find_arg_value(args, &param.name)
        };
        match param.direction {
            ParamDirection::In => {
                let value = if let Some(arg) = arg {
                    eval_arg_expr(ctx, arg)?
                } else if let Some(default) = &param.default {
                    expr::eval_expr(ctx, default)?
                } else {
                    default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                        .unwrap_or(Value::Null)
                };
                param_values.push((param.name.clone(), value));
            }
            ParamDirection::Out => {
                if matches!(mode, BindingMode::Function) {
                    let value =
                        default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                            .unwrap_or(Value::Null);
                    param_values.push((param.name.clone(), value));
                }
                if let Some(arg) = arg {
                    let ArgValue::Target(target) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    out_targets.push(OutputBinding::Param {
                        param: param.name.clone(),
                        target: target.clone(),
                    });
                }
            }
            ParamDirection::InOut => {
                if let Some(arg) = arg {
                    let ArgValue::Target(target) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    let value = expr::read_lvalue(ctx, target)?;
                    param_values.push((param.name.clone(), value.clone()));
                    out_targets.push(OutputBinding::Param {
                        param: param.name.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
    }
    Ok(PreparedBindings {
        should_execute: true,
        param_values,
        out_targets,
    })
}

pub(crate) fn init_locals(
    ctx: &mut EvalContext<'_>,
    locals: &[VarDef],
) -> Result<(), RuntimeError> {
    for local in locals {
        if local.external {
            continue;
        }
        if let Some(fb_name) = function_block_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or(RuntimeError::UndefinedFunctionBlock(fb_name))?;
            let instance_id = create_fb_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                fb,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        if let Some(class_name) = class_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            let instance_id = create_class_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                class_def,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        let value = if let Some(expr) = &local.initializer {
            eval_expr(ctx, expr)?
        } else {
            default_value_for_type_id(local.type_id, ctx.registry, &ctx.profile)
                .unwrap_or(Value::Null)
        };
        ctx.storage.set_local(local.name.clone(), value);
    }
    Ok(())
}

pub(crate) fn init_locals_in_frame(
    ctx: &mut EvalContext<'_>,
    locals: &[VarDef],
) -> Result<(), RuntimeError> {
    for local in locals {
        if local.external {
            continue;
        }
        if let Some(fb_name) = function_block_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(fb_name.to_ascii_uppercase());
            let fb = function_blocks
                .get(&key)
                .ok_or(RuntimeError::UndefinedFunctionBlock(fb_name))?;
            let instance_id = create_fb_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                fb,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        if let Some(class_name) = class_type_name(local.type_id, ctx.registry) {
            let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
            let functions = ctx.functions.ok_or(RuntimeError::TypeMismatch)?;
            let stdlib = ctx.stdlib.ok_or(RuntimeError::TypeMismatch)?;
            let classes = ctx.classes.ok_or(RuntimeError::TypeMismatch)?;
            let key = SmolStr::new(class_name.to_ascii_uppercase());
            let class_def = classes.get(&key).ok_or(RuntimeError::TypeMismatch)?;
            let instance_id = create_class_instance(
                ctx.storage,
                ctx.registry,
                &ctx.profile,
                classes,
                function_blocks,
                functions,
                stdlib,
                class_def,
            )?;
            ctx.storage
                .set_local(local.name.clone(), Value::Instance(instance_id));
            continue;
        }
        let value = if let Some(expr) = &local.initializer {
            eval_expr(ctx, expr)?
        } else {
            default_value_for_type_id(local.type_id, ctx.registry, &ctx.profile)
                .unwrap_or(Value::Null)
        };
        ctx.storage.set_local(local.name.clone(), value);
    }
    Ok(())
}

fn function_block_type_name(type_id: TypeId, registry: &TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        trust_hir::Type::FunctionBlock { name } => Some(name.clone()),
        trust_hir::Type::Alias { target, .. } => function_block_type_name(*target, registry),
        _ => None,
    }
}

fn class_type_name(type_id: TypeId, registry: &TypeRegistry) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        trust_hir::Type::Class { name } => Some(name.clone()),
        trust_hir::Type::Alias { target, .. } => class_type_name(*target, registry),
        _ => None,
    }
}

fn collect_outputs(
    ctx: &mut EvalContext<'_>,
    out_targets: &[OutputBinding],
) -> Result<Vec<(expr::LValue, Value)>, RuntimeError> {
    let mut values = Vec::new();
    for binding in out_targets {
        match binding {
            OutputBinding::Param { param, target } => {
                let value = expr::read_lvalue(ctx, &expr::LValue::Name(param.clone()))?;
                values.push((target.clone(), value));
            }
            OutputBinding::Value { target, value } => {
                values.push((target.clone(), value.clone()));
            }
        }
    }
    Ok(values)
}

fn write_output_values(
    ctx: &mut EvalContext<'_>,
    values: Vec<(expr::LValue, Value)>,
) -> Result<(), RuntimeError> {
    for (target, value) in values {
        expr::write_lvalue(ctx, &target, value)?;
    }
    Ok(())
}

fn eval_arg_expr(ctx: &mut EvalContext<'_>, arg: &CallArg) -> Result<Value, RuntimeError> {
    expr::read_arg_value(ctx, arg)
}

fn find_arg_value<'a>(args: &'a [CallArg], name: &SmolStr) -> Option<&'a CallArg> {
    args.iter().find(|arg| arg.name.as_ref() == Some(name))
}

fn find_arg_target<'a>(args: &'a [CallArg], name: &SmolStr) -> Option<&'a expr::LValue> {
    args.iter().find_map(|arg| match &arg.value {
        ArgValue::Target(target) if arg.name.as_ref() == Some(name) => Some(target),
        _ => None,
    })
}

fn is_en_eno(param: &Param) -> bool {
    matches!(param.direction, ParamDirection::In | ParamDirection::Out)
        && (param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO"))
}
