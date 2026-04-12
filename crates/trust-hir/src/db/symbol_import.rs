use super::*;
use crate::types::{StructField, UnionVariant};

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
            if self.target.lookup(symbol.name.as_str()).is_some() {
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

        for (old_id, new_id) in id_map.iter() {
            if parent_map.get(old_id).copied().flatten().is_none() {
                if let Some(symbol) = self.target.get(*new_id) {
                    if self
                        .target
                        .lookup_in_scope(ScopeId::GLOBAL, symbol.name.as_str())
                        .is_none()
                    {
                        let _ = self.target.define_in_scope(
                            ScopeId::GLOBAL,
                            symbol.name.clone(),
                            *new_id,
                        );
                    }
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
                                let _ =
                                    self.target.define_in_scope(scope_id, name.clone(), *new_id);
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
                                    let _ = self.target.define_in_scope(
                                        ScopeId::GLOBAL,
                                        name.clone(),
                                        *new_id,
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(base) = source.extends_name(*old_id) {
                self.target.set_extends(*new_id, base.clone());
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
        if type_id.builtin_name().is_some() {
            return type_id;
        }

        if let Some(mapped) = self.type_map.get(&(source_file, type_id)).copied() {
            return mapped;
        }

        if !self.importing.insert((source_file, type_id)) {
            return TypeId::UNKNOWN;
        }

        let source = match self.sources.get(&source_file) {
            Some(table) => table,
            None => {
                self.importing.remove(&(source_file, type_id));
                return TypeId::UNKNOWN;
            }
        };
        let Some(ty) = source.type_by_id(type_id).cloned() else {
            self.importing.remove(&(source_file, type_id));
            return TypeId::UNKNOWN;
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
                let fields = fields
                    .into_iter()
                    .map(|field| StructField {
                        name: field.name,
                        type_id: self.import_type(source_file, field.type_id),
                        address: field.address,
                    })
                    .collect();
                self.target.register_struct_type(name.clone(), fields)
            }
            Type::Union { name, variants } => {
                let variants = variants
                    .into_iter()
                    .map(|variant| UnionVariant {
                        name: variant.name,
                        type_id: self.import_type(source_file, variant.type_id),
                        address: variant.address,
                    })
                    .collect();
                self.target.register_union_type(name.clone(), variants)
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

        self.type_map.insert((source_file, type_id), mapped);
        self.importing.remove(&(source_file, type_id));
        mapped
    }
}
