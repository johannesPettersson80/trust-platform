//! Owner, root and instance ancestry resolution consume no execution allowance (TIME-08).
use super::super::{services, EngineData};
use super::*;

fn fixture() -> (
    PreparedModule,
    alloc::boxed::Box<EngineData>,
    [InstanceId; 3],
) {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    // Three identities with private templates: the resolutions these tests take read the
    // templates and the roots, never an instance's variables.
    data.construction.instance_templates = Default::default();
    let ids = [InstanceId(1000), InstanceId(1001), InstanceId(1002)];
    for (id, template) in [(ids[0], 1000), (ids[1], 1001), (ids[2], 9000)] {
        data.construction
            .instance_templates
            .insert(id, template)
            .unwrap();
    }
    prepared.method_owners.clear();
    prepared
        .method_owners
        .extend([(42, 9000), (100, 1000), (101, 1001)]);
    (prepared, data, ids)
}
fn frame(instance: InstanceId, pou: Option<u32>, owner: Option<u32>) -> VmFrame {
    VmFrame {
        pou_id: pou,
        activation: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: Some(instance),
        instance_owner: owner,
    }
}
#[test]
fn method_fallback_and_each_ancestry_search_consume_no_allowance() {
    // The cell fixture's `fb_g : Derived` is a persistent instance whose inheritance parent, a
    // `Base`, lies in its range (specification 34 §6.2). A method that `Base` owns, called on
    // `fb_g` through an owner no root names, resolves by the method fallback and one parent hop,
    // which is bookkeeping (TIME-08): an empty allowance resolves the `Base` part and stays empty.
    use crate::memory::cell_access::fixture::{self as cells, BASE_POU, DERIVED_POU};
    use crate::memory::view::StoreAccess;
    let mut prepared = cells::prepared();
    prepared.method_owners.clear();
    prepared.method_owners.extend([(42, BASE_POU)]);
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let derived = super::super::test_store::global_instance(&state, "fb_g");
    let base = StoreAccess::instance_parent(&state, derived)
        .unwrap()
        .expect("the Base part of fb_g");
    let owner_frame = frame(derived, Some(42), Some(u32::MAX));
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state
            .resolve_instance_owner(u32::MAX, &owner_frame)
            .map_err(VmTrap::into_runtime_error),
        Ok(base)
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    assert_eq!(
        StoreAccess::instance_parent(&state, derived).unwrap(),
        Some(base)
    );
    assert_eq!(
        state
            .data
            .construction
            .instance_templates
            .get(&derived)
            .copied(),
        Some(DERIVED_POU)
    );
    assert_eq!(
        state
            .data
            .construction
            .instance_templates
            .get(&base)
            .copied(),
        Some(BASE_POU)
    );
}

#[test]
fn no_template_and_nonmatching_owner_skip_the_ancestry_walk() {
    let (prepared, mut data, ids) = fixture();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state
            .resolve_instance_owner(u32::MAX, &frame(ids[0], None, Some(u32::MAX)))
            .map_err(VmTrap::into_runtime_error),
        Ok(ids[0])
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
    let (root_index, root_entry) = prepared
        .roots
        .entries
        .iter()
        .enumerate()
        .find(|(_, root)| root.instance_owner_id.is_some())
        .expect("GPIO has an owned root");
    let encoded = root_entry.instance_owner_id.unwrap();
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state
            .resolve_instance_owner(encoded, &frame(ids[0], Some(42), None))
            .map_err(VmTrap::into_runtime_error),
        Ok(state.data.construction.roots[root_index].unwrap())
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}

#[test]
fn matching_root_template_resolves_without_method_fallback_or_charge() {
    let (prepared, mut data, ids) = fixture();
    let root_entry = prepared
        .roots
        .entries
        .iter()
        .find(|root| root.instance_owner_id.is_some())
        .expect("GPIO has an owned root");
    let encoded = root_entry.instance_owner_id.unwrap();
    let template = root_entry.template_pou_id.unwrap();
    data.construction
        .instance_templates
        .insert(ids[0], template)
        .unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    state.data.resources.work_budget.reset(0);
    assert_eq!(
        state
            .resolve_instance_owner(encoded, &frame(ids[0], Some(42), Some(encoded)))
            .map_err(VmTrap::into_runtime_error),
        Ok(ids[0])
    );
    assert_eq!(state.data.resources.work_budget.remaining(), 0);
}
