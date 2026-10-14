//! Retain scope names project ordinary admitted storage, never a second live bank.
use smol_str::SmolStr;
use trust_runtime::{
    debug::DebugControl,
    error::RuntimeError,
    memory::{MemoryLocation, VariableStorage},
    value::{Value, ValueRef},
    RuntimeMetadata,
};

pub(in crate::adapter) fn references(
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
) -> Vec<(SmolStr, ValueRef)> {
    let mut references = metadata.retained_references(storage);
    // Explicit detached inspection fixtures may still contain a legacy bank.
    // An installed source-free controller never creates that bank.
    references.extend(
        storage
            .retain()
            .iter()
            .enumerate()
            .map(|(offset, (name, _))| {
                (
                    name.clone(),
                    ValueRef {
                        location: MemoryLocation::Retain,
                        offset,
                        path: Vec::new(),
                    },
                )
            }),
    );
    references
}

pub(in crate::adapter) fn reference(
    metadata: &RuntimeMetadata,
    storage: &VariableStorage,
    name: &str,
) -> Option<ValueRef> {
    metadata.retained_reference(storage, name).or_else(|| {
        // Detached legacy inspection fixtures only; never an installed bank.
        storage
            .retain()
            .iter()
            .enumerate()
            .find(|(_, (label, _))| label.eq_ignore_ascii_case(name))
            .map(|(offset, _)| ValueRef {
                location: MemoryLocation::Retain,
                offset,
                path: Vec::new(),
            })
    })
}

fn destination_name<'a>(
    storage: &'a VariableStorage,
    reference: &ValueRef,
) -> Result<&'a str, RuntimeError> {
    if !reference.path.is_empty() {
        return Err(RuntimeError::InvalidAlias);
    }
    let values = match reference.location {
        MemoryLocation::Global => storage.globals(),
        MemoryLocation::Instance(id) => {
            &storage
                .get_instance(id)
                .ok_or(RuntimeError::ReferenceLifetime)?
                .variables
        }
        MemoryLocation::Retain => storage.retain(),
        _ => return Err(RuntimeError::ReferenceLifetime),
    };
    values
        .get_index(reference.offset)
        .map(|(name, _)| name.as_str())
        .ok_or(RuntimeError::ReferenceLifetime)
}

pub(in crate::adapter) fn queue_write(
    control: &DebugControl,
    storage: &VariableStorage,
    reference: &ValueRef,
    value: Value,
) -> Result<(), RuntimeError> {
    let name = destination_name(storage, reference)?;
    match reference.location {
        MemoryLocation::Global => control.enqueue_global_write(name, value),
        MemoryLocation::Instance(id) => control.enqueue_instance_write(id, name, value),
        MemoryLocation::Retain => control.enqueue_retain_write(name, value),
        _ => return Err(RuntimeError::ReferenceLifetime),
    }
    Ok(())
}

pub(in crate::adapter) fn force(
    control: &DebugControl,
    storage: &VariableStorage,
    reference: &ValueRef,
    value: Option<Value>,
) -> Result<(), RuntimeError> {
    let name = destination_name(storage, reference)?;
    match (reference.location, value) {
        (MemoryLocation::Global, Some(value)) => control.force_global(name, value),
        (MemoryLocation::Global, None) => control.release_global(name),
        (MemoryLocation::Instance(id), Some(value)) => control.force_instance(id, name, value),
        (MemoryLocation::Instance(id), None) => control.release_instance(id, name),
        (MemoryLocation::Retain, Some(value)) => control.force_retain(name, value),
        (MemoryLocation::Retain, None) => control.release_retain(name),
        _ => return Err(RuntimeError::ReferenceLifetime),
    }
    Ok(())
}
