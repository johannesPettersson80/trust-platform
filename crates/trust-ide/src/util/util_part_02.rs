pub(crate) struct SymbolFilter<'a> {
    symbols: &'a SymbolTable,
}

impl<'a> SymbolFilter<'a> {
    pub(crate) fn new(symbols: &'a SymbolTable) -> Self {
        Self { symbols }
    }

    pub(crate) fn symbols(&self) -> &'a SymbolTable {
        self.symbols
    }

    pub(crate) fn scope_symbols(&self, scope_id: ScopeId) -> Vec<&'a Symbol> {
        let mut items = Vec::new();
        let mut seen: FxHashSet<String> = FxHashSet::default();
        let mut current = Some(scope_id);

        while let Some(scope_id) = current {
            let Some(scope) = self.symbols.get_scope(scope_id) else {
                break;
            };
            for symbol_id in scope.symbol_ids() {
                let Some(symbol) = self.symbols.get(*symbol_id) else {
                    continue;
                };
                if !seen.insert(symbol.name.to_ascii_uppercase()) {
                    continue;
                }
                items.push(symbol);
            }
            current = scope.parent;
        }

        items
    }

    pub(crate) fn symbol_at_range(&self, range: TextRange) -> Option<&'a Symbol> {
        self.symbols.iter().find(|sym| sym.range == range)
    }

    pub(crate) fn resolve_in_scope(&self, name: &str, scope_id: ScopeId) -> Option<&'a Symbol> {
        self.symbols
            .resolve(name, scope_id)
            .and_then(|symbol_id| self.symbols.get(symbol_id))
    }

    pub(crate) fn lookup_global(&self, name: &str) -> Option<&'a Symbol> {
        self.symbols
            .lookup(name)
            .and_then(|symbol_id| self.symbols.get(symbol_id))
    }

    pub(crate) fn type_symbols(&self) -> impl Iterator<Item = &'a Symbol> {
        self.symbols.iter().filter(|symbol| {
            matches!(
                symbol.kind,
                SymbolKind::Type
                    | SymbolKind::FunctionBlock
                    | SymbolKind::Class
                    | SymbolKind::Interface
            )
        })
    }

    pub(crate) fn symbol_with_type_id<F>(&self, type_id: TypeId, predicate: F) -> Option<&'a Symbol>
    where
        F: Fn(&Symbol) -> bool,
    {
        self.symbols
            .iter()
            .find(|sym| sym.type_id == type_id && predicate(sym))
    }

    pub(crate) fn owner_for_type(&self, type_id: TypeId) -> Option<SymbolId> {
        self.symbols
            .iter()
            .find(|sym| {
                sym.type_id == type_id
                    && matches!(
                        sym.kind,
                        SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface
                    )
            })
            .map(|sym| sym.id)
    }

    pub(crate) fn members_of_owner(&self, owner_id: SymbolId) -> impl Iterator<Item = &'a Symbol> {
        self.symbols
            .iter()
            .filter(move |sym| sym.parent == Some(owner_id))
    }

    pub(crate) fn members_in_hierarchy<F>(
        &self,
        owner_id: SymbolId,
        mut predicate: F,
    ) -> Vec<&'a Symbol>
    where
        F: FnMut(&Symbol) -> bool,
    {
        let mut items = Vec::new();
        let mut seen: FxHashSet<String> = FxHashSet::default();
        let mut current = Some(owner_id);

        while let Some(owner_id) = current {
            for symbol in self
                .symbols
                .iter()
                .filter(|sym| sym.parent == Some(owner_id))
            {
                if !predicate(symbol) {
                    continue;
                }
                if !seen.insert(symbol.name.to_ascii_uppercase()) {
                    continue;
                }
                items.push(symbol);
            }
            current = self
                .symbols
                .extends_reference(owner_id)
                .and_then(|reference| {
                    let name = reference.name().display();
                    self.symbols
                        .resolve_oop_reference_for_owner(owner_id, name.as_str())
                });
        }

        items
    }
}
