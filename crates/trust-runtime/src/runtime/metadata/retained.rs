//! Retained-label projection over the shared Live Values module; the labels themselves are built
//! and resolved by `trust_runtime_services::live_values::retained_names` (specification 11,
//! "Live Values"; specification 34 §4.3.1).
use super::{RuntimeMetadata, SmolStr, VariableStorage};
use crate::value::ValueRef;
use trust_runtime_services::live_values::retained_names::{self as shared, RetainedNames};

pub(crate) use trust_runtime_services::live_values::retained_names::{
    RetainedBinding, RetainedLabel,
};

impl RuntimeMetadata {
    /// The retained labels of this installation, as the shared Live Values apply consumes them.
    pub(crate) fn retained_names(&self) -> &RetainedNames {
        &self.retained_bindings
    }
    /// Resolve a declared retained label without allocating or scanning other declarations. No
    /// cached live instance ID survives a restart/replacement (specification 11, "Live Values").
    pub fn retained_reference(&self, storage: &VariableStorage, name: &str) -> Option<ValueRef> {
        shared::retained_reference(&self.retained_bindings, storage, name)
    }
    /// Preserve the control protocol's exact-name snapshot lookup.
    pub fn retained_reference_exact(
        &self,
        storage: &VariableStorage,
        name: &str,
    ) -> Option<ValueRef> {
        shared::retained_reference_exact(&self.retained_bindings, storage, name)
    }
    /// Project retained global/program destinations in stable declaration order.
    /// This allocating list is for presentation only, never single cycle writes.
    pub fn retained_references(&self, storage: &VariableStorage) -> Vec<(SmolStr, ValueRef)> {
        shared::retained_references(&self.retained_bindings, storage)
    }
}
