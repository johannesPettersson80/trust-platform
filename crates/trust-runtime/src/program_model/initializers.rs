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
