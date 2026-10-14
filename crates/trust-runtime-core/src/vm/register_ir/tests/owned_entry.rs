use super::*;

#[test]
fn owned_initial_locals_move_into_reserved_frame_without_clone_admission() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 3;
    let mut context = Context::new(&module);
    context.charge_clone_work = true;
    context.budget.reset(1_000);
    let (reserved_address, _) = context
        .scratch
        .as_ref()
        .unwrap()
        .frames
        .reserved_payload_addresses();
    let array = Value::Array(Box::new(
        crate::value::ArrayValue::from_untyped_parts(
            vec![Value::DInt(4), Value::DInt(5)],
            vec![(0, 1)],
        )
        .unwrap(),
    ));
    let expected = vec![Value::DInt(3), array, Value::DInt(6)];
    let mut owned = Some(expected.clone());
    let Value::Array(array) = &owned.as_ref().unwrap()[1] else {
        panic!("array input")
    };
    let array_address = array.elements().as_ptr();

    let result = try_execute_pou_with_register_ir_with_parameter_presence_owned(
        &mut context,
        &module,
        1,
        None,
        &mut owned,
        Some(&[true, false, true]),
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap()
    .unwrap();

    assert_eq!(result.locals, expected);
    assert_eq!(owned, Some(vec![Value::Null; 3]));
    assert_eq!(context.clone_checks.get(), 0);
    assert_eq!(result.locals.as_ptr(), reserved_address);
    let Value::Array(array) = &result.locals[1] else {
        panic!("array result")
    };
    assert_eq!(array.elements().as_ptr(), array_address);
    assert_eq!(context.seen_parameter_presence, vec![true, false, true]);
    context
        .recycle_optimized_result_locals(1, result.locals)
        .unwrap();
    let (recycled_address, _) = context
        .scratch
        .as_ref()
        .unwrap()
        .frames
        .reserved_payload_addresses();
    assert_eq!(recycled_address, reserved_address);
}

#[test]
fn borrowed_initial_locals_keep_clone_admission_and_charge() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 2;
    let mut baseline = Context::new(&module);
    baseline.charge_clone_work = true;
    baseline.budget.reset(1_000);
    let baseline_result = try_execute_pou_with_register_ir_with_parameter_presence(
        &mut baseline,
        &module,
        1,
        None,
        None,
        None,
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap()
    .unwrap();
    let baseline_remaining = baseline.budget.remaining();
    baseline
        .recycle_optimized_result_locals(1, baseline_result.locals)
        .unwrap();

    let mut context = Context::new(&module);
    context.charge_clone_work = true;
    context.budget.reset(1_000);
    let initial = [Value::DInt(11), Value::DInt(22)];
    let result = try_execute_pou_with_register_ir_with_parameter_presence(
        &mut context,
        &module,
        1,
        None,
        Some(&initial),
        None,
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap()
    .unwrap();

    assert_eq!(result.locals, initial);
    assert_eq!(context.clone_checks.get(), initial.len());
    assert_eq!(
        context.budget.remaining() + initial.len(),
        baseline_remaining
    );
    context
        .recycle_optimized_result_locals(1, result.locals)
        .unwrap();
}

#[test]
fn owned_entry_declines_without_mutating_payload_for_debug_missing_and_occupied_paths() {
    let module = module(vec![0x06], Value::DInt(1));

    let mut debug = Context::new(&module);
    debug.debug_active = true;
    let mut debug_input = Some(vec![Value::DInt(17)]);
    let debug_before = debug_input.clone();
    assert!(
        try_execute_pou_with_register_ir_with_parameter_presence_owned(
            &mut debug,
            &module,
            1,
            None,
            &mut debug_input,
            None,
            false,
            0,
            ExecutionEntry::Nested,
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(debug_input, debug_before);

    let mut missing = Context::new(&module);
    let mut missing_input = Some(vec![Value::DInt(18)]);
    let missing_before = missing_input.clone();
    assert!(
        try_execute_pou_with_register_ir_with_parameter_presence_owned(
            &mut missing,
            &module,
            99,
            None,
            &mut missing_input,
            None,
            false,
            0,
            ExecutionEntry::Nested,
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(missing_input, missing_before);

    let mut occupied = Context::new(&module);
    let scratch = occupied.scratch.take().unwrap();
    let mut occupied_input = Some(vec![Value::DInt(19)]);
    let occupied_before = occupied_input.clone();
    assert!(
        try_execute_pou_with_register_ir_with_parameter_presence_owned(
            &mut occupied,
            &module,
            1,
            None,
            &mut occupied_input,
            None,
            false,
            0,
            ExecutionEntry::Nested,
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(occupied_input, occupied_before);
    occupied.scratch = Some(scratch);
}

#[test]
fn owned_entry_failures_after_transfer_recycle_the_reserved_frame_for_reuse() {
    let mut module = module(vec![0x06], Value::DInt(1));
    module.pou_by_id.get_mut(&1).unwrap().local_ref_count = 3;
    let mut context = Context::new(&module);
    context.charge_clone_work = true;
    let reserved_bytes = context.scratch.as_ref().unwrap().storage_bytes();
    let (reserved_address, presence_address) = context
        .scratch
        .as_ref()
        .unwrap()
        .frames
        .reserved_payload_addresses();
    let array = Value::Array(Box::new(
        crate::value::ArrayValue::from_untyped_parts(vec![Value::DInt(8)], vec![(0, 0)]).unwrap(),
    ));

    let mut presence_failure = Some(vec![Value::DInt(1), array.clone(), Value::DInt(3)]);
    let error = try_execute_pou_with_register_ir_with_parameter_presence_owned(
        &mut context,
        &module,
        1,
        None,
        &mut presence_failure,
        Some(&[true, false, true, false]),
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap_err();
    assert_eq!(error, RuntimeError::InvalidExecutionState);
    assert_eq!(presence_failure, Some(vec![Value::Null; 3]));
    assert!(context.register_buffers_available(1));
    assert_eq!(
        context.scratch.as_ref().unwrap().storage_bytes(),
        reserved_bytes
    );
    assert_eq!(
        context
            .scratch
            .as_ref()
            .unwrap()
            .frames
            .reserved_payload_addresses(),
        (reserved_address, presence_address)
    );

    context.fail_initialization = true;
    let mut initializer_failure = Some(vec![Value::DInt(4), array, Value::DInt(6)]);
    let error = try_execute_pou_with_register_ir_with_parameter_presence_owned(
        &mut context,
        &module,
        1,
        None,
        &mut initializer_failure,
        None,
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap_err();
    assert_eq!(error, RuntimeError::InvalidExecutionState);
    assert_eq!(initializer_failure, Some(vec![Value::Null; 3]));
    assert!(context.register_buffers_available(1));
    assert_eq!(
        context.scratch.as_ref().unwrap().storage_bytes(),
        reserved_bytes
    );
    context.fail_initialization = false;

    let mut recovered_input = Some(vec![Value::DInt(12), Value::Null, Value::DInt(14)]);
    let recovered = try_execute_pou_with_register_ir_with_parameter_presence_owned(
        &mut context,
        &module,
        1,
        None,
        &mut recovered_input,
        None,
        false,
        0,
        ExecutionEntry::Nested,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        recovered.locals,
        vec![Value::DInt(12), Value::Null, Value::DInt(14)]
    );
    assert_eq!(recovered.locals.as_ptr(), reserved_address);
    assert_eq!(context.clone_checks.get(), 0);
    context
        .recycle_optimized_result_locals(1, recovered.locals)
        .unwrap();
    assert_eq!(
        context
            .scratch
            .as_ref()
            .unwrap()
            .frames
            .reserved_payload_addresses(),
        (reserved_address, presence_address)
    );
}
