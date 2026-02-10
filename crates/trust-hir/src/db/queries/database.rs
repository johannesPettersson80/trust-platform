use super::diagnostics::expression_id_at_offset;
use super::symbol_import::SymbolImporter;
use super::*;
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
            });
        };

        salsa::Cancelled::catch(|| salsa_backend::analyze_query(&db, project, file_id).clone())
            .unwrap_or_else(|_| {
                Arc::new(FileAnalysis {
                    symbols: Arc::new(SymbolTable::default()),
                    diagnostics: Arc::new(Vec::new()),
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
            return TypeId::UNKNOWN;
        };

        salsa::Cancelled::catch(|| salsa_backend::type_of_query(&db, project, file_id, expr_id))
            .unwrap_or(TypeId::UNKNOWN)
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
        let Some((db, source)) = self.source_handle_for_file(file_id) else {
            return Arc::new(SymbolTable::default());
        };

        salsa::Cancelled::catch(|| salsa_backend::file_symbols_query(&db, source).clone())
            .unwrap_or_else(|_| Arc::new(SymbolTable::default()))
    }

    fn resolve_name(&self, file_id: FileId, name: &str) -> Option<SymbolId> {
        let symbols = self.file_symbols(file_id);
        symbols.lookup_any(name)
    }

    fn type_of(&self, file_id: FileId, expr_id: u32) -> TypeId {
        self.type_of_salsa(file_id, expr_id)
    }

    fn expr_id_at_offset(&self, file_id: FileId, offset: u32) -> Option<u32> {
        let (db, source) = self.source_handle_for_file(file_id)?;

        salsa::Cancelled::catch(|| {
            let green = salsa_backend::parse_green(&db, source).clone();
            let root = SyntaxNode::new_root(green);
            let offset = TextSize::from(offset);
            expression_id_at_offset(&root, offset)
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

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLock;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;

    fn install_cross_file_fixture(db: &mut Database) -> (FileId, FileId) {
        let file_lib = FileId(10);
        let file_main = FileId(11);
        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(1);\nEND_PROGRAM\n"
                .to_string(),
        );
        (file_lib, file_main)
    }

    fn install_diagnostics_fixture(db: &mut Database) -> FileId {
        let file_lib = FileId(20);
        let file_main = FileId(21);
        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(TRUE);\nEND_PROGRAM\n"
                .to_string(),
        );
        file_main
    }

    fn expr_id_for(db: &Database, file_id: FileId, needle: &str) -> u32 {
        let source = db.source_text(file_id);
        let offset = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing needle '{needle}' in source"))
            as u32;
        db.expr_id_at_offset(file_id, offset)
            .unwrap_or_else(|| panic!("missing expression id for '{needle}'"))
    }

    #[test]
    fn file_symbols_reuses_unchanged_file_across_unrelated_edit() {
        let mut db = Database::new();
        let file_main = FileId(1);
        let file_aux = FileId(2);

        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    counter : INT;\nEND_VAR\ncounter := counter + 1;\nEND_PROGRAM\n"
                .to_string(),
        );
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := TRUE;\nEND_PROGRAM\n".to_string(),
        );

        let before = db.file_symbols(file_main);
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := FALSE;\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.file_symbols(file_main);

        assert!(
            Arc::ptr_eq(&before, &after),
            "unchanged file symbols should be reused across unrelated edits"
        );
    }

    #[test]
    fn file_symbols_recomputes_when_its_file_changes() {
        let mut db = Database::new();
        let file = FileId(3);

        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let before = db.file_symbols(file);

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 42;\nEND_PROGRAM\n".to_string(),
        );
        let after = db.file_symbols(file);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "updated file symbols should not reuse stale analysis"
        );
        assert!(
            after.lookup_any("value").is_some(),
            "updated symbol table should contain new declarations"
        );
    }

    #[test]
    fn analyze_salsa_returns_expected_cross_file_result() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let analysis = db.analyze_salsa(file_main);

        assert!(
            analysis.symbols.lookup_any("AddOne").is_some(),
            "cross-file function should be available in analyzed symbol table"
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.is_error()),
            "valid fixture should not emit error diagnostics"
        );
    }

    #[test]
    fn analyze_salsa_reuses_result_without_edits() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let first = db.analyze_salsa(file_main);
        let second = db.analyze_salsa(file_main);

        assert!(
            Arc::ptr_eq(&first, &second),
            "salsa analyze should reuse cached analysis when inputs are unchanged"
        );
    }

    #[test]
    fn analyze_salsa_recomputes_after_target_edit() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);

        let before = db.analyze_salsa(file_main);
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(2);\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.analyze_salsa(file_main);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "salsa analyze should invalidate cached analysis when the target file changes"
        );
    }

    #[test]
    fn diagnostics_salsa_reuses_result_without_edits() {
        let mut db = Database::new();
        let file_main = install_diagnostics_fixture(&mut db);

        let first = db.diagnostics_salsa(file_main);
        let second = db.diagnostics_salsa(file_main);

        assert!(
            Arc::ptr_eq(&first, &second),
            "salsa diagnostics should reuse cached diagnostics when inputs are unchanged"
        );
    }

    #[test]
    fn diagnostics_salsa_recomputes_after_target_edit() {
        let mut db = Database::new();
        let file_main = install_diagnostics_fixture(&mut db);

        let before = db.diagnostics_salsa(file_main);
        db.set_source_text(
            file_main,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := AddOne(1);\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.diagnostics_salsa(file_main);

        assert!(
            !Arc::ptr_eq(&before, &after),
            "salsa diagnostics should invalidate cached result when the target file changes"
        );
        assert!(
            after.len() < before.len(),
            "fixing invalid call should reduce diagnostics"
        );
    }

    #[test]
    fn type_of_salsa_returns_expected_type_for_cross_file_call() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);
        let expr_id = expr_id_for(&db, file_main, "AddOne(1)");

        let ty = db.type_of_salsa(file_main, expr_id);
        assert_eq!(
            ty,
            TypeId::INT,
            "type_of should resolve AddOne(INT) call result type"
        );
    }

    #[test]
    fn type_of_salsa_stable_across_unrelated_edit() {
        let mut db = Database::new();
        let (_file_lib, file_main) = install_cross_file_fixture(&mut db);
        let file_aux = FileId(22);
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := TRUE;\nEND_PROGRAM\n".to_string(),
        );

        let expr_id = expr_id_for(&db, file_main, "AddOne(1)");
        let before = db.type_of_salsa(file_main, expr_id);
        db.set_source_text(
            file_aux,
            "PROGRAM Aux\nVAR\n    flag : BOOL;\nEND_VAR\nflag := FALSE;\nEND_PROGRAM\n"
                .to_string(),
        );
        let after = db.type_of_salsa(file_main, expr_id);

        assert_eq!(
            before, after,
            "unrelated edits should not change typed expression result"
        );
    }

    #[test]
    fn type_of_salsa_recomputes_after_dependency_edit() {
        let mut db = Database::new();
        let (file_lib, file_main) = install_cross_file_fixture(&mut db);
        let expr_id_before = expr_id_for(&db, file_main, "AddOne(1)");
        let before = db.type_of_salsa(file_main, expr_id_before);

        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : BOOL\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x > 0;\nEND_FUNCTION\n"
                .to_string(),
        );

        let expr_id_after = expr_id_for(&db, file_main, "AddOne(1)");
        let after = db.type_of_salsa(file_main, expr_id_after);

        assert_ne!(
            before, after,
            "type_of should invalidate when dependent declaration types change"
        );
        assert_eq!(
            after,
            TypeId::BOOL,
            "updated dependency should produce BOOL"
        );
    }

    #[test]
    fn remove_source_text_clears_single_file_queries() {
        let mut db = Database::new();
        let file = FileId(30);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );
        assert!(db.file_symbols(file).lookup_any("value").is_some());

        db.remove_source_text(file);

        assert_eq!(db.source_text(file).as_str(), "");
        assert!(db.file_symbols(file).lookup_any("value").is_none());
        assert!(db.diagnostics(file).is_empty());
    }

    #[test]
    fn remove_source_text_invalidates_cross_file_dependency() {
        let mut db = Database::new();
        let (file_lib, file_main) = install_cross_file_fixture(&mut db);
        let before = db.analyze(file_main);
        assert!(before
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.is_error()));

        db.remove_source_text(file_lib);
        let after = db.analyze(file_main);

        assert!(
            after
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.is_error()),
            "missing dependency should emit an error"
        );
    }

    #[test]
    fn remove_and_readd_source_restores_cross_file_resolution() {
        let mut db = Database::new();
        let (file_lib, file_main) = install_cross_file_fixture(&mut db);
        db.remove_source_text(file_lib);
        db.set_source_text(
            file_lib,
            "FUNCTION AddOne : INT\nVAR_INPUT\n    x : INT;\nEND_VAR\nAddOne := x + 1;\nEND_FUNCTION\n"
                .to_string(),
        );

        let analysis = db.analyze(file_main);
        assert!(analysis.symbols.lookup_any("AddOne").is_some());
        assert!(analysis
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.is_error()));
    }

    #[test]
    fn source_text_and_symbols_stay_consistent_after_edit() {
        let mut db = Database::new();
        let file = FileId(31);
        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 7;\nEND_PROGRAM\n".to_string(),
        );

        assert!(db.source_text(file).contains("value : INT"));
        assert!(db.file_symbols(file).lookup_any("value").is_some());
    }

    #[test]
    fn set_source_text_existing_file_skips_project_input_resync() {
        let mut db = Database::new();
        let file = FileId(32);
        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let sync_before = db.with_salsa_state_read(|state| state.project_sync_count);

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );
        let sync_after = db.with_salsa_state_read(|state| state.project_sync_count);

        assert_eq!(
            sync_before, sync_after,
            "editing existing file text should not rebuild project input membership"
        );
    }

    #[test]
    fn set_source_text_same_content_keeps_source_revision() {
        let mut db = Database::new();
        let file = FileId(33);
        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let before = db.source_revision.load(Ordering::Relaxed);

        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let after = db.source_revision.load(Ordering::Relaxed);

        assert_eq!(
            before, after,
            "setting identical source content should not bump source revision"
        );
    }

    #[test]
    fn remove_missing_source_keeps_source_revision() {
        let mut db = Database::new();
        let before = db.source_revision.load(Ordering::Relaxed);

        db.remove_source_text(FileId(34));
        let after = db.source_revision.load(Ordering::Relaxed);

        assert_eq!(
            before, after,
            "removing unknown source should not bump source revision"
        );
    }

    #[test]
    fn analyze_syncs_stale_salsa_state_revision() {
        let mut db = Database::new();
        let file = FileId(35);
        db.set_source_text(file, "PROGRAM Main\nEND_PROGRAM\n".to_string());
        let current = db.source_revision.load(Ordering::Relaxed);

        db.with_salsa_state(|state| {
            state.synced_revision = 0;
        });

        let _ = db.analyze_salsa(file);
        let synced = db.with_salsa_state(|state| state.synced_revision);
        assert_eq!(
            synced, current,
            "analyze should refresh stale salsa state to current source revision"
        );
    }

    #[test]
    fn expr_id_at_offset_returns_none_for_missing_file() {
        let db = Database::new();
        assert!(
            db.expr_id_at_offset(FileId(36), 0).is_none(),
            "missing files should not produce expression ids"
        );
    }

    #[test]
    fn expr_id_at_offset_tracks_updated_source() {
        let mut db = Database::new();
        let file = FileId(37);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );

        let old_offset = db
            .source_text(file)
            .find("1")
            .expect("old literal should exist") as u32;
        assert!(
            db.expr_id_at_offset(file, old_offset).is_some(),
            "initial source should resolve an expression id"
        );

        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 2;\nEND_PROGRAM\n"
                .to_string(),
        );

        let new_offset = db
            .source_text(file)
            .find("value + 2")
            .expect("updated expression should exist") as u32;
        assert!(
            db.expr_id_at_offset(file, new_offset).is_some(),
            "updated source should resolve expression ids from fresh parse cache"
        );
    }

    #[test]
    fn salsa_event_counters_emit_query_categories() {
        let mut db = Database::new_with_salsa_observability();
        let file = FileId(39);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 1;\nEND_PROGRAM\n"
                .to_string(),
        );

        db.reset_salsa_event_counters();
        let _ = db.file_symbols(file);
        let first = db.salsa_event_snapshot();
        assert!(
            first.total > 0 && first.recomputes > 0,
            "first query should emit observable execution events"
        );

        let _ = db.file_symbols(file);
        let second = db.salsa_event_snapshot();
        assert!(
            second.total > first.total,
            "second query should continue emitting events"
        );
        assert!(
            second.cache_hits >= first.cache_hits,
            "memoized query path should not decrease cache-hit counters"
        );
    }

    #[test]
    fn cancellation_requests_keep_queries_stable() {
        let mut setup_db = Database::new_with_salsa_observability();
        let file = FileId(40);
        setup_db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := value + 1;\nEND_PROGRAM\n"
                .to_string(),
        );
        let db = Arc::new(setup_db);

        let worker_db = Arc::clone(&db);
        let worker = thread::spawn(move || {
            for _ in 0..80 {
                let _ = worker_db.analyze(file);
                let _ = worker_db.diagnostics(file);
            }
        });

        for _ in 0..80 {
            db.trigger_salsa_cancellation();
        }

        worker
            .join()
            .expect("query worker should finish without panic");
        let snapshot = db.salsa_event_snapshot();
        assert!(
            snapshot.cancellation_flags > 0,
            "cancellation requests should emit cancellation event counters"
        );
    }

    #[test]
    fn concurrent_edit_and_query_loops_do_not_panic() {
        let file = FileId(37);
        let db = Arc::new(RwLock::new(Database::new()));
        db.write().set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 0;\nEND_PROGRAM\n".to_string(),
        );

        let writer_db = Arc::clone(&db);
        let writer = thread::spawn(move || {
            for value in 0..120 {
                writer_db.write().set_source_text(
                    file,
                    format!(
                        "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := {};\nEND_PROGRAM\n",
                        value % 10
                    ),
                );
            }
        });

        let mut readers = Vec::new();
        for _ in 0..2 {
            let reader_db = Arc::clone(&db);
            readers.push(thread::spawn(move || {
                for _ in 0..200 {
                    let guard = reader_db.read();
                    let analysis = guard.analyze(file);
                    assert!(
                        analysis
                            .diagnostics
                            .iter()
                            .all(|diagnostic| !diagnostic.is_error()),
                        "concurrent read path should remain stable while edits happen"
                    );
                    let source = guard.source_text(file);
                    let offset = source.find("value :=").unwrap_or(0) as u32;
                    let _ = guard.expr_id_at_offset(file, offset);
                    let _ = guard.file_symbols(file);
                    let _ = guard.diagnostics(file);
                }
            }));
        }

        writer.join().expect("writer thread should finish");
        for reader in readers {
            reader.join().expect("reader thread should finish");
        }
    }

    #[test]
    fn query_boundary_sequence_no_longer_panics() {
        let mut db = Database::new();
        let file = FileId(38);
        db.set_source_text(
            file,
            "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := 1;\nEND_PROGRAM\n".to_string(),
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for value in 0..100 {
                db.set_source_text(
                    file,
                    format!(
                        "PROGRAM Main\nVAR\n    value : INT;\nEND_VAR\nvalue := {};\nEND_PROGRAM\n",
                        value
                    ),
                );
                let _ = db.file_symbols(file);
                let _ = db.analyze(file);
                let _ = db.diagnostics(file);
                let source = db.source_text(file);
                let offset = source.find("value :=").unwrap_or(0) as u32;
                let _ = db.expr_id_at_offset(file, offset);
            }
        }));

        assert!(
            result.is_ok(),
            "query boundary sequence should not panic after owned-state refactor"
        );
    }
}
