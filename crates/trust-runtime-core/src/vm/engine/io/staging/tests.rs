use super::*;
use crate::io_image::{IoAddressKey, IoValueCodec, PreparedIoBinding};
use alloc::vec;

#[test]
fn image_write_validation_accepts_each_area_without_mutating_flat_or_hierarchical_images() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let flat: Vec<_> = ["%IX0.0", "%QX0.0", "%MX0.0"]
        .into_iter()
        .map(|text| IoAddress::parse(text).unwrap())
        .collect();
    let hierarchical: Vec<_> = ["%IX1.2.3", "%QX1.2.3", "%MX1.2.3"]
        .into_iter()
        .map(|text| IoAddress::parse(text).unwrap())
        .collect();
    for address in &hierarchical {
        prepared.io_bindings.push(PreparedIoBinding {
            address: address.clone(),
            ref_idx: 0,
            type_id: None,
            codec: IoValueCodec::default(),
        });
    }
    let services = &super::super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    data.images.inputs = vec![0xA5];
    data.images.outputs = vec![0x5A];
    data.images.memory = vec![0x3C];
    let sentinel = IoAddressKey::from(&hierarchical[0]);
    data.images
        .hierarchical
        .insert(sentinel, Value::Bool(false));
    let inputs = data.images.inputs.clone();
    let outputs = data.images.outputs.clone();
    let memory = data.images.memory.clone();
    let hierarchy = data.images.hierarchical.clone();
    let mut engine = EngineState::borrow(&mut data, &prepared, services);

    for address in flat.iter().chain(&hierarchical) {
        engine
            .validate_cycle_image_write(address, Value::Bool(true))
            .unwrap();
    }

    assert_eq!(engine.data.images.inputs, inputs);
    assert_eq!(engine.data.images.outputs, outputs);
    assert_eq!(engine.data.images.memory, memory);
    assert_eq!(engine.data.images.hierarchical, hierarchy);
}

/// A copied binding's window, which preparation checks against the image's limits, is still checked
/// against the image's length at every scan (checklist P1-6): with images shorter than the
/// fixture's `%IX0.0` and `%QX0.0`, the sample and the publication are refused as an access past the
/// admitted image, as the checks of a process-image access refuse them, and the output image stays
/// as it was. Catches a scan that trusts the window and reads or writes past the image, which would
/// refuse with another error or panic.
#[test]
fn a_copied_binding_past_a_shorter_image_is_refused_as_before() {
    let prepared = PreparedModule::from_bytes(
        include_bytes!("../../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let leaves: Vec<_> = prepared.io_leaves.iter().flatten().collect();
    assert_eq!(leaves.len(), 2, "the fixture binds button and led");
    assert!(
        leaves.iter().all(|leaf| leaf.copy && leaf.window.is_some()),
        "both bindings are copied bits with a window: {leaves:?}"
    );
    let services = &super::super::super::services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    data.images.inputs.clear();
    data.images.outputs.clear();
    let mut engine = EngineState::borrow(&mut data, &prepared, services);
    assert!(engine.data.construction.initializers.is_empty());
    let past = RuntimeError::InvalidIoAddress("address exceeds admitted process image".into());
    assert_eq!(engine.sample_input_image(), Err(past.clone()));
    assert_eq!(engine.publish_output_image(), Err(past));
    assert!(engine.data.images.outputs.is_empty());
}
