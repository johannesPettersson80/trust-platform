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
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    state.data.storage.set_global("probe_depth", Value::Int(2));
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    // Depth two already covers ordinary entry plus synchronous return initialization.
    let buffers = state.data.resources.buffers.len();
    state.data.storage.set_global("probe_depth", Value::Int(4));
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(
        state.data.resources.buffers.len(),
        buffers,
        "user call nesting must not acquire another recursive dispatcher buffer"
    );
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("program root")
    };
    assert_eq!(
        state.data.storage.get_instance_var(*plant, "depth_result"),
        Some(&Value::Int(4))
    );
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert_eq!(state.data.storage.execution_frame_owner_count(), 0);
}

#[cfg(not(feature = "register-ir"))]
#[test]
fn stack_only_native_chain_preserves_depth_and_cleans_up_after_refusal() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("program root")
    };
    let plant = *plant;
    let pou = *state
        .data
        .construction
        .instance_templates
        .get(&plant)
        .expect("Plant has an admitted program template");

    state.data.storage.set_global("probe_depth", Value::Int(4));
    state.execute_pou(pou, Some(plant)).unwrap();
    assert_eq!(
        state.data.storage.get_instance_var(plant, "depth_result"),
        Some(&Value::Int(4)),
        "native user calls must return through the stack-only cursor driver"
    );
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert_eq!(state.data.storage.execution_activation_count(), 0);

    let limited = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 3,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut limited_data =
        EngineState::construct_data(&limited, 0, &services::LOGICAL_ONLY).unwrap();
    let mut limited_state =
        EngineState::borrow(&mut limited_data, &limited, &services::LOGICAL_ONLY);
    let Some(Value::Instance(limited_plant)) = limited_state.data.storage.get_global("Plant")
    else {
        panic!("program root")
    };
    let limited_plant = *limited_plant;
    let limited_pou = *limited_state
        .data
        .construction
        .instance_templates
        .get(&limited_plant)
        .expect("Plant has an admitted program template");
    limited_state
        .data
        .storage
        .set_global("probe_depth", Value::Int(4));
    assert_eq!(
        limited_state.execute_pou(limited_pou, Some(limited_plant)),
        Err(VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(limited_state.data.storage.execution_frame_count(), 0);
    assert_eq!(limited_state.data.storage.execution_activation_count(), 0);
    assert_eq!(limited_state.data.storage.execution_frame_owner_count(), 0);

    limited_state
        .data
        .resources
        .work_budget
        .reset(limited.limits.max_work);
    limited_state
        .data
        .storage
        .set_global("probe_depth", Value::Int(2));
    limited_state
        .execute_pou(limited_pou, Some(limited_plant))
        .unwrap();
    assert_eq!(
        limited_state
            .data
            .storage
            .get_instance_var(limited_plant, "depth_result"),
        Some(&Value::Int(2))
    );
    assert_eq!(limited_state.data.storage.execution_frame_count(), 0);
    assert_eq!(limited_state.data.storage.execution_activation_count(), 0);
    assert_eq!(limited_state.data.storage.execution_frame_owner_count(), 0);
}

#[cfg(feature = "register-ir")]
#[test]
fn explicit_stack_root_yields_to_distinct_register_callees() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits {
            max_call_depth: 4,
            ..crate::vm::PreparationLimits::default()
        },
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.storage.set_global("probe_depth", Value::Int(2));
    state.data.resources.register.set_profile_enabled(true);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("program root")
    };
    let plant = *plant;
    let pou = *state
        .data
        .construction
        .instance_templates
        .get(&plant)
        .expect("Plant has an admitted program template");

    crate::vm::dispatch::execute_pou_stack_with_locals(
        &mut state,
        &prepared.vm,
        pou,
        Some(plant),
        None,
        false,
        0,
        crate::vm::budget::ExecutionEntry::Nested,
    )
    .unwrap();

    assert!(
        state
            .data
            .resources
            .register
            .profile_snapshot()
            .register_programs_executed
            > 0,
        "the explicit stack root must yield eligible child calls into the register pump"
    );
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
}

#[cfg(feature = "register-ir")]
#[test]
fn nested_call_work_allowance_is_shared_across_all_dispatch_modes() {
    use crate::error::{ExecutionLimitKind, RuntimeError};
    use crate::vm::register_ir::RegisterExecutionConfiguration;

    struct Mode {
        debug: bool,
        tier1: bool,
    }
    impl services::ExecutionServices for Mode {
        fn register_configuration(&self) -> RegisterExecutionConfiguration {
            RegisterExecutionConfiguration {
                profile_enabled: true,
                tier1_enabled: self.tier1,
                tier1_hot_threshold: 1,
                ..RegisterExecutionConfiguration::default()
            }
        }

        fn debug_active(&self) -> bool {
            self.debug
        }

        fn deadline_exceeded(&self) -> bool {
            false
        }

        fn has_wall_clock(&self) -> bool {
            false
        }

        fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
            Err(RuntimeError::ProfileUnsupported("clock".into()))
        }
    }

    fn run(
        prepared: &PreparedModule,
        mode: &Mode,
        work_limit: usize,
    ) -> (Result<(), RuntimeError>, usize, u64, usize, usize, usize) {
        let mut data = EngineState::construct_data(prepared, 0, mode).unwrap();
        let mut state = EngineState::borrow(&mut data, prepared, mode);
        state.data.storage.set_global("probe_depth", Value::Int(2));
        state.data.resources.work_budget.reset(work_limit);
        let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
            panic!("program root")
        };
        let plant = *plant;
        let pou = *state
            .data
            .construction
            .instance_templates
            .get(&plant)
            .expect("Plant has an admitted program template");
        let result = state.execute_pou(pou, Some(plant));
        let remaining = state.data.resources.work_budget.remaining();
        let tier1_blocks = state
            .data
            .resources
            .register
            .tier1_snapshot()
            .block_executions;
        let frames = state.data.storage.execution_frame_count();
        let activations = state.data.storage.execution_activation_count();
        let owners = state.data.storage.execution_frame_owner_count();
        (result, remaining, tier1_blocks, frames, activations, owners)
    }

    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let modes = [
        Mode {
            debug: true,
            tier1: false,
        },
        Mode {
            debug: false,
            tier1: false,
        },
        Mode {
            debug: false,
            tier1: true,
        },
    ];

    for mode in &modes {
        let full_limit = prepared.limits.max_work;
        let (result, remaining, tier1_blocks, frames, activations, owners) =
            run(&prepared, mode, full_limit);
        result.expect("the admitted nested-call path fits the default allowance");
        assert_eq!(frames, 0);
        assert_eq!(activations, 0);
        assert_eq!(owners, 0);
        if mode.tier1 {
            assert!(tier1_blocks > 0, "the tier-1 path must execute a block");
        }
        let consumed = full_limit - remaining;
        assert!(consumed > 1, "the nested fixture must consume work");

        let (result, remaining, _, frames, activations, owners) = run(&prepared, mode, consumed);
        result.expect("the measured complete-path allowance must succeed");
        assert_eq!(remaining, 0);
        assert_eq!(frames, 0);
        assert_eq!(activations, 0);
        assert_eq!(owners, 0);

        let (result, _, _, frames, activations, owners) = run(&prepared, mode, consumed - 1);
        assert!(matches!(
            result,
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                limit,
                ..
            }) if limit == consumed - 1
        ));
        assert_eq!(frames, 0, "a refused nested call must retire every frame");
        assert_eq!(
            activations, 0,
            "a refused nested call must release every activation"
        );
        assert_eq!(owners, 0, "a refused nested call must release frame owners");
    }
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
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
    state.data.storage.set_global("probe_depth", Value::Int(4));
    assert_eq!(
        state.execute_cycle(Duration::from_millis(10)),
        Err(VmTrap::CallStackOverflow.into_runtime_error())
    );
    assert_eq!(state.data.storage.execution_activation_count(), 0);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    assert_eq!(state.data.storage.execution_frame_owner_count(), 0);
    assert!(state.data.construction.initializers.is_empty());
    state.data.storage.set_global("probe_depth", Value::Int(2));
    // Invoke the same shared entry after resetting the enclosing operation;
    // this checks cleanup without clearing or reconstructing the installed state.
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    let Some(Value::Instance(plant)) = state.data.storage.get_global("Plant") else {
        panic!("program root")
    };
    let plant = *plant;
    let pou = *state
        .data
        .construction
        .instance_templates
        .get(&plant)
        .unwrap();
    state.execute_pou(pou, Some(plant)).unwrap();
    assert_eq!(
        state.data.storage.get_instance_var(plant, "depth_result"),
        Some(&Value::Int(2))
    );
    assert_eq!(state.data.storage.execution_activation_count(), 0);
}

#[test]
fn forged_initializer_calls_reject_before_binding_and_preserve_lexical_locals() {
    let bytes =
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
    let prepared =
        PreparedModule::from_bytes(bytes, crate::vm::PreparationLimits::default()).unwrap();
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
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
        assert_eq!(state.data.storage.execution_frame_count(), 0);
        assert_eq!(state.data.storage.execution_activation_count(), 0);
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
    let mut state_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let mut state = EngineState::borrow(&mut state_data, &prepared, &services::LOGICAL_ONLY);
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
    state.data.resources.buffers.clear();
    state
        .data
        .resources
        .construction_bytes
        .set(prepared.limits.max_construction_bytes);
    assert!(matches!(
        crate::vm::dispatch::execute_initializer_body(&mut state, &prepared.vm, &mut frame, 0),
        Err(RuntimeError::ExecutionLimit {
            kind: crate::error::ExecutionLimitKind::AllocationBytes,
            ..
        })
    ));
    assert_eq!(frame.locals, vec![Value::Int(23)]);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
    state.data.resources.construction_bytes.set(0);
    state
        .data
        .resources
        .work_budget
        .reset(prepared.limits.max_work);
    crate::vm::dispatch::execute_initializer_body(&mut state, &prepared.vm, &mut frame, 0).unwrap();
    assert_eq!(frame.locals, vec![Value::Int(23)]);
    assert_eq!(state.data.storage.execution_frame_count(), 0);
}
