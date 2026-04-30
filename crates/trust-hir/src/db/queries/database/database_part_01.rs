use super::symbol_import::SymbolImporter;
use super::*;
use crate::semantic::LEGACY_UNKNOWN_TYPE_ID;
use rustc_hash::FxHashSet;
use salsa::Setter;
use std::sync::atomic::Ordering;

impl Database {
    /// Creates a new empty database.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn source_revision(&self) -> u64 {
        self.source_revision.load(Ordering::Relaxed)
    }

    fn with_salsa_state<R>(&self, f: impl FnOnce(&mut salsa_backend::SalsaState) -> R) -> R {
        let mut state = self.salsa_state.lock();
        f(&mut state)
    }

    fn with_salsa_state_read<R>(&self, f: impl FnOnce(&salsa_backend::SalsaState) -> R) -> R {
        let state = self.salsa_state.lock();
        f(&state)
    }

    fn with_synced_salsa_state<R>(&self, f: impl FnOnce(&salsa_backend::SalsaState) -> R) -> R {
        let revision = self.source_revision();
        self.with_salsa_state(|state| {
            if state.synced_revision != revision {
                self.prepare_salsa_project(state);
                state.synced_revision = revision;
            }
            f(state)
        })
    }

    fn source_input_for_file(
        &self,
        state: &mut salsa_backend::SalsaState,
        file_id: FileId,
    ) -> Option<salsa_backend::SourceInput> {
        if let Some(source) = state.sources.get(&file_id).copied() {
            let Some(text) = self.sources.get(&file_id) else {
                state.sources.remove(&file_id);
                salsa_backend::sync_project_inputs(state);
                return None;
            };
            if source.text(&state.db) != text.as_ref().as_str() {
                source.set_text(&mut state.db).to(text.as_ref().clone());
            }
            return Some(source);
        }

        let text = self.sources.get(&file_id)?;
        let source = salsa_backend::SourceInput::new(&state.db, text.as_ref().clone());
        state.sources.insert(file_id, source);
        salsa_backend::sync_project_inputs(state);
        Some(source)
    }

    fn source_handle_for_file(
        &self,
        file_id: FileId,
    ) -> Option<(salsa_backend::SalsaDatabase, salsa_backend::SourceInput)> {
        if let Some(result) = self.with_salsa_state_read(|state| {
            state
                .sources
                .get(&file_id)
                .copied()
                .map(|source| (state.db.clone(), source))
        }) {
            return Some(result);
        }

        self.with_salsa_state(|state| {
            self.source_input_for_file(state, file_id)
                .map(|source| (state.db.clone(), source))
        })
    }

    fn project_symbol_tables(&self) -> FxHashMap<FileId, Arc<SymbolTable>> {
        let mut tables = FxHashMap::default();
        for &file_id in self.sources.keys() {
            tables.insert(file_id, self.file_symbols(file_id));
        }
        tables
    }

    /// Returns all known file IDs.
    pub fn file_ids(&self) -> Vec<FileId> {
        self.sources.keys().copied().collect()
    }

    /// Returns aggregated Salsa event counters for observability.
    pub fn salsa_event_snapshot(&self) -> SalsaEventSnapshot {
        self.with_salsa_state_read(|state| state.db.event_snapshot())
    }

    /// Clears Salsa event counters.
    pub fn reset_salsa_event_counters(&self) {
        self.with_salsa_state(|state| state.db.reset_event_stats());
    }

    /// Requests cancellation of running Salsa computations.
    pub fn trigger_salsa_cancellation(&self) {
        self.with_salsa_state(|state| {
            salsa::Database::trigger_cancellation(&mut state.db);
        });
    }

    /// Remove source text and cached query inputs for a file.
    pub fn remove_source_text(&mut self, file_id: FileId) {
        if self.sources.remove(&file_id).is_none() {
            return;
        }

        let new_revision = self.source_revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.with_salsa_state(|state| {
            state.sources.remove(&file_id);
            salsa_backend::sync_project_inputs(state);
            state.synced_revision = new_revision;
        });
    }

    fn merge_project_symbols_filtered(
        &self,
        file_id: FileId,
        symbols: &mut SymbolTable,
        allowed_files: &FxHashSet<FileId>,
    ) {
        if allowed_files.is_empty() {
            return;
        }
        let tables = self.project_symbol_tables();
        let mut ordered_ids: Vec<FileId> = allowed_files.iter().copied().collect();
        ordered_ids.sort_by_key(|id| id.0);
        let mut importer = SymbolImporter::new(symbols, &tables);
        for other_id in ordered_ids {
            if other_id == file_id {
                continue;
            }
            let Some(table) = tables.get(&other_id) else {
                continue;
            };
            importer.import_table(other_id, table);
        }
    }

    fn analyze_salsa(&self, file_id: FileId) -> Arc<FileAnalysis> {
        let Some((db, project)) = self.with_synced_salsa_state(|state| {
            state
                .sources
                .contains_key(&file_id)
                .then_some((state.db.clone(), salsa_backend::project_inputs(state)))
        }) else {
            return Arc::new(FileAnalysis {
                symbols: Arc::new(SymbolTable::default()),
                diagnostics: Arc::new(Vec::new()),
                expression_types: Arc::new(FxHashMap::default()),
                declaration_catalog: Arc::new(DeclarationCatalog::default()),
            });
        };

        salsa::Cancelled::catch(|| salsa_backend::analyze_query(&db, project, file_id).clone())
            .unwrap_or_else(|_| {
                Arc::new(FileAnalysis {
                    symbols: Arc::new(SymbolTable::default()),
                    diagnostics: Arc::new(Vec::new()),
                    expression_types: Arc::new(FxHashMap::default()),
                    declaration_catalog: Arc::new(DeclarationCatalog::default()),
                })
            })
    }

    fn diagnostics_salsa(&self, file_id: FileId) -> Arc<Vec<Diagnostic>> {
        let Some((db, project)) = self.with_synced_salsa_state(|state| {
            state
                .sources
                .contains_key(&file_id)
                .then_some((state.db.clone(), salsa_backend::project_inputs(state)))
        }) else {
            return Arc::new(Vec::new());
        };

        salsa::Cancelled::catch(|| salsa_backend::diagnostics_query(&db, project, file_id).clone())
            .unwrap_or_else(|_| Arc::new(Vec::new()))
    }

    fn type_of_salsa(&self, file_id: FileId, expr_id: u32) -> TypeId {
        let Some((db, project)) = self.with_synced_salsa_state(|state| {
            state
                .sources
                .contains_key(&file_id)
                .then_some((state.db.clone(), salsa_backend::project_inputs(state)))
        }) else {
            return LEGACY_UNKNOWN_TYPE_ID;
        };

        salsa::Cancelled::catch(|| salsa_backend::type_of_query(&db, project, file_id, expr_id))
            .unwrap_or(LEGACY_UNKNOWN_TYPE_ID)
    }

    fn prepare_salsa_project(&self, state: &mut salsa_backend::SalsaState) {
        let mut removed_files = false;
        state.sources.retain(|file_id, _| {
            let keep = self.sources.contains_key(file_id);
            if !keep {
                removed_files = true;
            }
            keep
        });

        let mut project_changed = false;
        for (&known_file_id, text) in &self.sources {
            if let Some(source) = state.sources.get(&known_file_id).copied() {
                if source.text(&state.db) != text.as_ref().as_str() {
                    source.set_text(&mut state.db).to(text.as_ref().clone());
                }
                continue;
            }
            let source = salsa_backend::SourceInput::new(&state.db, text.as_ref().clone());
            state.sources.insert(known_file_id, source);
            project_changed = true;
        }

        if removed_files || project_changed || state.project_inputs.is_none() {
            salsa_backend::sync_project_inputs(state);
        }
    }

    /// Returns a symbol table augmented with project-wide symbols.
    pub fn file_symbols_with_project(&self, file_id: FileId) -> Arc<SymbolTable> {
        self.analyze(file_id).symbols.clone()
    }

    /// Returns a symbol table augmented with project symbols filtered to a file set.
    pub fn file_symbols_with_project_filtered(
        &self,
        file_id: FileId,
        allowed_files: &FxHashSet<FileId>,
    ) -> Arc<SymbolTable> {
        let base = self.file_symbols(file_id);
        let mut symbols = (*base).clone();
        self.merge_project_symbols_filtered(file_id, &mut symbols, allowed_files);
        Arc::new(symbols)
    }
}
