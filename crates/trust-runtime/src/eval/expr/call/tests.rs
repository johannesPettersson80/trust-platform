use super::{bind_split_args, bind_stdlib_named_args, ArgValue, CallArg, EvalContext, Expr};
use crate::error::RuntimeError;
use crate::memory::VariableStorage;
use crate::stdlib::StdParams;
use crate::value::{DateTimeProfile, Duration, Value};
use trust_hir::types::TypeRegistry;

fn make_context<'a>(storage: &'a mut VariableStorage, registry: &'a TypeRegistry) -> EvalContext<'a> {
    EvalContext {
        storage,
        registry,
        initializer_catalog: None,
        profile: DateTimeProfile::default(),
        now: Duration::ZERO,
        debug: None,
        call_depth: 0,
        functions: None,
        stdlib: None,
        function_blocks: None,
        classes: None,
        using: None,
        access: None,
        current_instance: None,
        return_name: None,
        loop_depth: 0,
        execution_deadline: None,
    }
}

fn unnamed_literal_arg(value: Value) -> CallArg {
    CallArg {
        name: None,
        value: ArgValue::Expr(Expr::Literal(value)),
    }
}

#[test]
fn bind_stdlib_named_args_rejects_unnamed_arg_without_panic() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = make_context(&mut storage, &registry);
    let params = StdParams::Fixed(vec!["IN".into()]);
    let args = vec![unnamed_literal_arg(Value::Int(1))];

    let result = bind_stdlib_named_args(&mut ctx, &params, &args);
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidArgumentName(name)) if name.as_str() == "<unnamed>"
    ));
}

#[test]
fn bind_split_args_rejects_unnamed_named_call_without_panic() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = make_context(&mut storage, &registry);
    let args = vec![
        CallArg {
            name: Some("IN".into()),
            value: ArgValue::Expr(Expr::Literal(Value::Int(1))),
        },
        unnamed_literal_arg(Value::Int(2)),
    ];

    let result = bind_split_args(&mut ctx, &["IN", "YEAR"], &args);
    assert!(matches!(
        result,
        Err(RuntimeError::InvalidArgumentName(name)) if name.as_str() == "<unnamed>"
    ));
}
