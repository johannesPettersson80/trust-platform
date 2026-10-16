//! Retain scope names project ordinary admitted storage, never a second live bank.
use smol_str::SmolStr;
use trust_runtime::{
    debug::{DebugControl, ForcedVarTarget},
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

/// Queue a one-shot write to a retained or global destination. A full request queue refuses it
/// with the stable message `request queue full`, and nothing is queued.
pub(in crate::adapter) fn queue_write(
    control: &DebugControl,
    storage: &VariableStorage,
    reference: &ValueRef,
    value: Value,
) -> Result<(), String> {
    let name = destination_name(storage, reference).map_err(|error| error.to_string())?;
    let queued = match reference.location {
        MemoryLocation::Global => control.try_enqueue_global_write(name, value),
        MemoryLocation::Instance(id) => control.try_enqueue_instance_write(id, name, value),
        MemoryLocation::Retain => control.try_enqueue_retain_write(name, value),
        _ => return Err(RuntimeError::ReferenceLifetime.to_string()),
    };
    queued.map_err(|error| error.to_string())
}

/// The force a destination is keyed by in the debug control. Only retained, global and instance
/// destinations can be forced.
fn force_target(reference: &ValueRef, name: &str) -> Option<ForcedVarTarget> {
    Some(match reference.location {
        MemoryLocation::Global => ForcedVarTarget::Global(name.into()),
        MemoryLocation::Instance(id) => ForcedVarTarget::Instance(id, name.into()),
        MemoryLocation::Retain => ForcedVarTarget::Retain(name.into()),
        _ => return None,
    })
}

/// Force, or release when `value` is `None`, a retained, global or instance destination. Returns
/// the value the destination had forced before this call (`None` when it was not forced), so a
/// caller whose later checked write fails can put it back with `restore_force`. A full request
/// queue refuses a new force with `request queue full`, and the force set is then unchanged.
pub(in crate::adapter) fn force(
    control: &DebugControl,
    storage: &VariableStorage,
    reference: &ValueRef,
    value: Option<Value>,
) -> Result<Option<Value>, String> {
    let name = destination_name(storage, reference).map_err(|error| error.to_string())?;
    let previous =
        force_target(reference, name).and_then(|target| control.forced_var_value(&target));
    let forced = match (reference.location, value) {
        (MemoryLocation::Global, Some(value)) => control.try_force_global(name, value),
        (MemoryLocation::Global, None) => {
            control.release_global(name);
            Ok(())
        }
        (MemoryLocation::Instance(id), Some(value)) => control.try_force_instance(id, name, value),
        (MemoryLocation::Instance(id), None) => {
            control.release_instance(id, name);
            Ok(())
        }
        (MemoryLocation::Retain, Some(value)) => control.try_force_retain(name, value),
        (MemoryLocation::Retain, None) => {
            control.release_retain(name);
            Ok(())
        }
        _ => return Err(RuntimeError::ReferenceLifetime.to_string()),
    };
    forced.map(|()| previous).map_err(|error| error.to_string())
}

/// Put back a force that `force` returned as `previous`, after the checked write that followed it
/// failed. `None` releases the destination again.
pub(in crate::adapter) fn restore_force(
    control: &DebugControl,
    storage: &VariableStorage,
    reference: &ValueRef,
    previous: Option<Value>,
) -> Result<(), String> {
    force(control, storage, reference, previous).map(|_| ())
}

#[cfg(test)]
mod queue_refusal_tests {
    use super::*;
    use trust_runtime::harness::CompileSession;

    const COUNTER_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM Main
    Counter := Counter + 1;
END_PROGRAM
"#;

    #[test]
    fn retained_queue_write_refused_by_a_full_request_queue_changes_nothing() {
        let mut runtime = CompileSession::from_source(COUNTER_SOURCE)
            .build_runtime()
            .expect("fixture compiles");
        let control = DebugControl::new();
        runtime.set_debug_control(control.clone());
        for index in 0..1_024 {
            control
                .try_enqueue_global_write(format!("filler{index}"), Value::Int(0))
                .expect("a filler fits the bound");
        }
        let reference = runtime
            .storage()
            .ref_for_global("Counter")
            .expect("Counter has a reference");

        let error = queue_write(&control, runtime.storage(), &reference, Value::Int(99))
            .expect_err("a full queue refuses the write");

        assert_eq!(error, "request queue full");
        runtime.execute_cycle().expect("the scan runs");
        assert_eq!(
            runtime.storage().get_global("Counter"),
            Some(&Value::Int(1)),
            "the refused write must not reach Counter, so the scan only increments it"
        );
    }
}
