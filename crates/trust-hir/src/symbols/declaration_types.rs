// Read-only tooling projection; preserve the existing HIR type identity domain.
impl SymbolTable {
    /// Export declaration types for read-only tooling without constructing executable state.
    pub fn declaration_types(&self) -> crate::types::TypeRegistry {
        crate::types::TypeRegistry::declaration_snapshot(
            self.types.clone(), self.type_names.clone(), self.named_value_types.clone(),
            self.overlapping_struct_types.clone(),
        )
    }

}
