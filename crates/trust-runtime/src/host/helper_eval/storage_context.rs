//! Type context for detached helper evaluation, including nested indices.
use trust_hir::types::TypeRegistry;

pub(super) struct EvaluationTypes<'a> {
    pub(super) registry: &'a TypeRegistry,
}
impl std::ops::Deref for EvaluationTypes<'_> {
    type Target = TypeRegistry;
    fn deref(&self) -> &Self::Target {
        self.registry
    }
}
