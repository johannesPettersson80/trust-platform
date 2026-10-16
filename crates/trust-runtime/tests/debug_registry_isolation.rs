//! HOST-09: debug-only types cannot change an installed application's schema.

use trust_runtime::{
    error::RuntimeError,
    harness::{parse_debug_expression, parse_debug_lvalue, CompileSession},
    program_model::{Expr, LValue, SizeOfTarget},
    value::Value,
};

#[test]
fn debug_anonymous_type_is_evaluated_without_installing_it() {
    let runtime =
        CompileSession::from_source("PROGRAM Main VAR count : INT := INT#9; END_VAR END_PROGRAM")
            .build_runtime()
            .unwrap();
    let original_time = runtime.current_time();
    let original_global = runtime.storage().get_global("Main").cloned();
    let mut scratch = runtime.registry().clone();
    let expr =
        parse_debug_expression("SIZEOF(STRING[17])", &mut scratch, runtime.profile(), &[]).unwrap();
    let Expr::SizeOf(SizeOfTarget::Type(type_id)) = &expr else {
        panic!("SIZEOF must retain its parser-owned type identity");
    };
    assert!(scratch.get(*type_id).is_some());
    assert!(runtime.registry().get(*type_id).is_none());
    assert_eq!(
        runtime
            .evaluate_debug_expression(&expr, &scratch, None)
            .unwrap(),
        Value::DInt(17)
    );
    assert!(matches!(
        runtime.evaluate_debug_expression(&expr, runtime.registry(), None),
        Err(RuntimeError::TypeMismatch)
    ));
    assert!(runtime.registry().lookup("STRING[17]").is_none());
    assert_eq!(runtime.current_time(), original_time);
    assert_eq!(
        runtime.storage().get_global("Main").cloned(),
        original_global
    );
}

#[test]
fn debug_assignment_binds_scratch_indices_without_mutating_types() {
    let mut runtime = CompileSession::from_source(
        "PROGRAM Main VAR data : ARRAY[1..17] OF INT; END_VAR END_PROGRAM",
    )
    .build_runtime()
    .unwrap();
    let mut scratch = runtime.registry().clone();
    let target = parse_debug_lvalue(
        "Main.data[SIZEOF(STRING[17])]",
        &mut scratch,
        runtime.profile(),
        &[],
    )
    .unwrap();
    let target = runtime
        .resolve_debug_lvalue(target, &scratch, None)
        .unwrap();
    let LValue::Index { indices, .. } = &target else {
        panic!("indexed target");
    };
    assert!(matches!(
        indices.as_slice(),
        [Expr::Literal(Value::DInt(17))]
    ));
    drop(scratch);
    assert!(runtime.registry().lookup("STRING[17]").is_none());
    assert_eq!(runtime.read_lvalue(&target, None).unwrap(), Value::Int(0));
    runtime.write_lvalue(&target, Value::Int(8), None).unwrap();
    assert_eq!(runtime.read_lvalue(&target, None).unwrap(), Value::Int(8));
    let mut scratch = runtime.registry().clone();
    let invalid = parse_debug_lvalue(
        "Main.data[SIZEOF(STRING[18])]",
        &mut scratch,
        runtime.profile(),
        &[],
    )
    .unwrap();
    let invalid = runtime
        .resolve_debug_lvalue(invalid, &scratch, None)
        .unwrap();
    assert!(runtime.write_lvalue(&invalid, Value::Int(9), None).is_err());
    assert_eq!(runtime.read_lvalue(&target, None).unwrap(), Value::Int(8));
    assert!(runtime.registry().lookup("STRING[18]").is_none());
}

#[test]
fn paused_assignment_drops_scratch_types_and_waits_for_the_cycle_boundary() {
    let mut runtime = CompileSession::from_source(
        "PROGRAM Main VAR data : ARRAY[1..17] OF INT; END_VAR END_PROGRAM",
    )
    .build_runtime()
    .unwrap();
    let control = runtime.enable_debug();
    control.refresh_snapshot_from_storage_with_abi(
        runtime.storage(),
        runtime.current_time(),
        runtime.prepared().storage_abi(),
    );
    let mut scratch = runtime.registry().clone();
    let parsed = parse_debug_lvalue(
        "Main.data[SIZEOF(STRING[17])]",
        &mut scratch,
        runtime.profile(),
        &[],
    )
    .unwrap();
    let target = control
        .with_snapshot(|snapshot| {
            snapshot.resolve_lvalue_expressions(
                parsed,
                &scratch,
                Some(runtime.stdlib()),
                &runtime.profile(),
                None,
            )
        })
        .unwrap()
        .unwrap();
    drop(scratch);
    control
        .with_snapshot(|snapshot| {
            snapshot.write_lvalue(
                &target,
                Value::Int(8),
                runtime.registry(),
                &runtime.profile(),
                None,
            )
        })
        .unwrap()
        .unwrap();
    control.enqueue_lvalue_write(None, target.clone(), Value::Int(8));
    assert!(runtime.registry().lookup("STRING[17]").is_none());
    assert_eq!(runtime.read_lvalue(&target, None).unwrap(), Value::Int(0));
    assert_eq!(
        control
            .with_snapshot(|snapshot| {
                snapshot.read_lvalue(&target, runtime.registry(), &runtime.profile(), None)
            })
            .unwrap()
            .unwrap(),
        Value::Int(8)
    );
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.read_lvalue(&target, None).unwrap(), Value::Int(8));
    assert!(runtime.registry().lookup("STRING[17]").is_none());
    assert!(!runtime.faulted());
}
