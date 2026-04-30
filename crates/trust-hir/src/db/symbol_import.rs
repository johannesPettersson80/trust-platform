use super::*;
use crate::diagnostics::DiagnosticCode;
use crate::semantic::{SemanticOutcome, LEGACY_UNKNOWN_TYPE_ID};
use crate::types::{StructField, UnionVariant};

fn should_report_import_name_collision(kind: &SymbolKind) -> bool {
    !matches!(
        kind,
        SymbolKind::Namespace
            | SymbolKind::Type
            | SymbolKind::FunctionBlock
            | SymbolKind::Class
            | SymbolKind::Interface
    )
}

fn imported_owner_scope_kind(kind: &SymbolKind) -> Option<ScopeKind> {
    match kind {
        SymbolKind::Configuration => Some(ScopeKind::Configuration),
        SymbolKind::Resource => Some(ScopeKind::Resource),
        SymbolKind::Namespace | SymbolKind::Interface => Some(ScopeKind::Namespace),
        SymbolKind::Program => Some(ScopeKind::Program),
        SymbolKind::Function { .. } => Some(ScopeKind::Function),
        SymbolKind::FunctionBlock => Some(ScopeKind::FunctionBlock),
        SymbolKind::Class => Some(ScopeKind::Class),
        SymbolKind::Method { .. } => Some(ScopeKind::Method),
        SymbolKind::Property { .. } => Some(ScopeKind::Property),
        _ => None,
    }
}

pub(super) struct SymbolImporter<'a> {
    target: &'a mut SymbolTable,
    sources: &'a FxHashMap<FileId, Arc<SymbolTable>>,
    type_map: FxHashMap<(FileId, TypeId), TypeId>,
    importing: FxHashSet<(FileId, TypeId)>,
}

impl<'a> SymbolImporter<'a> {
    pub(super) fn new(
        target: &'a mut SymbolTable,
        sources: &'a FxHashMap<FileId, Arc<SymbolTable>>,
    ) -> Self {
        Self {
            target,
            sources,
            type_map: FxHashMap::default(),
            importing: FxHashSet::default(),
        }
    }

    pub(super) fn import_table(&mut self, source_file: FileId, source: &SymbolTable) {
        let mut namespace_targets: FxHashMap<Vec<SmolStr>, SymbolId> = FxHashMap::default();
        for symbol in self.target.iter() {
            if !matches!(symbol.kind, SymbolKind::Namespace) {
                continue;
            }
            if let Some(path) = Self::namespace_path(self.target, symbol.id) {
                namespace_targets.insert(path, symbol.id);
            }
        }

        let mut parent_map: FxHashMap<SymbolId, Option<SymbolId>> = FxHashMap::default();
        let mut source_symbols: Vec<Symbol> = source.iter().cloned().collect();
        source_symbols.sort_by_key(|sym| sym.id.0);
        for symbol in &source_symbols {
            parent_map.insert(symbol.id, symbol.parent);
        }

        let mut root_cache: FxHashMap<SymbolId, SymbolId> = FxHashMap::default();
        let mut root_for = |id: SymbolId| -> SymbolId {
            if let Some(root) = root_cache.get(&id) {
                return *root;
            }
            let mut current = id;
            while let Some(parent) = parent_map.get(&current).copied().flatten() {
                current = parent;
            }
            root_cache.insert(id, current);
            current
        };

        let mut importable_roots: FxHashSet<SymbolId> = FxHashSet::default();
        for symbol in source_symbols.iter().filter(|sym| sym.parent.is_none()) {
            if symbol.range.is_empty() {
                continue;
            }
            if matches!(symbol.kind, SymbolKind::Namespace) {
                importable_roots.insert(symbol.id);
                continue;
            }
            if let Some(existing_id) = self.target.lookup(symbol.name.as_str()) {
                if should_report_import_name_collision(&symbol.kind) {
                    let existing_range = self
                        .target
                        .get(existing_id)
                        .map(|existing| existing.range)
                        .unwrap_or_else(|| TextRange::empty(0.into()));
                    self.target.record_import_collision(
                        symbol.name.clone(),
                        existing_range,
                        symbol.range,
                    );
                }
                continue;
            }
            importable_roots.insert(symbol.id);
        }

        let mut id_map: FxHashMap<SymbolId, SymbolId> = FxHashMap::default();
        for symbol in source_symbols {
            let root_id = root_for(symbol.id);
            if !importable_roots.contains(&root_id) {
                continue;
            }

            if matches!(symbol.kind, SymbolKind::Namespace) {
                if let Some(path) = Self::namespace_path(source, symbol.id) {
                    if let Some(existing_id) = namespace_targets.get(&path).copied() {
                        id_map.insert(symbol.id, existing_id);
                        continue;
                    }
                }
            }

            let mut imported = symbol.clone();
            imported.kind = self.import_symbol_kind(source_file, &symbol.kind);
            imported.type_id = self.import_type(source_file, symbol.type_id);
            imported.origin = Some(SymbolOrigin {
                file_id: source_file,
                symbol_id: symbol.id,
            });
            imported.parent = if symbol.parent.is_none() {
                None
            } else {
                Some(SymbolId::UNKNOWN)
            };

            let new_id = self.target.add_symbol_raw(imported);
            id_map.insert(symbol.id, new_id);

            if matches!(symbol.kind, SymbolKind::Namespace) {
                if let Some(path) = Self::namespace_path(source, symbol.id) {
                    namespace_targets.insert(path, new_id);
                }
            }
        }

        for (old_id, new_id) in id_map.iter() {
            let old_parent = parent_map.get(old_id).copied().flatten();
            if let Some(new_parent) = old_parent.and_then(|pid| id_map.get(&pid).copied()) {
                if let Some(symbol) = self.target.get_mut(*new_id) {
                    symbol.parent = Some(new_parent);
                }
            }

            if let Some(symbol) = self.target.get_mut(*new_id) {
                match &mut symbol.kind {
                    SymbolKind::Function { parameters, .. }
                    | SymbolKind::Method { parameters, .. } => {
                        let mut remapped = Vec::with_capacity(parameters.len());
                        for param_id in parameters.iter() {
                            if let Some(new_param) = id_map.get(param_id).copied() {
                                remapped.push(new_param);
                            }
                        }
                        *parameters = remapped;
                    }
                    _ => {}
                }
            }
        }

        let mut imported_owner_ids: Vec<SymbolId> = id_map.values().copied().collect();
        imported_owner_ids.sort_by_key(|id| id.0);
        for new_id in imported_owner_ids {
            let Some(scope_kind) = self
                .target
                .get(new_id)
                .and_then(|symbol| imported_owner_scope_kind(&symbol.kind))
            else {
                continue;
            };
            self.target.ensure_scope_for_owner(new_id, scope_kind);
        }

        for (old_id, new_id) in id_map.iter() {
            if parent_map.get(old_id).copied().flatten().is_none() {
                if let Some(symbol) = self.target.get(*new_id) {
                    self.define_imported_symbol_in_scope(
                        ScopeId::GLOBAL,
                        symbol.name.clone(),
                        *new_id,
                    );
                }
            } else {
                let parent_id = self.target.get(*new_id).and_then(|symbol| symbol.parent);
                if let Some(parent_id) = parent_id {
                    let parent_kind = self.target.get(parent_id).map(|parent| parent.kind.clone());
                    match parent_kind {
                        Some(SymbolKind::Namespace) => {
                            let name = self.target.get(*new_id).map(|symbol| symbol.name.clone());
                            self.ensure_namespace_scope(parent_id);
                            if let (Some(scope_id), Some(name)) =
                                (self.target.scope_for_owner(parent_id), name)
                            {
                                self.define_imported_symbol_in_scope(scope_id, name, *new_id);
                            }
                        }
                        Some(SymbolKind::Configuration | SymbolKind::Resource) => {
                            let define_globally = self
                                .target
                                .get(*new_id)
                                .map(|symbol| {
                                    matches!(
                                        symbol.kind,
                                        SymbolKind::Variable {
                                            qualifier: VarQualifier::Global,
                                        } | SymbolKind::Variable {
                                            qualifier: VarQualifier::Access,
                                        } | SymbolKind::Constant
                                    )
                                })
                                .unwrap_or(false);
                            if define_globally {
                                let name =
                                    self.target.get(*new_id).map(|symbol| symbol.name.clone());
                                if let Some(name) = name {
                                    self.define_imported_symbol_in_scope(
                                        ScopeId::GLOBAL,
                                        name,
                                        *new_id,
                                    );
                                }
                            }
                        }
                        Some(kind) if imported_owner_scope_kind(&kind).is_some() => {
                            let name = self.target.get(*new_id).map(|symbol| symbol.name.clone());
                            if let (Some(scope_id), Some(name)) =
                                (self.target.scope_for_owner(parent_id), name)
                            {
                                self.define_imported_symbol_in_scope(scope_id, name, *new_id);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(base) = source.extends_name(*old_id) {
                self.target.set_extends(*new_id, base.clone());
            }
            if let Some(interfaces) = source.implements_names(*old_id) {
                self.target
                    .set_implements(*new_id, interfaces.into_iter().cloned().collect());
            }
        }

        for new_id in id_map.values() {
            if let Some(symbol) = self.target.get(*new_id) {
                if matches!(symbol.kind, SymbolKind::Namespace) {
                    self.ensure_namespace_scope(*new_id);
                }
            }
        }
    }

    fn define_imported_symbol_in_scope(
        &mut self,
        scope_id: ScopeId,
        name: SmolStr,
        new_id: SymbolId,
    ) {
        if let Some(existing_id) = self.target.lookup_in_scope(scope_id, name.as_str()) {
            self.record_import_scope_collision(&name, existing_id, new_id);
            return;
        }

        if let Some(existing_id) = self.target.define_in_scope(scope_id, name.clone(), new_id) {
            self.record_import_scope_collision(&name, existing_id, new_id);
        }
    }

    fn record_import_scope_collision(
        &mut self,
        name: &SmolStr,
        existing_id: SymbolId,
        duplicate_id: SymbolId,
    ) {
        if existing_id == duplicate_id {
            return;
        }
        let Some(duplicate) = self.target.get(duplicate_id) else {
            return;
        };
        if !should_report_import_name_collision(&duplicate.kind) {
            return;
        }
        let existing_range = self
            .target
            .get(existing_id)
            .map(|existing| existing.range)
            .unwrap_or_else(|| TextRange::empty(0.into()));
        self.target
            .record_import_collision(name.clone(), existing_range, duplicate.range);
    }

    fn namespace_path(table: &SymbolTable, symbol_id: SymbolId) -> Option<Vec<SmolStr>> {
        let mut parts = Vec::new();
        let mut current = symbol_id;
        loop {
            let symbol = table.get(current)?;
            if !matches!(symbol.kind, SymbolKind::Namespace) {
                return None;
            }
            parts.push(symbol.name.clone());
            if let Some(parent) = symbol.parent {
                current = parent;
            } else {
                break;
            }
        }
        parts.reverse();
        Some(parts)
    }

    fn ensure_namespace_scope(&mut self, namespace_id: SymbolId) {
        if self.target.scope_for_owner(namespace_id).is_some() {
            return;
        }
        let parent_scope = if let Some(parent_id) = self
            .target
            .get(namespace_id)
            .and_then(|symbol| symbol.parent)
        {
            if let Some(parent) = self.target.get(parent_id) {
                if matches!(parent.kind, SymbolKind::Namespace) {
                    self.ensure_namespace_scope(parent_id);
                    self.target
                        .scope_for_owner(parent_id)
                        .unwrap_or(ScopeId::GLOBAL)
                } else {
                    ScopeId::GLOBAL
                }
            } else {
                ScopeId::GLOBAL
            }
        } else {
            ScopeId::GLOBAL
        };

        let previous_scope = self.target.current_scope();
        self.target.set_current_scope(parent_scope);
        self.target
            .push_scope(ScopeKind::Namespace, Some(namespace_id));
        self.target.set_current_scope(previous_scope);
    }

    fn import_symbol_kind(&mut self, source_file: FileId, kind: &SymbolKind) -> SymbolKind {
        match kind {
            SymbolKind::Function {
                return_type,
                parameters,
            } => SymbolKind::Function {
                return_type: self.import_type(source_file, *return_type),
                parameters: parameters.clone(),
            },
            SymbolKind::Method {
                return_type,
                parameters,
            } => SymbolKind::Method {
                return_type: return_type.map(|ty| self.import_type(source_file, ty)),
                parameters: parameters.clone(),
            },
            SymbolKind::Property {
                prop_type,
                has_get,
                has_set,
            } => SymbolKind::Property {
                prop_type: self.import_type(source_file, *prop_type),
                has_get: *has_get,
                has_set: *has_set,
            },
            _ => kind.clone(),
        }
    }

    fn import_type(&mut self, source_file: FileId, type_id: TypeId) -> TypeId {
        match self.import_type_outcome(source_file, type_id) {
            SemanticOutcome::Resolved(type_id) => type_id,
            _ => LEGACY_UNKNOWN_TYPE_ID,
        }
    }

    fn import_type_outcome(
        &mut self,
        source_file: FileId,
        type_id: TypeId,
    ) -> SemanticOutcome<TypeId> {
        if type_id.builtin_name().is_some() {
            return SemanticOutcome::Resolved(type_id);
        }

        if let Some(mapped) = self.type_map.get(&(source_file, type_id)).copied() {
            return SemanticOutcome::Resolved(mapped);
        }

        if !self.importing.insert((source_file, type_id)) {
            return SemanticOutcome::SuppressedCascade {
                primary: DiagnosticCode::CyclicDependency,
                range: None,
            };
        }

        let source = match self.sources.get(&source_file).cloned() {
            Some(table) => table,
            None => {
                self.importing.remove(&(source_file, type_id));
                return SemanticOutcome::InvariantViolation {
                    message: SmolStr::new("missing source symbol table during type import"),
                    range: None,
                };
            }
        };
        let Some(ty) = source.type_by_id(type_id).cloned() else {
            self.importing.remove(&(source_file, type_id));
            return SemanticOutcome::Unknown {
                name: None,
                range: None,
            };
        };

        let mapped = match ty {
            Type::Array {
                element,
                dimensions,
            } => {
                let element = self.import_type(source_file, element);
                self.target.register_array_type(element, dimensions)
            }
            Type::Struct { name, fields } => {
                let mut imported_fields = Vec::with_capacity(fields.len());
                for field in fields {
                    let default_initializer = field
                        .default_initializer
                        .and_then(|id| self.target.import_initializer_from(source.as_ref(), id));
                    imported_fields.push(StructField {
                        name: field.name,
                        type_id: self.import_type(source_file, field.type_id),
                        address: field.address,
                        default_initializer,
                    });
                }
                self.target
                    .register_struct_type(name.clone(), imported_fields)
            }
            Type::Union { name, variants } => {
                let mut imported_variants = Vec::with_capacity(variants.len());
                for variant in variants {
                    let default_initializer = variant
                        .default_initializer
                        .and_then(|id| self.target.import_initializer_from(source.as_ref(), id));
                    imported_variants.push(UnionVariant {
                        name: variant.name,
                        type_id: self.import_type(source_file, variant.type_id),
                        address: variant.address,
                        default_initializer,
                    });
                }
                self.target
                    .register_union_type(name.clone(), imported_variants)
            }
            Type::Enum { name, base, values } => {
                let base = self.import_type(source_file, base);
                self.target.register_enum_type(name.clone(), base, values)
            }
            Type::Pointer { target } => {
                let target = self.import_type(source_file, target);
                self.target.register_pointer_type(target)
            }
            Type::Reference { target } => {
                let target = self.import_type(source_file, target);
                self.target.register_reference_type(target)
            }
            Type::Subrange { base, lower, upper } => {
                let base = self.import_type(source_file, base);
                self.target.register_subrange_type(base, lower, upper)
            }
            Type::FunctionBlock { name } => self
                .target
                .register_type(name.clone(), Type::FunctionBlock { name }),
            Type::Class { name } => self
                .target
                .register_type(name.clone(), Type::Class { name }),
            Type::Interface { name } => self
                .target
                .register_type(name.clone(), Type::Interface { name }),
            Type::Alias { name, target } => {
                let target = self.import_type(source_file, target);
                self.target
                    .register_type(name.clone(), Type::Alias { name, target })
            }
            Type::String { max_len } => match max_len {
                Some(len) => self.target.register_type(
                    SmolStr::new(format!("STRING[{}]", len)),
                    Type::String { max_len: Some(len) },
                ),
                None => TypeId::STRING,
            },
            Type::WString { max_len } => match max_len {
                Some(len) => self.target.register_type(
                    SmolStr::new(format!("WSTRING[{}]", len)),
                    Type::WString { max_len: Some(len) },
                ),
                None => TypeId::WSTRING,
            },
            _ => type_id,
        };

        if let Some(source_initializer) = source.type_default_initializer(type_id) {
            if let Some(target_initializer) = self
                .target
                .import_initializer_from(source.as_ref(), source_initializer)
            {
                self.target
                    .set_type_default_initializer(mapped, target_initializer);
            }
        }

        self.type_map.insert((source_file, type_id), mapped);
        self.importing.remove(&(source_file, type_id));
        SemanticOutcome::Resolved(mapped)
    }
}
