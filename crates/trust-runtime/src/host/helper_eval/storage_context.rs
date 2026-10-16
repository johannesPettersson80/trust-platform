//! Selected storage ABI for detached helper evaluation, including nested indices.
use crate::value::StorageAbi;
use trust_hir::types::TypeRegistry;

pub(super) struct EvaluationTypes<'a> {
    pub(super) registry: &'a TypeRegistry,
    pub(super) abi: StorageAbi,
}
impl std::ops::Deref for EvaluationTypes<'_> {
    type Target = TypeRegistry;
    fn deref(&self) -> &Self::Target {
        self.registry
    }
}
