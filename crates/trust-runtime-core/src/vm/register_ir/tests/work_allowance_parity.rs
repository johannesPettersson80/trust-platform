//! TIME-08: the stack, register and tier-1 executors charge the same work for the same execution,
//! at every budget prefix, not only at the prefixes where they refuse.
use super::*;
use crate::error::{ExecutionLimitKind, RuntimeError};

/// The failure boundary of one run: admitted, or the kind, `used` and `limit` of the refused
/// charge. `requested` is left out on purpose: it is the size of the failed charge, and the
/// register executor charges a fused operation at once.
#[derive(Debug, PartialEq)]
enum Boundary {
    Admitted,
    Refused(ExecutionLimitKind, usize, usize),
    Other(RuntimeError),
}

/// One run: its failure boundary, the work left, and the globals the program stores.
#[derive(Debug, PartialEq)]
struct Outcome {
    boundary: Boundary,
    remaining: usize,
    g0: Option<Value>,
    g1: Option<Value>,
}

fn boundary(result: Result<(), RuntimeError>) -> Boundary {
    match result {
        Ok(()) => Boundary::Admitted,
        Err(RuntimeError::ExecutionLimit {
            kind, used, limit, ..
        }) => Boundary::Refused(kind, used, limit),
        Err(other) => Boundary::Other(other),
    }
}

fn stack_outcome(module: &VmModule, budget: usize, g0: i32) -> Outcome {
    let mut stack = Context::new(module);
    stack.storage.set_global("g0", Value::DInt(g0));
    stack.storage.set_global("g1", Value::DInt(-1));
    // TIME-08 charges no scalar copy, so the harness charges instructions only, like the engine.
    stack.charge_clone_work = false;
    stack.budget.reset(budget);
    let result = crate::vm::dispatch::execute_pou_stack_with_locals(
        &mut stack,
        module,
        1,
        None,
        None,
        false,
        0,
        super::super::super::budget::ExecutionEntry::Nested,
    )
    .map(|_| ());
    Outcome {
        boundary: boundary(result),
        remaining: stack.budget.remaining(),
        g0: stack.storage.get_global("g0").cloned(),
        g1: stack.storage.get_global("g1").cloned(),
    }
}

fn register_outcome(module: &VmModule, budget: usize, tier1_enabled: bool, g0: i32) -> Outcome {
    let mut optimized = Context::new(module);
    optimized.storage.set_global("g0", Value::DInt(g0));
    optimized.storage.set_global("g1", Value::DInt(-1));
    optimized.charge_clone_work = false;
    optimized.tier1.set_enabled(tier1_enabled);
    optimized.tier1.hot_block_threshold = 1;
    optimized.budget.reset(budget);
    let result = try_execute_pou_with_register_ir_with_locals(
        &mut optimized,
        module,
        1,
        None,
        None,
        false,
        0,
        super::super::super::budget::ExecutionEntry::Nested,
    )
    .map(|_| ());
    Outcome {
        boundary: boundary(result),
        remaining: optimized.budget.remaining(),
        g0: optimized.storage.get_global("g0").cloned(),
        g1: optimized.storage.get_global("g1").cloned(),
    }
}

#[test]
fn fused_dint_remainder_charges_the_same_work_on_every_executor_at_each_prefix() {
    let code = vec![
        0x20, 0, 0, 0, 0, // LOAD_REF g0
        0x10, 0, 0, 0, 0,    // LOAD_CONST -1
        0x44, // MOD
        0x21, 0, 0, 0, 0,    // STORE_REF g0
        0x06, // RETURN
    ];
    let module = module(code, Value::DInt(-1));
    let lowered = lower_pou_to_register_ir(&module, 1).unwrap();
    // The fused lowering must be the path under test, not a fallback to stack execution.
    assert!(matches!(
        lowered.blocks[0].instructions.as_slice(),
        [
            RegisterInstr::BinaryRefConstToRef { .. },
            RegisterInstr::Return
        ]
    ));

    for budget in 0..=7 {
        let stack = stack_outcome(&module, budget, i32::MIN);
        for tier1_enabled in [false, true] {
            // `requested` is the failed charge's size; fused operations are charged at once.
            assert_eq!(
                register_outcome(&module, budget, tier1_enabled, i32::MIN),
                stack,
                "budget={budget}, tier1={tier1_enabled}"
            );
        }
    }
}

#[test]
fn repeated_operands_charge_the_same_work_on_stack_register_and_tier1_at_each_prefix() {
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
        let stack = stack_outcome(&module, budget, 7);
        // `requested` is the failed charge's size; fused operations are charged at once.
        assert_eq!(
            register_outcome(&module, budget, false, 7),
            stack,
            "budget={budget}"
        );
        assert_eq!(
            register_outcome(&module, budget, true, 7),
            stack,
            "budget={budget}, tier1"
        );

        if budget == 16 {
            // TIME-08: one unit per executed original instruction (LOAD_REF, DUP, ADD,
            // STORE_REF, RETURN), so five of the sixteen units are consumed.
            assert_eq!(stack.boundary, Boundary::Admitted);
            assert_eq!(stack.remaining, 11);
            assert_eq!(stack.g1, Some(Value::DInt(14)));
        }
    }
}

#[test]
fn for_zero_step_trap_charges_the_same_work_on_every_executor_at_each_prefix() {
    // LOAD_CONST TRUE; JMP_FALSE past the trap; HALT; RETURN, as the encoder lays out a zero-step
    // guard. The register trap is charged one unit before it faults, as the stack HALT is.
    let mut code = vec![0x10, 0, 0, 0, 0, 0x04];
    code.extend_from_slice(&1_i32.to_le_bytes());
    code.extend_from_slice(&[0x01, 0x06]);
    let module = module(code, Value::Bool(true));
    let lowered = lower_pou_to_register_ir(&module, 1).unwrap();
    assert!(matches!(
        lowered.blocks[1].instructions.as_slice(),
        [RegisterInstr::ForStepZero]
    ));

    for budget in 0..=6 {
        let stack = stack_outcome(&module, budget, i32::MIN);
        for tier1_enabled in [false, true] {
            assert_eq!(
                register_outcome(&module, budget, tier1_enabled, i32::MIN),
                stack,
                "budget={budget}, tier1={tier1_enabled}"
            );
        }
        if budget == 6 {
            // LOAD_CONST, JMP_FALSE and HALT are charged before the trap, leaving three units.
            assert_eq!(stack.boundary, Boundary::Other(RuntimeError::ForStepZero));
            assert_eq!(stack.remaining, 3);
        }
    }
}
