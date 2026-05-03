#[test]
fn register_lowering_cache_hits_after_first_execution() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();
    runtime.storage_mut().set_global("g0", Value::DInt(1));

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first execution");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second execution");
    assert_eq!(first, RegisterExecutionOutcome::Executed);
    assert_eq!(second, RegisterExecutionOutcome::Executed);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 0);
}

#[test]
fn register_lowering_cache_caches_lowering_errors() {
    let mut code = Vec::new();
    code.push(0x02);
    emit_i32(&mut code, 4096);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, Vec::new(), 0);

    let mut runtime = Runtime::new();
    runtime.set_vm_register_lowering_cache_enabled(true);
    runtime.reset_vm_register_lowering_cache();

    let first = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("first fallback");
    let second = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("second fallback");
    assert_eq!(first, RegisterExecutionOutcome::FallbackToStack);
    assert_eq!(second, RegisterExecutionOutcome::FallbackToStack);

    let snapshot = runtime.vm_register_lowering_cache_snapshot();
    assert!(snapshot.enabled);
    assert_eq!(snapshot.cached_entries, 1);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.hits, 1);
    assert_eq!(snapshot.build_errors, 1);
}

#[test]
fn register_executor_tier1_specialized_executor_keeps_startup_path_cold_until_hot_threshold() {
    let mut code = Vec::new();
    code.push(0x20);
    emit_u32(&mut code, 0);
    code.push(0x10);
    emit_u32(&mut code, 0);
    code.push(0x40);
    code.push(0x21);
    emit_u32(&mut code, 0);
    code.push(0x06);
    let (module, pou_id) = manual_vm_module(code, vec![Value::DInt(1)], 1);

    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("g0", Value::DInt(41));
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();

    let outcome = try_execute_pou_with_register_ir(&mut runtime, &module, pou_id, None)
        .expect("execute register program");
    assert_eq!(outcome, RegisterExecutionOutcome::Executed);
    let snapshot = runtime.vm_tier1_specialized_executor_snapshot();
    assert_eq!(snapshot.compile_attempts, 0);
    assert_eq!(snapshot.block_executions, 0);
}

#[test]
fn tier1_dint_binary_guard_returns_exact_arithmetic_results() {
    let cases = [
        (BinaryOp::Add, 7, 5, Value::DInt(12)),
        (BinaryOp::Sub, 7, 5, Value::DInt(2)),
        (BinaryOp::Mul, 7, 5, Value::DInt(35)),
        (BinaryOp::Div, 8, 2, Value::DInt(4)),
        (BinaryOp::Mod, 8, 3, Value::DInt(2)),
    ];

    for (op, left, right, expected) in cases {
        assert_eq!(
            super::apply_dint_binary_guard_borrowed(
                op,
                &Value::DInt(left),
                &Value::DInt(right),
            )
            .expect("guard result"),
            Some(expected),
            "unexpected guard result for {op:?}",
        );
    }

    let div_zero = super::apply_dint_binary_guard_borrowed(
        BinaryOp::Div,
        &Value::DInt(8),
        &Value::DInt(0),
    )
    .expect_err("division by zero should fail");
    assert!(matches!(div_zero, RuntimeError::DivisionByZero));
}

#[test]
fn tier1_dint_binary_guard_returns_exact_comparison_results() {
    let cases = [
        (BinaryOp::Eq, 7, 7, Value::Bool(true)),
        (BinaryOp::Ne, 7, 5, Value::Bool(true)),
        (BinaryOp::Lt, 5, 7, Value::Bool(true)),
        (BinaryOp::Lt, 7, 7, Value::Bool(false)),
        (BinaryOp::Le, 7, 7, Value::Bool(true)),
        (BinaryOp::Gt, 7, 5, Value::Bool(true)),
        (BinaryOp::Gt, 7, 7, Value::Bool(false)),
        (BinaryOp::Ge, 7, 7, Value::Bool(true)),
    ];

    for (op, left, right, expected) in cases {
        assert_eq!(
            super::apply_dint_binary_guard_borrowed(
                op,
                &Value::DInt(left),
                &Value::DInt(right),
            )
            .expect("guard result"),
            Some(expected),
            "unexpected guard result for {op:?}",
        );
    }
}

#[test]
fn tier1_dint_binary_guard_declines_unsupported_inputs() {
    assert_eq!(
        super::apply_dint_binary_guard_borrowed(
            BinaryOp::Add,
            &Value::Bool(true),
            &Value::DInt(1),
        )
        .expect("guard result"),
        None
    );
    assert_eq!(
        super::apply_dint_binary_guard_borrowed(
            BinaryOp::And,
            &Value::DInt(1),
            &Value::DInt(1),
        )
        .expect("guard result"),
        None
    );
}

#[test]
fn tier1_compiler_accepts_all_fused_binary_register_forms() {
    let instructions = [
        RegisterInstr::BinaryRefToRef {
            op: BinaryOp::Add,
            left_ref_idx: 0,
            right_ref_idx: 1,
            dest_ref_idx: 2,
        },
        RegisterInstr::BinaryRefConstToRef {
            op: BinaryOp::Sub,
            left_ref_idx: 0,
            const_idx: 0,
            dest_ref_idx: 2,
        },
        RegisterInstr::BinaryConstRefToRef {
            op: BinaryOp::Mul,
            const_idx: 0,
            right_ref_idx: 1,
            dest_ref_idx: 2,
        },
    ];

    for instruction in instructions {
        compile_single_tier1_instruction(instruction).expect("fused binary should compile");
    }
}

#[test]
fn tier1_compiler_accepts_cmp_ref_const_jump_only_for_comparisons() {
    compile_single_tier1_instruction(RegisterInstr::CmpRefConstJumpIf {
        op: BinaryOp::Lt,
        ref_idx: 0,
        const_idx: 0,
        jump_if_true: true,
        target: BlockTarget::Exit,
    })
    .expect("comparison branch should compile");

    let err =
        compile_single_tier1_instruction(RegisterInstr::CmpRefConstJumpIf {
            op: BinaryOp::Add,
            ref_idx: 0,
            const_idx: 0,
            jump_if_true: true,
            target: BlockTarget::Exit,
        })
        .expect_err("non-comparison branch should be rejected");
    assert!(
        err.contains("unsupported_cmp_op:add"),
        "unexpected compile error: {err}",
    );
}

fn compile_single_tier1_instruction(instruction: RegisterInstr) -> Result<(), String> {
    let (module, pou_id) = manual_vm_module(Vec::new(), vec![Value::DInt(1)], 3);
    let block = RegisterBlock {
        id: 0,
        start_pc: 0,
        end_pc: 0,
        entry_stack_depth: 0,
        instructions: vec![instruction],
    };
    let key = super::tier1_block_key(&module, pou_id, &block);
    super::compile_tier1_block(&module, &block, key).map(|_| ())
}

