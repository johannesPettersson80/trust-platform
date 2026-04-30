use super::defs::*;
use super::helpers::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::TextRange;

use crate::db::FileId;
use crate::semantic::{
    DeclarationCatalog, DeclarationKind, DeclarationRecord, DeclarationReferenceKind,
    DeclarationReferenceRecord, QualifiedName, SemanticOutcome, SemanticRole, SourceIdentity,
};
use crate::types::{
    ArrayDimensionExt, InitializerCatalog, InitializerId, InitializerRecord, StructField, Type,
    TypeId, UnionVariant,
};

/// Result of resolving an unqualified enum value name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumValueResolution {
    /// No enum value matched the requested name.
    NotFound,
    /// Exactly one enum value matched the requested name.
    Resolved(i64),
    /// More than one enum value matched; callers must diagnose ambiguity.
    Ambiguous,
}

/// Result of resolving a name that must be unique across the analyzed symbol table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UniqueSymbolResolution {
    /// No symbol matched.
    NotFound,
    /// Exactly one symbol matched.
    Single(SymbolId),
    /// Multiple symbols matched.
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OopReference {
    name: SmolStr,
    range: Option<TextRange>,
}

impl OopReference {
    fn new(name: SmolStr, range: Option<TextRange>) -> Self {
        Self { name, range }
    }
}

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
    /// Local source symbol lookup by declaration range/name.
    symbols_by_name_range: FxHashMap<(u32, u32, SmolStr), SymbolId>,
    /// Scope lookup by owning symbol.
    scope_by_owner: FxHashMap<SymbolId, ScopeId>,
    /// Type name lookup.
    type_names: FxHashMap<SmolStr, TypeId>,
    /// Type definitions by ID.
    types: FxHashMap<TypeId, Type>,
    /// Extends relationships retained as semantic reference records.
    extends: FxHashMap<SymbolId, OopReference>,
    /// Implements relationships retained as semantic reference records.
    implements: FxHashMap<SymbolId, Vec<OopReference>>,
    /// Constant values by (scope, name).
    const_values: FxHashMap<(Option<SmolStr>, SmolStr), i64>,
    /// Source-backed initializer records for declaration defaults.
    initializer_catalog: InitializerCatalog,
    /// Project import declarations skipped because another symbol already owns the name.
    import_collisions: Vec<ImportCollision>,
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
            symbols_by_name_range: FxHashMap::default(),
            scope_by_owner: FxHashMap::default(),
            type_names: FxHashMap::default(),
            types: FxHashMap::default(),
            extends: FxHashMap::default(),
            implements: FxHashMap::default(),
            const_values: FxHashMap::default(),
            initializer_catalog: InitializerCatalog::default(),
            import_collisions: Vec::new(),
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
        if let Some(owner) = owner {
            self.scope_by_owner.insert(owner, id);
        }
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
        self.scope_by_owner.get(&owner).copied()
    }

    pub(crate) fn ensure_scope_for_owner(
        &mut self,
        owner: SymbolId,
        kind: ScopeKind,
    ) -> Option<ScopeId> {
        if let Some(scope_id) = self.scope_for_owner(owner) {
            return Some(scope_id);
        }
        let parent_scope = self
            .get(owner)
            .and_then(|symbol| symbol.parent)
            .and_then(|parent| self.scope_for_owner(parent))
            .unwrap_or(ScopeId::GLOBAL);
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes
            .push(Scope::new(id, kind, Some(parent_scope), Some(owner)));
        self.scope_by_owner.insert(owner, id);
        Some(id)
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

        match (matches.len(), first) {
            (0, _) => UsingResolution::None,
            (1, Some(id)) => UsingResolution::Single(id),
            (1, None) => UsingResolution::None,
            _ => UsingResolution::Ambiguous,
        }
    }

    /// Adds a symbol to the table and the current scope.
    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;
        symbol.id = id;

        let name = symbol.name.clone();
        self.index_source_symbol_by_name_range(&symbol);

        // Add to global lookup if it's in the global scope
        if self.current_scope == ScopeId::GLOBAL {
            self.global_names.entry(normalize_name(&name)).or_insert(id);
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
        self.index_source_symbol_by_name_range(&symbol);

        // Add to global lookup if it's a top-level symbol, but don't override
        // existing definitions (keeps local/primary symbols stable).
        if symbol.parent.is_none() {
            let key = normalize_name(&symbol.name);
            self.global_names.entry(key).or_insert(id);
        }

        self.symbols.insert(id, symbol);
        id
    }

    fn index_source_symbol_by_name_range(&mut self, symbol: &Symbol) {
        if symbol.origin.is_some() {
            return;
        }
        let key = symbol_name_range_key(symbol.name.as_str(), symbol.range);
        self.symbols_by_name_range.insert(key, symbol.id);
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

    /// Records a project-import name collision that must become a diagnostic.
    pub fn record_import_collision(
        &mut self,
        name: SmolStr,
        existing_range: TextRange,
        duplicate_range: TextRange,
    ) {
        self.import_collisions.push(ImportCollision {
            name,
            existing_range,
            duplicate_range,
        });
    }

    /// Returns project-import name collisions collected while merging symbol tables.
    #[must_use]
    pub fn import_collisions(&self) -> &[ImportCollision] {
        &self.import_collisions
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
    #[cfg(test)]
    pub(crate) fn lookup_any(&self, name: &str) -> Option<SymbolId> {
        let normalized = normalize_name(name);
        if let Some(id) = self.global_names.get(&normalized) {
            return Some(*id);
        }
        self.symbols
            .values()
            .find(|sym| sym.name.as_str().eq_ignore_ascii_case(name))
            .map(|sym| sym.id)
    }

    /// Resolves a name only when it has one table-wide match.
    #[must_use]
    pub(crate) fn resolve_unique_symbol_name(&self, name: &str) -> UniqueSymbolResolution {
        let mut matched = None;
        for symbol in self
            .symbols
            .values()
            .filter(|symbol| symbol.name.as_str().eq_ignore_ascii_case(name))
        {
            if matched.is_some_and(|existing| existing != symbol.id) {
                return UniqueSymbolResolution::Ambiguous;
            }
            matched = Some(symbol.id);
        }
        matched.map_or(
            UniqueSymbolResolution::NotFound,
            UniqueSymbolResolution::Single,
        )
    }

    /// Looks up a local source symbol by declaration name and range.
    #[must_use]
    pub fn lookup_by_name_range(&self, name: &str, range: TextRange) -> Option<SymbolId> {
        self.symbols_by_name_range
            .get(&symbol_name_range_key(name, range))
            .copied()
    }

    /// Resolves a name, supporting namespace-qualified identifiers.
    #[must_use]
    pub fn resolve_global_or_qualified_name(&self, name: &str) -> Option<SymbolId> {
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

    /// Registers a source-backed initializer record and returns a table-local ID.
    pub fn register_initializer(&mut self, record: InitializerRecord) -> InitializerId {
        self.initializer_catalog.insert(record)
    }

    /// Copies an initializer record from another symbol table and returns a fresh local ID.
    pub fn import_initializer_from(
        &mut self,
        source: &SymbolTable,
        id: InitializerId,
    ) -> Option<InitializerId> {
        source
            .initializer(id)
            .cloned()
            .map(|record| self.register_initializer(record))
    }

    /// Associates a TYPE-level default initializer with a type.
    pub fn set_type_default_initializer(&mut self, type_id: TypeId, id: InitializerId) {
        self.initializer_catalog.set_type_default(type_id, id);
    }

    /// Returns a TYPE-level default initializer for a type.
    #[must_use]
    pub fn type_default_initializer(&self, type_id: TypeId) -> Option<InitializerId> {
        self.initializer_catalog.type_default(type_id)
    }

    /// Returns the HIR initializer catalog.
    #[must_use]
    pub fn initializer_catalog(&self) -> &InitializerCatalog {
        &self.initializer_catalog
    }

    /// Returns a HIR initializer record.
    #[must_use]
    pub fn initializer(&self, id: InitializerId) -> Option<&InitializerRecord> {
        self.initializer_catalog.get(id)
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
            .map(ArrayDimensionExt::display_bounds)
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
    pub fn lookup_registered_type_name(&self, name: &str) -> Option<TypeId> {
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
        match self.resolve_enum_value_by_name(name) {
            EnumValueResolution::Resolved(value) => Some(value),
            EnumValueResolution::NotFound | EnumValueResolution::Ambiguous => None,
        }
    }

    /// Resolves an enum value by name and reports ambiguous unqualified matches.
    #[must_use]
    pub fn resolve_enum_value_by_name(&self, name: &str) -> EnumValueResolution {
        let mut matched = None;
        for ty in self.types.values() {
            let Type::Enum { values, .. } = ty else {
                continue;
            };
            if let Some((_, value)) = values
                .iter()
                .find(|(value_name, _)| value_name.eq_ignore_ascii_case(name))
            {
                if matched.is_some() {
                    return EnumValueResolution::Ambiguous;
                }
                matched = Some(*value);
            }
        }
        matched.map_or(EnumValueResolution::NotFound, EnumValueResolution::Resolved)
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
    pub(crate) fn set_extends(&mut self, owner: SymbolId, base: SmolStr) {
        self.extends.insert(owner, OopReference::new(base, None));
    }

    /// Records implemented interfaces for a symbol.
    pub(crate) fn set_implements(&mut self, owner: SymbolId, interfaces: Vec<SmolStr>) {
        self.implements.insert(
            owner,
            interfaces
                .into_iter()
                .map(|name| OopReference::new(name, None))
                .collect(),
        );
    }

    /// Returns the base type name for a symbol, if any.
    #[must_use]
    pub(crate) fn extends_name(&self, owner: SymbolId) -> Option<&SmolStr> {
        self.extends.get(&owner).map(|reference| &reference.name)
    }

    /// Returns implemented interface names for a symbol, if any.
    #[must_use]
    pub(crate) fn implements_names(&self, owner: SymbolId) -> Option<Vec<&SmolStr>> {
        self.implements.get(&owner).map(|references| {
            references
                .iter()
                .map(|reference| &reference.name)
                .collect::<Vec<_>>()
        })
    }

    /// Returns the retained EXTENDS reference record for a symbol.
    #[must_use]
    pub fn extends_reference(&self, owner: SymbolId) -> Option<DeclarationReferenceRecord> {
        let reference = self.extends.get(&owner)?;
        Some(self.oop_reference_record(owner, DeclarationReferenceKind::Extends, reference))
    }

    /// Returns retained IMPLEMENTS reference records for a symbol.
    #[must_use]
    pub fn implements_references(&self, owner: SymbolId) -> Vec<DeclarationReferenceRecord> {
        self.implements
            .get(&owner)
            .map(|references| {
                references
                    .iter()
                    .map(|reference| {
                        self.oop_reference_record(
                            owner,
                            DeclarationReferenceKind::Implements,
                            reference,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Builds a HIR-owned declaration catalog for this analyzed symbol table.
    #[must_use]
    pub fn declaration_catalog(&self, file_id: FileId) -> DeclarationCatalog {
        let mut entries = Vec::new();
        let mut source_symbols: Vec<&Symbol> = self
            .symbols
            .values()
            .filter(|symbol| !symbol.range.is_empty())
            .collect();
        source_symbols.sort_by_key(|symbol| symbol.id.0);

        for symbol in source_symbols {
            let Some(qualified_name) = self.qualified_name_for_symbol(symbol.id) else {
                continue;
            };
            let source_file = symbol
                .origin
                .map(|origin| origin.file_id)
                .unwrap_or(file_id);
            entries.push(DeclarationRecord::new(
                symbol.id,
                qualified_name,
                SourceIdentity::new(source_file, symbol.range),
                symbol.type_id,
                declaration_kind_for_symbol(&symbol.kind),
                semantic_role_for_symbol(&symbol.kind),
                symbol.parent,
                self.containing_scope_for_symbol(symbol),
                self.scope_for_owner(symbol.id),
                symbol.origin.is_some(),
            ));
        }

        let mut references = Vec::new();
        let mut reference_owners: Vec<SymbolId> = self
            .extends
            .keys()
            .chain(self.implements.keys())
            .copied()
            .collect();
        reference_owners.sort_by_key(|owner| owner.0);
        reference_owners.dedup();
        for owner in reference_owners {
            if self.get(owner).is_none_or(|symbol| symbol.range.is_empty()) {
                continue;
            }
            if let Some(reference) = self.extends_reference(owner) {
                references.push(reference);
            }
            references.extend(self.implements_references(owner));
        }

        DeclarationCatalog::new(entries, references)
    }

    /// Resolves a raw OOP reference name relative to the owning symbol's scope.
    #[must_use]
    pub fn resolve_oop_reference_for_owner(&self, owner: SymbolId, name: &str) -> Option<SymbolId> {
        let parts = split_qualified_name(name);
        let symbol_id = if parts.len() > 1 {
            self.resolve_qualified(&parts)
        } else {
            let scope_id = self.scope_for_owner(owner)?;
            self.resolve(name, scope_id)
        }?;
        self.get(symbol_id)
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface
                )
            })
            .map(|_| symbol_id)
    }

    /// Resolves alias types to their underlying target.
    #[must_use]
    pub fn resolve_alias_type(&self, type_id: TypeId) -> TypeId {
        match self.resolve_alias_type_outcome(type_id) {
            SemanticOutcome::Resolved(type_id) => type_id,
            SemanticOutcome::InvariantViolation { .. } => type_id,
            _ => type_id,
        }
    }

    pub(crate) fn resolve_alias_type_outcome(&self, type_id: TypeId) -> SemanticOutcome<TypeId> {
        let mut current = type_id;
        let mut visited = FxHashSet::default();
        while visited.insert(current) {
            let Some(Type::Alias { target, .. }) = self.types.get(&current) else {
                return SemanticOutcome::Resolved(current);
            };
            if *target == current {
                return SemanticOutcome::InvariantViolation {
                    message: SmolStr::new(format!(
                        "cyclic alias type involving TypeId({})",
                        current.0
                    )),
                    range: None,
                };
            }
            current = *target;
        }
        SemanticOutcome::InvariantViolation {
            message: SmolStr::new(format!("cyclic alias type involving TypeId({})", current.0)),
            range: None,
        }
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
            let base_id = self.resolve_oop_reference_for_owner(symbol_id, base_name.as_str())?;
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
                let owner = self.resolve_global_or_qualified_name(name.as_str())?;
                self.resolve_member_symbol_in_hierarchy(owner, member_name)
            }
            _ => None,
        }
    }

    fn resolve_oop_reference_outcome_for_owner(
        &self,
        owner: SymbolId,
        name: &str,
        range: Option<TextRange>,
    ) -> SemanticOutcome<SymbolId> {
        let qualified_name = qualified_name_from_raw(name);
        let parts = split_qualified_name(name);
        let symbol_id = if parts.len() > 1 {
            self.resolve_qualified(&parts)
        } else {
            let Some(scope_id) = self.scope_for_owner(owner) else {
                return SemanticOutcome::InvariantViolation {
                    message: SmolStr::new(format!(
                        "missing owner scope while resolving declaration reference '{}'",
                        name
                    )),
                    range,
                };
            };
            self.resolve(name, scope_id)
        };

        let Some(symbol_id) = symbol_id else {
            return SemanticOutcome::Unknown {
                name: Some(qualified_name),
                range,
            };
        };

        let Some(symbol) = self.get(symbol_id) else {
            return SemanticOutcome::InvariantViolation {
                message: SmolStr::new(format!(
                    "resolved declaration reference '{}' to missing SymbolId({})",
                    name, symbol_id.0
                )),
                range,
            };
        };

        if matches!(
            symbol.kind,
            SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface
        ) {
            SemanticOutcome::Resolved(symbol_id)
        } else {
            SemanticOutcome::WrongKind {
                symbol_id,
                expected: SemanticRole::Type,
                actual: semantic_role_for_symbol(&symbol.kind),
                range,
            }
        }
    }

    fn oop_reference_record(
        &self,
        owner: SymbolId,
        kind: DeclarationReferenceKind,
        reference: &OopReference,
    ) -> DeclarationReferenceRecord {
        DeclarationReferenceRecord::new(
            owner,
            kind,
            qualified_name_from_raw(reference.name.as_str()),
            reference.range,
            self.resolve_oop_reference_outcome_for_owner(
                owner,
                reference.name.as_str(),
                reference.range,
            ),
        )
    }

    fn qualified_name_for_symbol(&self, symbol_id: SymbolId) -> Option<QualifiedName> {
        let mut parts = Vec::new();
        let mut visited = FxHashSet::default();
        let mut current = Some(symbol_id);
        while let Some(id) = current {
            if !visited.insert(id) {
                return None;
            }
            let symbol = self.get(id)?;
            parts.push(symbol.name.clone());
            current = symbol.parent;
        }
        parts.reverse();
        QualifiedName::new(parts)
    }

    fn containing_scope_for_symbol(&self, symbol: &Symbol) -> Option<ScopeId> {
        let mut parent = symbol.parent;
        while let Some(parent_id) = parent {
            if let Some(scope_id) = self.scope_for_owner(parent_id) {
                return Some(scope_id);
            }
            parent = self
                .get(parent_id)
                .and_then(|parent_symbol| parent_symbol.parent);
        }
        Some(ScopeId::GLOBAL)
    }
}

fn symbol_name_range_key(name: &str, range: TextRange) -> (u32, u32, SmolStr) {
    (
        u32::from(range.start()),
        u32::from(range.end()),
        normalize_name(name),
    )
}

fn qualified_name_from_raw(name: &str) -> QualifiedName {
    QualifiedName::from_dotted(name).unwrap_or_else(|| {
        QualifiedName::new(vec![SmolStr::new(name)]).expect("fallback name is non-empty")
    })
}

fn declaration_kind_for_symbol(kind: &SymbolKind) -> DeclarationKind {
    match kind {
        SymbolKind::Program => DeclarationKind::Program,
        SymbolKind::Configuration => DeclarationKind::Configuration,
        SymbolKind::Resource => DeclarationKind::Resource,
        SymbolKind::Task => DeclarationKind::Task,
        SymbolKind::ProgramInstance => DeclarationKind::ProgramInstance,
        SymbolKind::Namespace => DeclarationKind::Namespace,
        SymbolKind::Function { .. } => DeclarationKind::Function,
        SymbolKind::FunctionBlock => DeclarationKind::FunctionBlock,
        SymbolKind::Class => DeclarationKind::Class,
        SymbolKind::Method { .. } => DeclarationKind::Method,
        SymbolKind::Property { .. } => DeclarationKind::Property,
        SymbolKind::Interface => DeclarationKind::Interface,
        SymbolKind::Variable { .. } => DeclarationKind::Variable,
        SymbolKind::Constant => DeclarationKind::Constant,
        SymbolKind::Type => DeclarationKind::Type,
        SymbolKind::EnumValue { .. } => DeclarationKind::EnumValue,
        SymbolKind::Parameter { .. } => DeclarationKind::Parameter,
    }
}

fn semantic_role_for_symbol(kind: &SymbolKind) -> SemanticRole {
    match kind {
        SymbolKind::Variable { .. }
        | SymbolKind::Constant
        | SymbolKind::EnumValue { .. }
        | SymbolKind::Parameter { .. }
        | SymbolKind::ProgramInstance
        | SymbolKind::Property { .. } => SemanticRole::Value,
        SymbolKind::Type
        | SymbolKind::FunctionBlock
        | SymbolKind::Class
        | SymbolKind::Interface => SemanticRole::Type,
        SymbolKind::Function { .. } | SymbolKind::Method { .. } => SemanticRole::Callable,
        SymbolKind::Namespace => SemanticRole::Namespace,
        SymbolKind::Program
        | SymbolKind::Configuration
        | SymbolKind::Resource
        | SymbolKind::Task => SemanticRole::ScopeOwner,
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::semantic::SemanticOutcome;
    use crate::types::Type;
    use crate::TypeId;
    use smol_str::SmolStr;
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
        assert_eq!(
            table.lookup_by_name_range("testprogram", TextRange::empty(0.into())),
            Some(id)
        );
    }

    #[test]
    fn top_level_symbol_lookup_policy_is_first_writer_for_all_insert_apis() {
        let mut scoped = SymbolTable::new();
        let first = scoped.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Duplicate",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        ));
        let second = scoped.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Duplicate",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        ));
        assert_ne!(first, second);
        assert_eq!(scoped.lookup("Duplicate"), Some(first));

        let mut raw = SymbolTable::new();
        let first = raw.add_symbol_raw(Symbol::new(
            SymbolId::UNKNOWN,
            "Duplicate",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        ));
        let second = raw.add_symbol_raw(Symbol::new(
            SymbolId::UNKNOWN,
            "Duplicate",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        ));
        assert_ne!(first, second);
        assert_eq!(raw.lookup("Duplicate"), Some(first));
    }

    #[test]
    fn alias_resolution_outcome_reports_cycle_explicitly() {
        let mut table = SymbolTable::new();
        let alias_a = TypeId(TypeId::USER_TYPES_START);
        let alias_b = TypeId(TypeId::USER_TYPES_START + 1);
        table.types.insert(
            alias_a,
            Type::Alias {
                name: SmolStr::new("AliasA"),
                target: alias_b,
            },
        );
        table.types.insert(
            alias_b,
            Type::Alias {
                name: SmolStr::new("AliasB"),
                target: alias_a,
            },
        );

        assert!(matches!(
            table.resolve_alias_type_outcome(alias_a),
            SemanticOutcome::InvariantViolation { .. }
        ));
        assert_eq!(table.resolve_alias_type(alias_a), alias_a);
    }
}
