//! Indexed retained-label projection; live instance identities are resolved on use.
use super::{RuntimeMetadata, SmolStr, VariableStorage};
use crate::value::{Value, ValueRef};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub(in crate::runtime) struct RetainedLabel(pub(in crate::runtime) SmolStr);
#[derive(Debug, Clone)]
pub(in crate::runtime) struct RetainedBinding {
    pub(in crate::runtime) root: Option<SmolStr>,
    pub(in crate::runtime) name: SmolStr,
}
struct LabelQuery<'a>(&'a str);
fn hash_label(name: &str, state: &mut impl Hasher) {
    for byte in name.bytes() {
        state.write_u8(byte.to_ascii_uppercase());
    }
    state.write_u8(0xff);
}
impl Hash for RetainedLabel {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_label(&self.0, state);
    }
}
impl PartialEq for RetainedLabel {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}
impl Eq for RetainedLabel {}
impl Hash for LabelQuery<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_label(self.0, state);
    }
}
impl indexmap::Equivalent<RetainedLabel> for LabelQuery<'_> {
    fn equivalent(&self, key: &RetainedLabel) -> bool {
        self.0.eq_ignore_ascii_case(&key.0)
    }
}
impl RetainedBinding {
    fn resolve(&self, storage: &VariableStorage) -> Option<ValueRef> {
        match &self.root {
            Some(root) => {
                let Value::Instance(id) = storage.get_global(root)? else {
                    return None;
                };
                storage.ref_for_instance(*id, &self.name)
            }
            None => storage.ref_for_global(&self.name),
        }
    }
}
impl RuntimeMetadata {
    /// Resolve a declared retained label without allocating or scanning other
    /// declarations. No cached live instance ID survives a restart/replacement.
    pub fn retained_reference(&self, storage: &VariableStorage, name: &str) -> Option<ValueRef> {
        self.retained_bindings
            .get(&LabelQuery(name))?
            .resolve(storage)
    }
    /// Preserve the control protocol's exact-name snapshot lookup.
    pub fn retained_reference_exact(
        &self,
        storage: &VariableStorage,
        name: &str,
    ) -> Option<ValueRef> {
        let (label, binding) = self.retained_bindings.get_key_value(&LabelQuery(name))?;
        (label.0.as_str() == name)
            .then(|| binding.resolve(storage))
            .flatten()
    }
    /// Project retained global/program destinations in stable declaration order.
    /// This allocating list is for presentation only, never single cycle writes.
    pub fn retained_references(&self, storage: &VariableStorage) -> Vec<(SmolStr, ValueRef)> {
        self.retained_bindings
            .iter()
            .filter_map(|(label, binding)| {
                binding
                    .resolve(storage)
                    .map(|reference| (label.0.clone(), reference))
            })
            .collect()
    }
}
