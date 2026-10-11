//! The saved compiler-produced call chain uses one ordinary dispatch buffer owner.
use super::*;
use crate::vm::VmTrap;

#[test]
fn compiled_call_depth_does_not_grow_native_dispatch_buffer_nesting() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    state.storage.set_global("probe_depth", Value::Int(2));
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    // Depth two already covers ordinary entry plus synchronous return initialization.
    let buffers = state.resources.buffers.len();
    state.storage.set_global("probe_depth", Value::Int(4));
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(
        state.resources.buffers.len(),
        buffers,
        "user call nesting must not acquire another recursive dispatcher buffer"
    );
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("program root")
    };
    assert_eq!(
        state.storage.get_instance_var(*plant, "depth_result"),
        Some(&Value::Int(4))
    );
    assert!(state.lifetimes.live_activations.is_empty());
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert_eq!(state.lifetimes.activation_pous.len(), 0);
}

#[test]
fn over_depth_unwinds_every_frame_and_allows_a_new_operation() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 3,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    state.storage.set_global("probe_depth", Value::Int(4));
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert!(state.lifetimes.live_activations.is_empty());
    assert_eq!(state.storage.execution_frame_count(), 0);
    assert_eq!(state.lifetimes.activation_pous.len(), 0);
    assert!(state.construction.initializers.is_empty());
    state.storage.set_global("probe_depth", Value::Int(2));
    // Invoke the same shared entry after resetting the enclosing operation;
    // this checks cleanup without clearing or reconstructing the installed state.
    state.resources.work_budget.reset(prepared.limits.max_work);
    let Some(Value::Instance(plant)) = state.storage.get_global("Plant") else {
        panic!("program root")
    };
    let plant = *plant;
    let pou = *state.construction.instance_templates.get(&plant).unwrap();
    state.execute_pou(pou, Some(plant)).unwrap();
    assert_eq!(
        state.storage.get_instance_var(plant, "depth_result"),
        Some(&Value::Int(2))
    );
    assert!(state.lifetimes.live_activations.is_empty());
}

#[test]
fn forged_initializer_calls_reject_before_binding_and_preserve_lexical_locals() {
    let bytes =
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
    let prepared =
        PreparedModule::from_bytes(bytes, crate::vm::PreparationLimits::default()).unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    for opcode in [0x05, 0x09] {
        let mut forged =
            PreparedModule::from_bytes(bytes, crate::vm::PreparationLimits::default()).unwrap();
        let start = forged.vm.code.len();
        forged.vm.code.push(opcode);
        if opcode == 0x09 {
            forged
                .vm
                .code
                .extend_from_slice(&crate::bytecode::NATIVE_CALL_KIND_FUNCTION.to_le_bytes());
        }
        forged.vm.code.extend_from_slice(&u32::MAX.to_le_bytes());
        forged.vm.code.extend_from_slice(&0u32.to_le_bytes());
        let mut lexical = VmFrame {
            activation: None,
            pou_id: None,
            return_pc: 0,
            code_start: start,
            code_end: forged.vm.code.len(),
            local_ref_start: 0,
            local_ref_count: 1,
            locals: vec![Value::Int(17)],
            parameter_values_present: Vec::new(),
            runtime_instance: None,
            instance_owner: None,
        };
        assert_eq!(
            crate::vm::dispatch::execute_initializer_body(&mut state, &forged.vm, &mut lexical, 0),
            Err(RuntimeError::StagingViolation)
        );
        assert_eq!(lexical.locals, vec![Value::Int(17)]);
        assert_eq!(state.storage.execution_frame_count(), 0);
        assert!(state.lifetimes.live_activations.is_empty());
    }
}

#[test]
fn initializer_pre_entry_failures_return_lexical_locals_and_allow_reuse() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut state = EngineState::new(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // Empty admitted range is sufficient: neither rejection may consume lexical state.
    let mut frame = VmFrame {
        activation: None,
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: vec![Value::Int(23)],
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };
    assert_eq!(
        crate::vm::dispatch::execute_initializer_body(&mut state, &prepared.vm, &mut frame, 4),
        Err(VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(frame.locals, vec![Value::Int(23)]);
    state.resources.buffers.clear();
    state
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    assert_eq!(
        crate::vm::dispatch::execute_initializer_body(&mut state, &prepared.vm, &mut frame, 0),
        Err(VmTrap::BudgetExceeded.into_runtime_error())
    );
    assert_eq!(frame.locals, vec![Value::Int(23)]);
    assert_eq!(state.storage.execution_frame_count(), 0);
    state.resources.construction_bytes.set(0);
    state.resources.work_budget.reset(prepared.limits.max_work);
    crate::vm::dispatch::execute_initializer_body(&mut state, &prepared.vm, &mut frame, 0).unwrap();
    assert_eq!(frame.locals, vec![Value::Int(23)]);
    assert_eq!(state.storage.execution_frame_count(), 0);
}
