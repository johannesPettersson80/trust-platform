//! Hierarchical process-image values across a resource-only commit (spec 34 HOST-09).
#![cfg(feature = "std")]
use std::sync::Arc;
use trust_runtime_core::{
    bytecode::{BytecodeModule, IoBinding, SectionData, SectionId},
    io_address::IoAddress,
    value::Value,
    vm::{PreparationLimits, PreparedModule},
};

const GPIO: &[u8] = include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

/// The F401 GPIO fixture with bound hierarchical input, output and marker bindings at
/// %IX3.4.0, %QX3.4.0 and %MX3.4.0. The binding setup follows `checked_owner_images.rs`.
fn hierarchy_module() -> BytecodeModule {
    let mut module = BytecodeModule::decode(GPIO).unwrap();
    let Some(SectionData::IoMap(io)) = module.section(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let input = io
        .bindings
        .iter()
        .find(|binding| strings.entries[binding.address_str_idx as usize].starts_with("%I"))
        .unwrap()
        .clone();
    let output = io
        .bindings
        .iter()
        .find(|binding| strings.entries[binding.address_str_idx as usize].starts_with("%Q"))
        .unwrap()
        .clone();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let first = strings.entries.len() as u32;
    for name in ["%IX3.4.0", "%QX3.4.0", "%MX3.4.0"] {
        strings.entries.push(name.into());
    }
    let Some(SectionData::IoMap(io)) = module.section_mut(SectionId::IoMap) else {
        panic!("I/O metadata")
    };
    io.bindings.push(IoBinding {
        address_str_idx: first,
        ..input
    });
    io.bindings.push(IoBinding {
        address_str_idx: first + 1,
        ..output.clone()
    });
    io.bindings.push(IoBinding {
        address_str_idx: first + 2,
        ..output
    });
    module
}

/// Spec 34 HOST-09, resource-only row: the flat images are resized by the commit, but a
/// hierarchical image value written before it keeps its value, both through the image
/// reader and through the output reader.
#[test]
fn resource_only_commit_keeps_hierarchical_image_values() {
    let limits = PreparationLimits::default();
    let mut raw = hierarchy_module();
    let prepared = PreparedModule::from_decoded(&raw, limits).unwrap();
    let mut state = Arc::new(prepared).instantiate_owned(0).unwrap();
    let address = IoAddress::parse("%QX3.4.0").unwrap();
    state.write_image(&address, Value::Bool(true)).unwrap();
    assert_eq!(
        state.read_hierarchical_image_ref(&address),
        Some(&Value::Bool(true))
    );
    let Some(SectionData::ResourceMeta(meta)) = raw.section_mut(SectionId::ResourceMeta) else {
        panic!("resource")
    };
    meta.resources[0].outputs_size += 8;
    let next = Arc::new(PreparedModule::from_decoded(&raw, limits).unwrap());
    state
        .prepare_resource_reconfiguration(next, 0, true)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        state.read_hierarchical_image_ref(&address),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        state.read_hierarchical_output(&address),
        Some(&Value::Bool(true))
    );
}
