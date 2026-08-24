use rustc_hash::FxHashMap;
use trust_hir::types::{InitializerId, TypeId};

use super::Expr;

#[derive(Debug, Clone, Default)]
pub struct InitializerCatalog {
    records: FxHashMap<InitializerId, Expr>,
    type_defaults: FxHashMap<TypeId, InitializerId>,
    next_id: u32,
}

impl InitializerCatalog {
    pub fn insert(&mut self, expr: Expr) -> InitializerId {
        let id = InitializerId(self.next_id);
        self.next_id += 1;
        self.records.insert(id, expr);
        id
    }

    pub fn set_type_default(&mut self, type_id: TypeId, initializer: InitializerId) {
        self.type_defaults.insert(type_id, initializer);
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&Expr> {
        self.records.get(&id)
    }

    pub fn type_default(&self, type_id: TypeId) -> Option<InitializerId> {
        self.type_defaults.get(&type_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::InitializerCatalog;
    use crate::program_model::Expr;
    use crate::value::Value;
    use trust_hir::TypeId;

    #[test]
    fn initializer_catalog_preserves_record_and_type_default_lookup() {
        let mut catalog = InitializerCatalog::default();

        let initializer = catalog.insert(Expr::Literal(Value::Int(7)));
        catalog.set_type_default(TypeId::INT, initializer);

        assert!(matches!(
            catalog.initializer(initializer),
            Some(Expr::Literal(Value::Int(7)))
        ));
        assert_eq!(catalog.type_default(TypeId::INT), Some(initializer));
    }

    #[test]
    fn initializer_catalog_preserves_sequence_missing_lookup_and_default_replacement() {
        let mut catalog = InitializerCatalog::default();

        assert!(catalog
            .initializer(trust_hir::types::InitializerId(0))
            .is_none());
        assert_eq!(catalog.type_default(TypeId::INT), None);

        let first = catalog.insert(Expr::Literal(Value::Int(7)));
        let second = catalog.insert(Expr::Literal(Value::Int(11)));

        assert_eq!(first, trust_hir::types::InitializerId(0));
        assert_eq!(second, trust_hir::types::InitializerId(1));
        assert!(matches!(
            catalog.initializer(first),
            Some(Expr::Literal(Value::Int(7)))
        ));
        assert!(matches!(
            catalog.initializer(second),
            Some(Expr::Literal(Value::Int(11)))
        ));

        catalog.set_type_default(TypeId::INT, first);
        assert_eq!(catalog.type_default(TypeId::INT), Some(first));
        catalog.set_type_default(TypeId::INT, second);
        assert_eq!(catalog.type_default(TypeId::INT), Some(second));
        assert_eq!(catalog.type_default(TypeId::BOOL), None);
        assert!(matches!(
            catalog.initializer(first),
            Some(Expr::Literal(Value::Int(7)))
        ));
    }
}
