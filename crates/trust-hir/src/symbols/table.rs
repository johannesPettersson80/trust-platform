use super::defs::*;
use super::helpers::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::TextRange;

use crate::types::{StructField, Type, TypeId, UnionVariant};

/// The symbol table containing all symbols and scopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    /// All symbols indexed by ID.
    symbols: FxHashMap<SymbolId, Symbol>,
    /// All scopes.
    scopes: Vec<Scope>,
    /// Current scope ID during collection.
    current_scope: ScopeId,
    /// Global name lookup.
    global_names: FxHashMap<SmolStr, SymbolId>,
    /// Type name lookup.
    type_names: FxHashMap<SmolStr, TypeId>,
    /// Type definitions by ID.
    types: FxHashMap<TypeId, Type>,
    /// Extends relationships (symbol -> base type name).
    extends: FxHashMap<SymbolId, SmolStr>,
    /// Implements relationships (symbol -> interface type names).
    implements: FxHashMap<SymbolId, Vec<SmolStr>>,
    /// Constant values by (scope, name).
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    /// Next symbol ID to assign.
    next_id: u32,
    /// Next type ID to assign.
    next_type_id: u32,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    /// Creates a new empty symbol table.
    #[must_use]
    pub fn new() -> Self {
        let mut table = Self {
            symbols: FxHashMap::default(),
            scopes: Vec::new(),
            current_scope: ScopeId::GLOBAL,
            global_names: FxHashMap::default(),
            type_names: FxHashMap::default(),
            types: FxHashMap::default(),
            extends: FxHashMap::default(),
            implements: FxHashMap::default(),
            const_values: FxHashMap::default(),
            next_id: 0,
            next_type_id: TypeId::USER_TYPES_START,
        };
        // Create global scope
        table
            .scopes
            .push(Scope::new(ScopeId::GLOBAL, ScopeKind::Global, None, None));
        table.register_builtin_types();
        table.register_builtin_function_blocks();
        table
    }

    /// Returns the current scope ID.
    #[must_use]
    pub fn current_scope(&self) -> ScopeId {
        self.current_scope
    }

    /// Sets the current scope ID.
    pub fn set_current_scope(&mut self, scope_id: ScopeId) {
        self.current_scope = scope_id;
    }

    /// Creates a new child scope and makes it current.
    pub fn push_scope(&mut self, kind: ScopeKind, owner: Option<SymbolId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        let parent = Some(self.current_scope);
        self.scopes.push(Scope::new(id, kind, parent, owner));
        self.current_scope = id;
        id
    }

    /// Pops the current scope and returns to the parent.
    pub fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.get(self.current_scope.0 as usize) {
            if let Some(parent) = scope.parent {
                self.current_scope = parent;
            }
        }
    }

    /// Gets a scope by ID.
    #[must_use]
    pub fn get_scope(&self, id: ScopeId) -> Option<&Scope> {
        self.scopes.get(id.0 as usize)
    }

    /// Returns all scopes in the symbol table.
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// Adds a USING directive to the current scope.
    pub fn add_using_directive(&mut self, path: Vec<SmolStr>, range: TextRange) {
        if let Some(scope) = self.scopes.get_mut(self.current_scope.0 as usize) {
            scope.using_directives.push(UsingDirective { path, range });
        }
    }

    /// Returns the total number of scopes.
    #[must_use]
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    /// Finds the scope owned by the given symbol.
    #[must_use]
    pub fn scope_for_owner(&self, owner: SymbolId) -> Option<ScopeId> {
        for (index, scope) in self.scopes.iter().enumerate() {
            if scope.owner == Some(owner) {
                return Some(ScopeId(index as u32));
            }
        }
        None
    }

    /// Resolves a name through the scope chain, starting from the given scope.
    #[must_use]
    pub fn resolve(&self, name: &str, from_scope: ScopeId) -> Option<SymbolId> {
        let mut scope_id = Some(from_scope);
        while let Some(sid) = scope_id {
            if let Some(scope) = self.scopes.get(sid.0 as usize) {
                if let Some(symbol_id) = scope.lookup_local(name) {
                    return Some(symbol_id);
                }
                match self.resolve_using_in_scope(scope, name) {
                    UsingResolution::Single(id) => return Some(id),
                    UsingResolution::Ambiguous => return None,
                    UsingResolution::None => {}
                }
                scope_id = scope.parent;
            } else {
                break;
            }
        }
        None
    }

    /// Resolves a name from the current scope.
    #[must_use]
    pub fn resolve_current(&self, name: &str) -> Option<SymbolId> {
        self.resolve(name, self.current_scope)
    }

    /// Resolves a name via USING directives in the given scope.
    pub fn resolve_using_in_scope(&self, scope: &Scope, name: &str) -> UsingResolution {
        if scope.using_directives.is_empty() {
            return UsingResolution::None;
        }

        let mut matches = FxHashSet::default();
        let mut first = None;

        for using in &scope.using_directives {
            let mut parts = using.path.clone();
            parts.push(SmolStr::new(name));
            let Some(symbol_id) = self.resolve_qualified(&parts) else {
                continue;
            };
            if let Some(symbol) = self.get(symbol_id) {
                if matches!(symbol.kind, SymbolKind::Namespace) {
                    continue;
                }
            }
            if matches.insert(symbol_id) {
                first.get_or_insert(symbol_id);
            }
        }

        match matches.len() {
            0 => UsingResolution::None,
            1 => UsingResolution::Single(first.unwrap_or(SymbolId::UNKNOWN)),
            _ => UsingResolution::Ambiguous,
        }
    }

    /// Adds a symbol to the table and the current scope.
    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;
        symbol.id = id;

        let name = symbol.name.clone();

        // Add to global lookup if it's in the global scope
        if self.current_scope == ScopeId::GLOBAL {
            self.global_names.insert(normalize_name(&name), id);
        }

        // Add to current scope
        if let Some(scope) = self.scopes.get_mut(self.current_scope.0 as usize) {
            scope.define(name, id);
        }

        self.symbols.insert(id, symbol);
        id
    }

    /// Adds a symbol to the table without adding to any scope (for internal use).
    pub fn add_symbol_raw(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;
        symbol.id = id;

        // Add to global lookup if it's a top-level symbol, but don't override
        // existing definitions (keeps local/primary symbols stable).
        if symbol.parent.is_none() {
            let key = normalize_name(&symbol.name);
            self.global_names.entry(key).or_insert(id);
        }

        self.symbols.insert(id, symbol);
        id
    }

    /// Defines an existing symbol ID in a scope.
    pub fn define_in_scope(
        &mut self,
        scope_id: ScopeId,
        name: SmolStr,
        id: SymbolId,
    ) -> Option<SymbolId> {
        let scope = self.scopes.get_mut(scope_id.0 as usize)?;
        scope.define(name, id)
    }

    /// Gets a symbol by ID.
    #[must_use]
    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(&id)
    }

    /// Gets a mutable reference to a symbol by ID.
    pub fn get_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(&id)
    }

    /// Looks up a symbol by name in the global scope.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.global_names.get(&normalize_name(name)).copied()
    }

    /// Resolves a qualified name via namespace symbols.
    #[must_use]
    pub fn resolve_qualified(&self, parts: &[SmolStr]) -> Option<SymbolId> {
        if parts.is_empty() {
            return None;
        }
        let mut current = self.lookup(parts[0].as_str())?;
        for part in parts.iter().skip(1) {
            let symbol = self.get(current)?;
            if !matches!(symbol.kind, SymbolKind::Namespace) {
                return None;
            }
            let mut next = None;
            for sym in self.symbols.values() {
                if sym.parent == Some(current) && sym.name.eq_ignore_ascii_case(part.as_str()) {
                    next = Some(sym.id);
                    break;
                }
            }
            current = next?;
        }
        Some(current)
    }

    /// Looks up a symbol by name in a specific scope.
    #[must_use]
    pub fn lookup_in_scope(&self, scope_id: ScopeId, name: &str) -> Option<SymbolId> {
        self.scopes
            .get(scope_id.0 as usize)
            .and_then(|scope| scope.lookup_local(name))
    }

    /// Looks up a symbol by name across all symbols.
    #[must_use]
    pub fn lookup_any(&self, name: &str) -> Option<SymbolId> {
        let normalized = normalize_name(name);
        if let Some(id) = self.global_names.get(&normalized) {
            return Some(*id);
        }
        self.symbols
            .values()
            .find(|sym| sym.name.as_str().eq_ignore_ascii_case(name))
            .map(|sym| sym.id)
    }

    /// Resolves a name, supporting namespace-qualified identifiers.
    #[must_use]
    pub fn resolve_by_name(&self, name: &str) -> Option<SymbolId> {
        if name.contains('.') {
            let parts = split_qualified_name(name);
            return self.resolve_qualified(&parts);
        }
        self.lookup(name)
    }

    pub(super) fn register_builtin(&mut self, id: TypeId, name: &str, ty: Type) {
        self.type_names.insert(normalize_name(name), id);
        self.types.insert(id, ty);
    }

    /// Registers a type by name and returns its ID.
    pub fn register_type(&mut self, name: impl Into<SmolStr>, ty: Type) -> TypeId {
        let name = name.into();
        let normalized = normalize_name(name.as_str());
        if let Some(existing) = self.type_names.get(&normalized).copied() {
            let should_replace = self.types.get(&existing).is_none_or(is_placeholder_alias);
            if should_replace {
                self.types.insert(existing, ty);
            }
            return existing;
        }
        let id = TypeId(self.next_type_id);
        self.next_type_id += 1;
        self.type_names.insert(normalized, id);
        self.types.insert(id, ty);
        id
    }

    /// Registers a struct type with fields.
    pub fn register_struct_type(
        &mut self,
        name: impl Into<SmolStr>,
        fields: Vec<StructField>,
    ) -> TypeId {
        let name = name.into();
        self.register_type(name.clone(), Type::Struct { name, fields })
    }

    /// Registers a union type with variants.
    pub fn register_union_type(
        &mut self,
        name: impl Into<SmolStr>,
        variants: Vec<UnionVariant>,
    ) -> TypeId {
        let name = name.into();
        self.register_type(name.clone(), Type::Union { name, variants })
    }

    /// Registers an enum type with values.
    pub fn register_enum_type(
        &mut self,
        name: impl Into<SmolStr>,
        base: TypeId,
        values: Vec<(SmolStr, i64)>,
    ) -> TypeId {
        let name = name.into();
        self.register_type(name.clone(), Type::Enum { name, base, values })
    }

    /// Registers an array type.
    pub fn register_array_type(&mut self, element: TypeId, dimensions: Vec<(i64, i64)>) -> TypeId {
        // Generate a unique name for the array type
        let elem_name = self.type_name(element).unwrap_or_else(|| SmolStr::new("?"));
        let dims_str: Vec<String> = dimensions
            .iter()
            .map(|(l, u)| format!("{}..{}", l, u))
            .collect();
        let name = format!("ARRAY[{}] OF {}", dims_str.join(", "), elem_name);
        self.register_type(
            name,
            Type::Array {
                element,
                dimensions,
            },
        )
    }

    /// Registers a pointer type.
    pub fn register_pointer_type(&mut self, target: TypeId) -> TypeId {
        let target_name = self.type_name(target).unwrap_or_else(|| SmolStr::new("?"));
        let name = format!("POINTER TO {}", target_name);
        self.register_type(name, Type::Pointer { target })
    }

    /// Registers a reference type.
    pub fn register_reference_type(&mut self, target: TypeId) -> TypeId {
        let target_name = self.type_name(target).unwrap_or_else(|| SmolStr::new("?"));
        let name = format!("REF_TO {}", target_name);
        self.register_type(name, Type::Reference { target })
    }

    /// Registers a subrange type.
    pub fn register_subrange_type(&mut self, base: TypeId, lower: i64, upper: i64) -> TypeId {
        let base_name = self.type_name(base).unwrap_or_else(|| SmolStr::new("?"));
        let name = format!("{}({}..{})", base_name, lower, upper);
        self.register_type(name, Type::Subrange { base, lower, upper })
    }

    /// Gets the name of a type by ID.
    #[must_use]
    pub fn type_name(&self, id: TypeId) -> Option<SmolStr> {
        // Check built-in types first
        if let Some(name) = id.builtin_name() {
            return Some(SmolStr::new(name));
        }
        // Look up in registered names
        self.type_names
            .iter()
            .find(|(_, &tid)| tid == id)
            .map(|(name, _)| name.clone())
    }

    /// Looks up a type ID by name.
    #[must_use]
    pub fn lookup_type(&self, name: &str) -> Option<TypeId> {
        self.type_names.get(&normalize_name(name)).copied()
    }

    /// Gets a type by ID.
    #[must_use]
    pub fn type_by_id(&self, id: TypeId) -> Option<&Type> {
        self.types.get(&id)
    }

    /// Resolves an enum value by name (case-insensitive) and returns its numeric value.
    #[must_use]
    pub fn enum_value_by_name(&self, name: &str) -> Option<i64> {
        for ty in self.types.values() {
            let Type::Enum { values, .. } = ty else {
                continue;
            };
            if let Some((_, value)) = values
                .iter()
                .find(|(value_name, _)| value_name.eq_ignore_ascii_case(name))
            {
                return Some(*value);
            }
        }
        None
    }

    /// Sets the table's constant values.
    pub fn set_const_values(&mut self, values: FxHashMap<(Option<SmolStr>, SmolStr), i64>) {
        self.const_values = values;
    }

    /// Returns a constant value for a given scope/name.
    #[must_use]
    pub fn const_value(&self, scope: &Option<SmolStr>, name: &str) -> Option<i64> {
        let key = const_key(scope, name);
        self.const_values.get(&key).copied()
    }

    /// Returns an iterator over all symbols.
    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Returns the number of symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Returns true if the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Records an extends relationship for a symbol.
    pub fn set_extends(&mut self, owner: SymbolId, base: SmolStr) {
        self.extends.insert(owner, base);
    }

    /// Records implemented interfaces for a symbol.
    pub fn set_implements(&mut self, owner: SymbolId, interfaces: Vec<SmolStr>) {
        self.implements.insert(owner, interfaces);
    }

    /// Returns the base type name for a symbol, if any.
    #[must_use]
    pub fn extends_name(&self, owner: SymbolId) -> Option<&SmolStr> {
        self.extends.get(&owner)
    }

    /// Returns implemented interface names for a symbol, if any.
    #[must_use]
    pub fn implements_names(&self, owner: SymbolId) -> Option<&[SmolStr]> {
        self.implements.get(&owner).map(|names| names.as_slice())
    }

    /// Resolves alias types to their underlying target.
    #[must_use]
    pub fn resolve_alias_type(&self, type_id: TypeId) -> TypeId {
        let mut current = type_id;
        let mut guard = 0;
        while guard < 16 {
            let Some(Type::Alias { target, .. }) = self.types.get(&current) else {
                break;
            };
            if *target == current {
                break;
            }
            current = *target;
            guard += 1;
        }
        current
    }

    /// Resolves a member symbol in an inheritance chain.
    #[must_use]
    pub fn resolve_member_symbol_in_hierarchy(
        &self,
        root_id: SymbolId,
        member_name: &str,
    ) -> Option<SymbolId> {
        let mut visited = FxHashSet::default();
        let mut current = Some(root_id);

        while let Some(symbol_id) = current {
            if !visited.insert(symbol_id) {
                break;
            }

            for sym in self.symbols.values() {
                if sym.parent == Some(symbol_id) && sym.name.eq_ignore_ascii_case(member_name) {
                    return Some(sym.id);
                }
            }

            let base_name = self.extends_name(symbol_id)?;
            let base_id = self.resolve_by_name(base_name.as_str())?;
            current = Some(base_id);
        }

        None
    }

    /// Resolves a member symbol for function blocks or interfaces by type.
    #[must_use]
    pub fn resolve_member_symbol_in_type(
        &self,
        type_id: TypeId,
        member_name: &str,
    ) -> Option<SymbolId> {
        let base = self.resolve_alias_type(type_id);
        match self.types.get(&base)? {
            Type::FunctionBlock { name } | Type::Class { name } | Type::Interface { name } => {
                let owner = self.resolve_by_name(name.as_str())?;
                self.resolve_member_symbol_in_hierarchy(owner, member_name)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::TypeId;
    use text_size::TextRange;

    #[test]
    fn test_symbol_table() {
        let mut table = SymbolTable::new();

        let sym = Symbol::new(
            SymbolId::UNKNOWN,
            "TestProgram",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        );

        let id = table.add_symbol(sym);

        assert!(table.get(id).is_some());
        assert_eq!(table.lookup("TestProgram"), Some(id));
    }
}
