//! A prepared plan is a leaf exactly when its program makes no user call.
use crate::vm::{PreparationLimits, PreparedModule};

#[test]
fn leaf_plans_are_the_fallback_free_programs_without_user_calls() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap();
    let plans = &prepared.register_plans;
    let mut leaves = 0;
    for (id, plan) in &plans.entries {
        let super::super::RegisterLoweringCacheEntry::Ready(lowered) = &plan.lowered else {
            assert!(!plans.is_leaf(*id), "an unlowered plan cannot run in place");
            continue;
        };
        let user_call = lowered.program.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    super::super::RegisterInstr::CallNative { kind, .. }
                        if *kind != crate::bytecode::NATIVE_CALL_KIND_STDLIB
                )
            })
        });
        let expected = !user_call && plan.fallback_reason().is_none();
        assert_eq!(plans.is_leaf(*id), expected, "POU {id}");
        leaves += usize::from(expected);
    }
    // Main calls a function block, a standard function block and a method; the
    // method body (`value := value + delta; Advance := previous^;`) calls nothing.
    let main = (0..u32::MAX)
        .take(64)
        .find(|id| {
            prepared
                .vm
                .pou_name(*id)
                .is_some_and(|name| name.eq_ignore_ascii_case("Main"))
        })
        .expect("Main POU");
    assert!(!plans.is_leaf(main));
    assert!(leaves > 0, "the fixture method is a leaf");
    assert!(!plans.is_leaf(u32::MAX), "an unknown POU is not a leaf");
}
