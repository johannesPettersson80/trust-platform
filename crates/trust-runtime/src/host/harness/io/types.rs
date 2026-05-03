#[derive(Debug, Clone)]
struct IoLeafBinding {
    reference: crate::value::ValueRef,
    offset_bytes: u64,
    bit_offset: u8,
    size: crate::io::IoSize,
    value_type: TypeId,
}

#[derive(Debug, Clone)]
pub(super) struct InstanceBinding {
    pub(super) reference: crate::value::ValueRef,
    pub(super) type_id: TypeId,
    pub(super) address: IoAddress,
    pub(super) display_name: SmolStr,
}

#[derive(Debug, Clone)]
enum FieldAddress {
    Relative { offset_bytes: u64, bit_offset: u8 },
    Absolute(IoAddress),
}

pub(super) fn bind_value_ref_to_address(
    io: &mut crate::io::IoInterface,
    registry: &TypeRegistry,
    reference: crate::value::ValueRef,
    type_id: TypeId,
    address: &IoAddress,
    display_name: Option<SmolStr>,
) -> Result<(), CompileError> {
    let mut bindings = Vec::new();
    collect_io_bindings(registry, type_id, reference, 0, 0, &mut bindings)?;
    for binding in bindings {
        let target = offset_address(
            address,
            binding.offset_bytes,
            binding.size,
            binding.bit_offset,
        )?;
        if let Some(name) = display_name.clone() {
            io.bind_ref_named_typed(binding.reference, target, binding.value_type, name);
        } else {
            io.bind_ref_typed(binding.reference, target, binding.value_type);
        }
    }
    Ok(())
}

pub(super) fn join_instance_path(prefix: &SmolStr, name: &SmolStr) -> SmolStr {
    if prefix.is_empty() {
        name.clone()
    } else {
        SmolStr::new(format!("{prefix}.{name}"))
    }
}
