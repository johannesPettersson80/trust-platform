use super::*;

#[test]
fn dint_mod_guard_matches_shared_arithmetic_for_minimum_and_zero_divisor() {
    use crate::program_model::{apply_binary, BinaryOp};

    for (left, right, expected) in [
        (i32::MIN, -1, Ok(Value::DInt(0))),
        (i32::MIN, 0, Err(RuntimeError::ModuloByZero)),
        (-17, 5, Ok(Value::DInt(-2))),
    ] {
        let shared = apply_binary(
            BinaryOp::Mod,
            Value::DInt(left),
            Value::DInt(right),
            &DateTimeProfile::default(),
        );
        let guarded = super::super::apply_dint_binary_guard_borrowed(
            BinaryOp::Mod,
            &Value::DInt(left),
            &Value::DInt(right),
        )
        .map(|result| result.expect("DINT operands use the optimized guard"));
        assert_eq!(shared, expected);
        assert_eq!(guarded, expected);
    }
}

#[test]
fn fused_dint_remainder_matches_stack_at_each_budget_prefix() {
    let code = vec![
        0x20, 0, 0, 0, 0, // LOAD_REF g0
        0x10, 0, 0, 0, 0,    // LOAD_CONST -1
        0x44, // MOD
        0x21, 0, 0, 0, 0,    // STORE_REF g0
        0x06, // RETURN
    ];
    let module = module(code, Value::DInt(-1));
    let lowered = lower_pou_to_register_ir(&module, 1).unwrap();
    assert!(matches!(
        lowered.blocks[0].instructions.as_slice(),
        [
            RegisterInstr::BinaryRefConstToRef { .. },
            RegisterInstr::Return
        ]
    ));
    assert!(sparse_register_read_counts_by_block(&lowered)[0].is_empty());
    let compiled = compile_tier1_block(&module, &lowered.blocks[0]).unwrap();
    assert!(matches!(
        compiled.instructions.as_slice(),
        [
            Tier1CompiledInstr::BinaryRefConstToRefDIntGuard { .. },
            Tier1CompiledInstr::Return
        ]
    ));
    for budget in 0..=7 {
        let mut stack = Context::new(&module);
        stack.storage.set_global("g0", Value::DInt(i32::MIN));
        stack.charge_clone_work = true;
        stack.budget.reset(budget);
        let stack_result = crate::vm::dispatch::execute_pou_stack_with_locals(
            &mut stack,
            &module,
            1,
            None,
            None,
            false,
            0,
            super::super::super::budget::ExecutionEntry::Nested,
        )
        .map(|_| ());

        for tier1_enabled in [false, true] {
            let mut optimized = Context::new(&module);
            optimized.storage.set_global("g0", Value::DInt(i32::MIN));
            optimized.charge_clone_work = true;
            optimized.tier1.set_enabled(tier1_enabled);
            optimized.tier1.hot_block_threshold = 1;
            optimized.budget.reset(budget);
            let register_result = try_execute_pou_with_register_ir_with_locals(
                &mut optimized,
                &module,
                1,
                None,
                None,
                false,
                0,
                super::super::super::budget::ExecutionEntry::Nested,
            )
            .map(|_| ());
            assert_eq!(
                register_result, stack_result,
                "budget={budget}, tier1={tier1_enabled}"
            );
            assert_eq!(
                optimized.storage.get_global("g0"),
                stack.storage.get_global("g0"),
                "budget={budget}, tier1={tier1_enabled}"
            );
        }
        if budget == 7 {
            assert_eq!(stack_result, Ok(()));
            assert_eq!(stack.storage.get_global("g0"), Some(&Value::DInt(0)));
        }
    }
}

#[test]
fn fused_real_division_preserves_stack_fault_at_each_budget_prefix() {
    let code = vec![
        0x20, 0, 0, 0, 0, // LOAD_REF g0
        0x10, 0, 0, 0, 0,    // LOAD_CONST 0.0
        0x43, // DIV
        0x21, 0, 0, 0, 0,    // STORE_REF g0
        0x06, // RETURN
    ];
    let module = module(code, Value::LReal(0.0));
    for budget in 0..=8 {
        let mut stack = Context::new(&module);
        stack.storage.set_global("g0", Value::LReal(1.0));
        stack.charge_clone_work = true;
        stack.budget.reset(budget);
        let stack_error = crate::vm::dispatch::execute_pou_stack_with_locals(
            &mut stack,
            &module,
            1,
            None,
            None,
            false,
            0,
            super::super::super::budget::ExecutionEntry::Nested,
        )
        .unwrap_err();
        if budget == 5 {
            assert_eq!(stack_error, RuntimeError::DivisionByZero);
        }

        for tier1_enabled in [false, true] {
            let mut optimized = Context::new(&module);
            optimized.storage.set_global("g0", Value::LReal(1.0));
            optimized.charge_clone_work = true;
            optimized.tier1.set_enabled(tier1_enabled);
            optimized.tier1.hot_block_threshold = 1;
            optimized.budget.reset(budget);
            let register_error = try_execute_pou_with_register_ir_with_locals(
                &mut optimized,
                &module,
                1,
                None,
                None,
                false,
                0,
                super::super::super::budget::ExecutionEntry::Nested,
            )
            .unwrap_err();
            assert_eq!(
                register_error, stack_error,
                "budget={budget}, tier1={tier1_enabled}"
            );
            assert_eq!(optimized.storage.get_global("g0"), Some(&Value::LReal(1.0)));
        }
    }
}

#[test]
fn register_frame_locals_and_presence_are_reserved_and_recycled() {
    let mut local_module = module(vec![0x06], Value::DInt(1));
    local_module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 3;
    let mut context = Context::new(&local_module);
    let reserved_bytes = context.scratch.as_ref().unwrap().storage_bytes();
    let mut expected_local_payload = None;
    let mut expected_presence_address = None;
    for _ in 0..2 {
        let result = try_execute_pou_with_register_ir_with_parameter_presence(
            &mut context,
            &local_module,
            1,
            None,
            Some(&[Value::Null]),
            Some(&[true]),
            false,
            0,
            super::super::super::budget::ExecutionEntry::Nested,
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.locals, vec![Value::Null, Value::Null, Value::Null]);
        assert_eq!(context.seen_parameter_presence, vec![true, false, false]);
        assert!(!context.register_buffers_available(1));
        let result_address = result.locals.as_ptr() as usize;
        let result_capacity = result.locals.capacity();
        let presence_address = *context
            .seen_presence_addresses
            .last()
            .expect("initialized frame presence buffer");
        if let Some(expected) = expected_local_payload {
            assert_eq!((result_address, result_capacity), expected);
        } else {
            expected_local_payload = Some((result_address, result_capacity));
        }
        if let Some(expected) = expected_presence_address {
            assert_eq!(presence_address, expected);
        } else {
            expected_presence_address = Some(presence_address);
        }
        context
            .recycle_optimized_result_locals(1, result.locals)
            .unwrap();
        assert!(context.register_buffers_available(1));
        let (reserved_locals, reserved_presence) = context
            .scratch
            .as_ref()
            .unwrap()
            .frames
            .reserved_payload_addresses();
        assert_eq!(reserved_locals as usize, result_address);
        assert_eq!(reserved_presence as usize, presence_address);
        assert_eq!(
            context.scratch.as_ref().unwrap().storage_bytes(),
            reserved_bytes
        );
    }

    let mut faulting = module(
        vec![
            0x20, 0, 0, 0, 0, // LOAD_REF g0
            0x20, 1, 0, 0, 0,    // LOAD_REF g1 (zero on the first entry)
            0x43, // DIV
            0x21, 0, 0, 0, 0,    // STORE_REF g0
            0x06, // RETURN
        ],
        Value::LReal(0.0),
    );
    faulting.refs.push(VmRef::Global {
        offset: 1,
        path: crate::value::RefPath::new(),
    });
    faulting.ref_types.push(None);
    faulting.pou_by_id.get_mut(&1).unwrap().local_ref_count = 3;
    let mut failed = Context::new(&faulting);
    failed.storage.set_global("g0", Value::LReal(1.0));
    failed.storage.set_global("g1", Value::LReal(0.0));
    let reserved_bytes = failed.scratch.as_ref().unwrap().storage_bytes();
    let failure = try_execute_pou_with_register_ir_with_parameter_presence(
        &mut failed,
        &faulting,
        1,
        None,
        None,
        Some(&[true]),
        false,
        0,
        super::super::super::budget::ExecutionEntry::Nested,
    )
    .unwrap_err();
    assert_eq!(failure, RuntimeError::DivisionByZero);
    assert_eq!(
        failed.scratch.as_ref().unwrap().storage_bytes(),
        reserved_bytes
    );
    failed.storage.set_global("g1", Value::LReal(1.0));
    let recovered = try_execute_pou_with_register_ir_with_parameter_presence(
        &mut failed,
        &faulting,
        1,
        None,
        None,
        None,
        false,
        0,
        super::super::super::budget::ExecutionEntry::Nested,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        recovered.locals,
        vec![Value::Null, Value::Null, Value::Null]
    );
    assert_eq!(failed.seen_parameter_presence, vec![false, false, false]);
    failed
        .recycle_optimized_result_locals(1, recovered.locals)
        .unwrap();
    assert!(failed.register_buffers_available(1));
    assert_eq!(
        failed.scratch.as_ref().unwrap().storage_bytes(),
        reserved_bytes
    );
}

#[test]
fn wide_string_reference_materialization_charges_unicode_scan_work() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.refs[0] = VmRef::Global {
        offset: 0,
        path: vec![RefSegment::Index(vec![1])],
    };
    let mut context = Context::new(&module);
    context
        .storage
        .set_global("g0", Value::WString("\u{03b1}".repeat(64)));
    // TIME-08 TEXT: 64 two-byte characters = 128 bytes, charged at 2 per byte = 256.
    context.budget.reset(255);
    let error = load_ref(&context, &module, &FrameStack::default(), 0).unwrap_err();
    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            requested: 256,
            limit: 255,
            ..
        })
    ));
    context.budget.reset(256);
    assert_eq!(
        load_ref(&context, &module, &FrameStack::default(), 0).unwrap(),
        Value::WChar(0x03b1)
    );
}

#[test]
fn tier1_repeated_register_operands_match_counted_interpreter_at_budget_prefixes() {
    let mut module = module(
        vec![
            0x20, 0, 0, 0, 0,    // LOAD_REF g0
            0x11, // DUP
            0x40, // ADD
            0x21, 1, 0, 0, 0,    // STORE_REF g1
            0x06, // RETURN
        ],
        Value::DInt(1),
    );
    module.refs.push(VmRef::Global {
        offset: 1,
        path: crate::value::RefPath::new(),
    });
    module.ref_types.push(None);

    for budget in 0..=16 {
        let mut interpreted = Context::new(&module);
        interpreted.storage.set_global("g0", Value::DInt(7));
        interpreted.storage.set_global("g1", Value::DInt(-1));
        interpreted.charge_clone_work = true;
        interpreted.budget.reset(budget);
        let interpreted_result =
            try_execute_pou_with_register_ir(&mut interpreted, &module, 1, None).map(|_| ());

        let mut tier1 = Context::new(&module);
        tier1.storage.set_global("g0", Value::DInt(7));
        tier1.storage.set_global("g1", Value::DInt(-1));
        tier1.charge_clone_work = true;
        tier1.tier1.set_enabled(true);
        tier1.tier1.hot_block_threshold = 1;
        tier1.budget.reset(budget);
        let tier1_result =
            try_execute_pou_with_register_ir(&mut tier1, &module, 1, None).map(|_| ());

        assert_eq!(tier1_result, interpreted_result, "budget={budget}");
        assert_eq!(tier1.budget.remaining(), interpreted.budget.remaining());
        assert_eq!(
            tier1.storage.get_global("g1"),
            interpreted.storage.get_global("g1")
        );
        if budget == 16 {
            assert_eq!(tier1_result, Ok(()));
            assert!(tier1.tier1.snapshot().block_executions > 0);
            let movement = tier1.profile.snapshot().value_ops;
            assert_eq!(movement.register_read_clones, 1);
            assert_eq!(movement.register_read_moves, 2);
        }
    }
}

#[test]
fn tier1_block_entry_stack_reads_remain_live_across_conditional_successors() {
    let code = vec![
        0x20, 0, 0, 0, 0, // LOAD_REF g0: carried value
        0x20, 1, 0, 0, 0, // LOAD_REF g1: branch condition
        0x04, 6, 0, 0, 0, // JMP_IF_FALSE to the second store
        0x21, 2, 0, 0, 0,    // STORE_REF g2
        0x06, // RETURN
        0x21, 3, 0, 0, 0,    // STORE_REF g3
        0x06, // RETURN
    ];
    let mut module = module(code, Value::DInt(1));
    for offset in 1..=3 {
        module.refs.push(VmRef::Global {
            offset,
            path: crate::value::RefPath::new(),
        });
        module.ref_types.push(None);
    }

    let program = lower_pou_to_register_ir(&module, 1).unwrap();
    let counts = sparse_register_read_counts_by_block(&program);
    let carried_blocks = program
        .blocks
        .iter()
        .filter(|block| block.entry_stack_depth == 1)
        .collect::<Vec<_>>();
    assert_eq!(carried_blocks.len(), 2);
    for block in carried_blocks {
        assert!(matches!(
            block.instructions[0],
            RegisterInstr::StoreRef { .. }
        ));
        assert_eq!(counts[block.id as usize], vec![(0, 2)]);
    }

    for (condition, expected) in [
        (true, (Value::DInt(7), Value::DInt(-1))),
        (false, (Value::DInt(-1), Value::DInt(7))),
    ] {
        let mut interpreted = Context::new(&module);
        interpreted.storage.set_global("g0", Value::DInt(7));
        interpreted.storage.set_global("g1", Value::Bool(condition));
        interpreted.storage.set_global("g2", Value::DInt(-1));
        interpreted.storage.set_global("g3", Value::DInt(-1));
        try_execute_pou_with_register_ir(&mut interpreted, &module, 1, None).unwrap();

        let mut tier1 = Context::new(&module);
        tier1.storage.set_global("g0", Value::DInt(7));
        tier1.storage.set_global("g1", Value::Bool(condition));
        tier1.storage.set_global("g2", Value::DInt(-1));
        tier1.storage.set_global("g3", Value::DInt(-1));
        tier1.tier1.set_enabled(true);
        tier1.tier1.hot_block_threshold = 1;
        try_execute_pou_with_register_ir(&mut tier1, &module, 1, None).unwrap();

        assert_eq!(tier1.storage.get_global("g2").cloned(), Some(expected.0));
        assert_eq!(tier1.storage.get_global("g3").cloned(), Some(expected.1));
        assert_eq!(tier1.tier1.snapshot().block_executions, 2);
    }
}

#[test]
fn tier1_normalized_duplicate_output_survives_store_before_jump() {
    let code = vec![
        0x20, 0, 0, 0, 0,    // LOAD_REF g0
        0x11, // DUP
        0x21, 1, 0, 0, 0, // STORE_REF g1, leaving one copy on the stack
        0x02, 0, 0, 0, 0, // JMP to the following block
        0x21, 2, 0, 0, 0,    // successor: STORE_REF g2 from the carried copy
        0x06, // RETURN
    ];
    let mut module = module(code, Value::DInt(1));
    for offset in 1..=2 {
        module.refs.push(VmRef::Global {
            offset,
            path: crate::value::RefPath::new(),
        });
        module.ref_types.push(None);
    }

    let program = lower_pou_to_register_ir(&module, 1).unwrap();
    assert_eq!(program.blocks.len(), 2);
    assert_eq!(program.blocks[0].entry_stack_depth, 0);
    assert_eq!(program.blocks[1].entry_stack_depth, 1);
    assert!(matches!(
        program.blocks[0].instructions.as_slice(),
        [
            RegisterInstr::LoadRef {
                dest: RegisterId(1),
                ..
            },
            RegisterInstr::StoreRef {
                src: RegisterId(1),
                ..
            },
            RegisterInstr::Move {
                src: RegisterId(1),
                dest: RegisterId(0)
            },
            RegisterInstr::Jump {
                target: BlockTarget::Block(1)
            }
        ]
    ));
    assert_eq!(
        sparse_register_read_counts_by_block(&program),
        vec![vec![(1, 2)], vec![(0, 2)]]
    );
    let dense = register_read_counts_by_block(&program);
    let sparse = sparse_register_read_counts_by_block(&program);
    for (dense, sparse) in dense.iter().zip(sparse) {
        let mut expanded = vec![0; program.max_registers as usize];
        for (register, count) in sparse {
            expanded[register as usize] = count;
        }
        assert_eq!(dense, &expanded);
    }

    for budget in 0..=20 {
        let mut interpreted = Context::new(&module);
        interpreted.storage.set_global("g0", Value::DInt(7));
        interpreted.storage.set_global("g1", Value::DInt(-1));
        interpreted.storage.set_global("g2", Value::DInt(-1));
        interpreted.charge_clone_work = true;
        interpreted.budget.reset(budget);
        let interpreted_result =
            try_execute_pou_with_register_ir(&mut interpreted, &module, 1, None).map(|_| ());

        let mut tier1 = Context::new(&module);
        tier1.storage.set_global("g0", Value::DInt(7));
        tier1.storage.set_global("g1", Value::DInt(-1));
        tier1.storage.set_global("g2", Value::DInt(-1));
        tier1.charge_clone_work = true;
        tier1.tier1.set_enabled(true);
        tier1.tier1.hot_block_threshold = 1;
        tier1.budget.reset(budget);
        let tier1_result =
            try_execute_pou_with_register_ir(&mut tier1, &module, 1, None).map(|_| ());

        assert_eq!(tier1_result, interpreted_result, "budget={budget}");
        assert_eq!(tier1.budget.remaining(), interpreted.budget.remaining());
        assert_eq!(
            tier1.storage.get_global("g1"),
            interpreted.storage.get_global("g1")
        );
        assert_eq!(
            tier1.storage.get_global("g2"),
            interpreted.storage.get_global("g2")
        );
        if budget == 20 {
            assert_eq!(tier1_result, Ok(()));
            assert_eq!(tier1.storage.get_global("g1"), Some(&Value::DInt(7)));
            assert_eq!(tier1.storage.get_global("g2"), Some(&Value::DInt(7)));
            assert_eq!(tier1.tier1.snapshot().block_executions, 2);
            let movement = tier1.profile.snapshot().value_ops;
            assert_eq!(movement.register_read_clones, 2);
            assert_eq!(movement.register_read_moves, 1);
        }
    }
}

#[test]
fn tier1_fused_self_field_store_keeps_its_register_value_read() {
    let module = module(vec![0x06], Value::DInt(1));
    let block = RegisterBlock {
        id: 0,
        start_pc: 0,
        end_pc: 4,
        entry_stack_depth: 0,
        bytecode_instruction_count: 4,
        instruction_costs: vec![3, 1],
        instruction_cost_steps: vec![vec![1, 1, 1], vec![1]],
        instructions: vec![
            RegisterInstr::StoreSelfFieldDynamic {
                field_idx: 0,
                value: RegisterId(2),
            },
            RegisterInstr::Return,
        ],
    };
    let program = RegisterProgram {
        pou_id: 1,
        entry_block: 0,
        max_registers: 3,
        blocks: vec![block.clone()],
    };

    assert_eq!(
        sparse_register_read_counts_by_block(&program),
        vec![vec![(2, 1)]]
    );
    let compiled = compile_tier1_block(&module, &block).unwrap();
    assert!(matches!(
        compiled.instructions.as_slice(),
        [
            Tier1CompiledInstr::StoreSelfFieldDynamic {
                value: RegisterId(2),
                ..
            },
            Tier1CompiledInstr::Return
        ]
    ));
}

#[test]
fn failed_non_last_clone_admission_keeps_register_and_read_count_unchanged() {
    let module = module(vec![0x06], Value::DInt(1));
    let mut context = Context::new(&module);
    context.charge_clone_work = true;
    context.budget.reset(0);
    let mut registers = [Value::String("payload".into())];
    let mut reads = [2_u32];

    let error =
        checked_read_register_with_counts(&mut context, &mut registers, &mut reads, RegisterId(0))
            .unwrap_err();
    assert!(matches!(
        error,
        RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::Work,
            ..
        }
    ));
    assert_eq!(reads, [2]);
    assert_eq!(registers, [Value::String("payload".into())]);

    context.budget.reset(1);
    let first =
        checked_read_register_with_counts(&mut context, &mut registers, &mut reads, RegisterId(0))
            .unwrap();
    assert_eq!(first, Value::String("payload".into()));
    assert_eq!(reads, [1]);
    assert_eq!(registers, [Value::String("payload".into())]);
    assert_eq!(context.budget.remaining(), 0);

    let last =
        checked_read_register_with_counts(&mut context, &mut registers, &mut reads, RegisterId(0))
            .unwrap();
    assert_eq!(last, Value::String("payload".into()));
    assert_eq!(reads, [0]);
    assert_eq!(registers, [Value::Null]);
    assert_eq!(context.budget.remaining(), 0);
    let movement = context.profile.snapshot().value_ops;
    assert_eq!(movement.register_read_clones, 1);
    assert_eq!(movement.register_read_moves, 1);
}

#[test]
fn tier1_store_dynamic_same_register_reads_reference_then_moves_value() {
    let module = module(
        vec![
            0x22, 0, 0, 0, 0,    // LOAD_REF_ADDR g0
            0x11, // DUP: StoreDynamic receives the same register twice
            0x33, // STORE_DYNAMIC
            0x06, // RETURN
        ],
        Value::DInt(1),
    );
    let mut interpreted = Context::new(&module);
    interpreted.storage.set_global("g0", Value::DInt(7));
    interpreted.charge_clone_work = true;
    interpreted.budget.reset(16);
    let interpreted_result =
        try_execute_pou_with_register_ir(&mut interpreted, &module, 1, None).map(|_| ());

    let mut tier1 = Context::new(&module);
    tier1.storage.set_global("g0", Value::DInt(7));
    tier1.charge_clone_work = true;
    tier1.tier1.set_enabled(true);
    tier1.tier1.hot_block_threshold = 1;
    tier1.budget.reset(16);
    let tier1_result = try_execute_pou_with_register_ir(&mut tier1, &module, 1, None).map(|_| ());

    assert_eq!(tier1_result, interpreted_result);
    assert_eq!(tier1.budget.remaining(), interpreted.budget.remaining());
    assert_eq!(
        tier1.storage.get_global("g0"),
        interpreted.storage.get_global("g0")
    );
    assert!(matches!(
        tier1.storage.get_global("g0"),
        Some(Value::Reference(Some(_)))
    ));
    assert!(tier1.tier1.snapshot().block_executions > 0);
    let movement = tier1.profile.snapshot().value_ops;
    assert_eq!(movement.register_read_clones, 1);
    assert_eq!(movement.register_read_moves, 1);
}

#[test]
fn tier1_store_dynamic_validates_reference_before_value_copy_admission() {
    let module = module(vec![0x06], Value::DInt(1));
    let source_block = RegisterBlock {
        id: 0,
        start_pc: 0,
        end_pc: 2,
        entry_stack_depth: 0,
        bytecode_instruction_count: 2,
        instruction_costs: vec![0, 0],
        instruction_cost_steps: vec![vec![0], vec![0]],
        instructions: vec![
            RegisterInstr::StoreDynamic {
                reference: RegisterId(0),
                value: RegisterId(1),
            },
            RegisterInstr::Move {
                src: RegisterId(1),
                dest: RegisterId(2),
            },
        ],
    };
    let program = RegisterProgram {
        pou_id: 1,
        entry_block: 0,
        max_registers: 3,
        blocks: vec![source_block.clone()],
    };
    let compiled = Tier1CompiledBlock {
        instructions: vec![
            Tier1CompiledInstr::StoreDynamic {
                reference: RegisterId(0),
                value: RegisterId(1),
            },
            Tier1CompiledInstr::Move {
                src: RegisterId(1),
                dest: RegisterId(2),
            },
        ],
    };
    let counts = sparse_register_read_counts_by_block(&program);
    assert_eq!(counts, vec![vec![(0, 1), (1, 2)]]);

    for (invalid_reference, expected_error) in [
        (Value::Reference(None), RuntimeError::NullReference),
        (Value::DInt(5), RuntimeError::TypeMismatch),
    ] {
        let mut context = Context::new(&module);
        context.charge_clone_work = true;
        context.budget.reset(0);
        let mut registers = [
            invalid_reference,
            Value::String("value must remain unread".into()),
            Value::Null,
        ];
        let mut remaining_register_reads = vec![0; program.max_registers as usize];
        for (register, count) in &counts[0] {
            remaining_register_reads[*register as usize] = *count;
        }
        let mut frames = FrameStack::default();
        let mut native_call_stack = OperandStack::default();

        let error = super::super::tier1::execute_tier1_compiled_block(
            &mut context,
            &module,
            &program,
            &source_block,
            &mut frames,
            &mut registers,
            &mut remaining_register_reads,
            &mut native_call_stack,
            &compiled,
            0,
        )
        .unwrap_err();

        assert_eq!(error, expected_error);
        assert_eq!(context.budget.remaining(), 0);
        assert_eq!(registers[0], Value::Null);
        assert_eq!(remaining_register_reads[0], 0);
        assert_eq!(
            registers[1],
            Value::String("value must remain unread".into())
        );
        assert_eq!(remaining_register_reads[1], 2);
        let movement = context.profile.snapshot().value_ops;
        assert_eq!(movement.register_read_clones, 0);
        assert_eq!(movement.register_read_moves, 1);
    }
}

#[test]
fn tier1_fault_after_last_use_move_recycles_buffers_for_next_entry() {
    let code = vec![
        0x20, 0, 0, 0, 0, // LOAD_REF g0
        0x21, 1, 0, 0, 0, // STORE_REF g1: last-use move before the later fault
        0x20, 2, 0, 0, 0, // LOAD_REF g2
        0x20, 3, 0, 0, 0,    // LOAD_REF g3
        0x43, // DIV
        0x21, 4, 0, 0, 0,    // STORE_REF g4
        0x06, // RETURN
    ];
    let mut module = module(code, Value::DInt(1));
    for offset in 1..=4 {
        module.refs.push(VmRef::Global {
            offset,
            path: crate::value::RefPath::new(),
        });
        module.ref_types.push(None);
    }
    let mut context = Context::new(&module);
    for (name, value) in [
        ("g0", Value::DInt(9)),
        ("g1", Value::DInt(-1)),
        ("g2", Value::DInt(10)),
        ("g3", Value::DInt(0)),
        ("g4", Value::DInt(-1)),
    ] {
        context.storage.set_global(name, value);
    }
    context.charge_clone_work = true;
    context.tier1.set_enabled(true);
    context.tier1.hot_block_threshold = 1;

    assert_eq!(
        try_execute_pou_with_register_ir(&mut context, &module, 1, None),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(context.storage.get_global("g1"), Some(&Value::DInt(9)));
    assert!(context.tier1.snapshot().cached_blocks > 0);
    assert_eq!(context.profile.snapshot().value_ops.register_read_moves, 1);
    let (frames, registers, reads, native_call_stack) =
        context.scratch.as_mut().unwrap().buffers_mut();
    assert!(frames.is_empty());
    assert!(registers.iter().all(|value| matches!(value, Value::Null)));
    assert!(reads.iter().all(|count| *count == 0));
    assert!(native_call_stack.peek().is_err());

    context.storage.set_global("g3", Value::DInt(2));
    context.budget.reset(99);
    assert_eq!(
        try_execute_pou_with_register_ir(&mut context, &module, 1, None),
        Ok(RegisterExecutionOutcome::Executed)
    );
    assert_eq!(context.storage.get_global("g1"), Some(&Value::DInt(9)));
    assert_eq!(context.storage.get_global("g4"), Some(&Value::DInt(5)));
}
