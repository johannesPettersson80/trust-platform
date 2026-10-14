// TIME-08: the stack, register and tier-1 executors fault at the same original instruction for
// every budget prefix of each fixture. This file is included into the register_ir test module,
// so it shares the helpers of backend_parity_followup.rs.

/// Two fixtures of five original instructions each. Both store once, at the fourth instruction:
/// `g0 := g0 + 1` and `g0 := g0 + g0`.
fn work_prefix_double_store_module() -> (VmModule, u32) {
    manual_vm_module(
        vec![
            0x20, 0, 0, 0, 0,    // LOAD_REF g0
            0x11, // DUP
            0x40, // ADD
            0x21, 0, 0, 0, 0,    // STORE_REF g0
            0x06, // RETURN
        ],
        vec![Value::DInt(1)],
        1,
    )
}

/// Builds a fixture module and returns it with its program's POU id.
type PrefixFixture = fn() -> (VmModule, u32);

/// The failure boundary of one run. `requested` is left out on purpose: it is the size of the
/// failed charge, and the register executor charges a fused operation at once.
#[derive(Debug, PartialEq)]
enum PrefixBoundary {
    Admitted,
    Refused(trust_runtime_core::error::ExecutionLimitKind, usize, usize),
    Other(RuntimeError),
}

fn prefix_boundary(result: Result<(), RuntimeError>) -> PrefixBoundary {
    match result {
        Ok(()) => PrefixBoundary::Admitted,
        Err(RuntimeError::ExecutionLimit {
            kind, used, limit, ..
        }) => PrefixBoundary::Refused(kind, used, limit),
        Err(other) => PrefixBoundary::Other(other),
    }
}

fn work_prefix_stack(build: PrefixFixture, budget: usize) -> (PrefixBoundary, Option<Value>) {
    let (mut module, pou_id) = build();
    module.set_legacy_instruction_budget(budget);
    let mut runtime = vmpar_seed_runtime();
    let result = execute_pou_stack_with_locals(
        &mut runtime,
        &module,
        pou_id,
        None,
        None,
        false,
        0,
        trust_runtime_core::vm::hosted::budget::ExecutionEntry::Root,
    )
    .map(|_| ());
    (
        prefix_boundary(result),
        runtime.storage().get_global("g0").cloned(),
    )
}

fn work_prefix_register(build: PrefixFixture, budget: usize) -> (PrefixBoundary, Option<Value>) {
    let (mut module, pou_id) = build();
    module.set_legacy_instruction_budget(budget);
    let mut runtime = vmpar_seed_runtime();
    let mut context = LegacyRegisterFixtureContext::new(&mut runtime, &module)
        .expect("prepare legacy register fixture");
    let result = try_execute_pou_with_register_ir(&mut context, &module, pou_id, None).map(|_| ());
    (
        prefix_boundary(result),
        context.storage().get_global("g0").cloned(),
    )
}

fn work_prefix_tier1(build: PrefixFixture, budget: usize) -> (PrefixBoundary, Option<Value>) {
    let (mut module, pou_id) = build();
    let mut runtime = vmpar_seed_runtime();
    runtime.set_vm_tier1_specialized_executor_enabled(true);
    runtime.reset_vm_tier1_specialized_executor();
    runtime.vm_tier1_specialized_executor.hot_block_threshold = 1;
    let mut context = LegacyRegisterFixtureContext::new(&mut runtime, &module)
        .expect("prepare legacy register fixture");
    try_execute_pou_with_register_ir(&mut context, &module, pou_id, None)
        .expect("tier1 warmup must compile the block");
    context.storage_mut().set_global("g0", Value::DInt(41));
    module.set_legacy_instruction_budget(budget);
    let result = try_execute_pou_with_register_ir(&mut context, &module, pou_id, None).map(|_| ());
    (
        prefix_boundary(result),
        context.storage().get_global("g0").cloned(),
    )
}

#[test]
fn every_budget_prefix_faults_at_the_same_instruction_on_all_executors_for_two_fixtures() {
    let fixtures: [(PrefixFixture, i32); 2] = [
        (vmpar_add_store_module, 42),
        (work_prefix_double_store_module, 82),
    ];
    for (build, stored) in fixtures {
        for budget in 0..=5 {
            let stack = work_prefix_stack(build, budget);
            // `requested` is the failed charge's size; fused operations charge at once.
            assert_eq!(
                work_prefix_register(build, budget),
                stack,
                "register budget={budget}"
            );
            assert_eq!(
                work_prefix_tier1(build, budget),
                stack,
                "tier1 budget={budget}"
            );

            // The store is the fourth instruction. RETURN is the fifth, so budgets 0..=3 stop
            // before the store and budget 4 stops at RETURN after it.
            let expected_value = if budget >= 4 { stored } else { 41 };
            assert_eq!(
                stack.1,
                Some(Value::DInt(expected_value)),
                "budget={budget}"
            );
            if budget < 5 {
                assert!(
                    matches!(
                        stack.0,
                        PrefixBoundary::Refused(
                            trust_runtime_core::error::ExecutionLimitKind::Work,
                            _,
                            _
                        )
                    ),
                    "budget={budget}: {:?}",
                    stack.0
                );
            } else {
                assert_eq!(stack.0, PrefixBoundary::Admitted, "budget={budget}");
            }
        }
    }
}
