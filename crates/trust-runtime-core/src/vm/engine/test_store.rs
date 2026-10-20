//! What the engine's unit tests read and seed through the store-access seam, on the flat store that
//! every state holds (checklist C1-19e): a global's value, an instance variable's value, the value a
//! reference names, and a global written with its declared type.

use super::EngineState;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{InstanceId, VariableStorage};
use crate::value::{Value, ValueRef};

/// The value of the global `name`, or `None` when the program declares no such global.
pub(super) fn global(state: &EngineState<'_>, name: &str) -> Option<Value> {
    let view = StoreAccess::read_global(state, name)?;
    Some(StoreAccess::materialize(state, &view).expect("the global reads"))
}

/// The value of the variable `name` of `instance`, or `None` when it has no such variable.
pub(super) fn instance_var(
    state: &EngineState<'_>,
    instance: InstanceId,
    name: &str,
) -> Option<Value> {
    let view = StoreAccess::read_instance_field(state, instance, name).ok()?;
    Some(StoreAccess::materialize(state, &view).expect("the variable reads"))
}

/// The value `reference` names, or `None` when it names nothing.
pub(super) fn read(state: &EngineState<'_>, reference: &ValueRef) -> Option<Value> {
    let view = StoreAccess::read_reference(state, reference.as_view()).ok()?;
    Some(StoreAccess::materialize(state, &view).expect("the target reads"))
}

/// Write `value`, of the global's declared type, into the global `name`.
pub(super) fn set_global(state: &mut EngineState<'_>, name: &str, value: Value) {
    assert!(
        StoreMutate::set_global(state, name, value),
        "the global {name} takes the value"
    );
}

/// The instance the global `name` names: a program root or a function-block variable.
pub(super) fn global_instance(state: &EngineState<'_>, name: &str) -> InstanceId {
    match global(state, name) {
        Some(Value::Instance(instance)) => instance,
        other => panic!("the global {name} names no instance: {other:?}"),
    }
}

/// The storage as the value store holds it: every global and every instance read from the flat
/// store's blocks, as `RuntimeState::storage` shows them.
pub(super) fn projection(state: &EngineState<'_>) -> VariableStorage {
    state
        .data
        .reader(state.prepared)
        .project_storage()
        .expect("the store reads")
}
