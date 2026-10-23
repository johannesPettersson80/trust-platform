use super::super::{services, EngineState};
use crate::vm::{call::context::CallContext, PreparationLimits, PreparedModule};

fn prepared() -> PreparedModule {
    PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap()
}

#[test]
fn engine_uses_canonical_pou_id_and_keeps_foreign_module_name_fallback() {
    let prepared = prepared();
    let counter_id = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
    let main_id = *prepared
        .vm
        .program_ids
        .iter()
        .find_map(|(name, id)| name.eq_ignore_ascii_case("Main").then_some(id))
        .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

    let counter_inputs = state.edge_inputs_for_pou(&prepared.vm, counter_id, "not-counter");
    assert_eq!(counter_inputs.len(), 1);
    assert_eq!(counter_inputs[0].name.as_str(), "pulse");
    assert!(counter_inputs[0].rising);

    // Main has no edge inputs. Supplying the valid Counter name ensures a
    // canonical lookup uses Main's prepared ID instead of the name fallback.
    assert!(state
        .edge_inputs_for_pou(&prepared.vm, main_id, "Counter")
        .is_empty());

    // A noncanonical module still uses the existing owner-based adapter path.
    let foreign_module = prepared.vm.clone();
    let fallback = state.edge_inputs_for_pou(&foreign_module, counter_id, "Counter");
    assert_eq!(fallback.len(), 1);
    assert_eq!(fallback[0].name, counter_inputs[0].name);
    assert_eq!(fallback[0].phase_name, counter_inputs[0].phase_name);
    assert_eq!(fallback[0].rising, counter_inputs[0].rising);
}
