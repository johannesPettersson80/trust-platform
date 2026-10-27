impl SourceDatabase for Database {
    fn source_text(&self, file_id: FileId) -> Arc<String> {
        self.sources
            .get(&file_id)
            .cloned()
            .unwrap_or_else(|| Arc::new(String::new()))
    }

    fn set_source_text(&mut self, file_id: FileId, text: String) {
        if self
            .sources
            .get(&file_id)
            .is_some_and(|existing| existing.as_ref() == &text)
        {
            return;
        }

        let text = Arc::new(text);
        self.sources.insert(file_id, text.clone());
        let new_revision = self.source_revision.fetch_add(1, Ordering::Relaxed) + 1;

        self.with_salsa_state(|state| {
            let mut file_set_changed = state.project_inputs.is_none();
            if let Some(source) = state.sources.get(&file_id).copied() {
                source.set_text(&mut state.db).to(text.as_ref().clone());
            } else {
                let source = salsa_backend::SourceInput::new(&state.db, text.as_ref().clone());
                state.sources.insert(file_id, source);
                file_set_changed = true;
            }
            if file_set_changed {
                salsa_backend::sync_project_inputs(state);
            }
            state.synced_revision = new_revision;
        });
    }
}

impl SemanticDatabase for Database {
    fn file_symbols(&self, file_id: FileId) -> Arc<SymbolTable> {
        let Some((db, project)) = self.with_synced_salsa_state(|state| {
            state
                .sources
                .contains_key(&file_id)
                .then_some((state.db.clone(), salsa_backend::project_inputs(state)))
        }) else {
            return Arc::new(SymbolTable::default());
        };

        salsa::Cancelled::catch(|| salsa_backend::file_symbols_query(&db, project, file_id).clone())
            .unwrap_or_else(|_| Arc::new(SymbolTable::default()))
    }

    fn resolve_name(&self, file_id: FileId, name: &str) -> Option<SymbolId> {
        let symbols = self.file_symbols(file_id);
        if name.contains('.') {
            let parts = name.split('.').map(SmolStr::new).collect::<Vec<_>>();
            symbols.resolve_qualified(&parts)
        } else {
            symbols.lookup(name)
        }
    }

    fn type_of(&self, file_id: FileId, expr_id: u32) -> TypeId {
        self.type_of_salsa(file_id, expr_id)
    }

    fn expr_id_at_offset(&self, file_id: FileId, offset: u32) -> Option<u32> {
        let (db, source) = self.source_handle_for_file(file_id)?;

        salsa::Cancelled::catch(|| {
            let index = salsa_backend::expression_index_query(&db, source);
            index.id_at_offset(TextSize::from(offset))
        })
        .ok()
        .flatten()
    }

    fn expr_id_for_range(&self, file_id: FileId, start: u32, end: u32) -> Option<u32> {
        let (db, source) = self.source_handle_for_file(file_id)?;

        salsa::Cancelled::catch(|| {
            let index = salsa_backend::expression_index_query(&db, source);
            index.id_for_range(TextRange::new(TextSize::from(start), TextSize::from(end)))
        })
        .ok()
        .flatten()
    }

    fn diagnostics(&self, file_id: FileId) -> Arc<Vec<Diagnostic>> {
        self.diagnostics_salsa(file_id)
    }

    fn analyze(&self, file_id: FileId) -> Arc<FileAnalysis> {
        self.analyze_salsa(file_id)
    }
}
