use super::collector::SymbolCollector;
use super::diagnostics::{
    add_unused_symbol_warnings, check_abstract_instantiations, check_class_semantics,
    check_configuration_semantics, check_cyclomatic_complexity, check_extends_implements_semantics,
    check_global_external_links_with_project, check_interface_conformance, check_nondeterminism,
    check_property_accessors, check_shared_global_task_hazards, check_unreachable_statements,
    check_using_directives, collect_used_symbols, expression_by_id, expression_context,
    resolve_declared_var_types_with_project, resolve_pending_types_with_table, type_check_file,
};
use super::symbol_import::SymbolImporter;
use super::*;
use rowan::GreenNode;
use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Setter;
use std::sync::atomic::{AtomicU64, Ordering};

#[salsa::db]
#[derive(Clone)]
pub(super) struct SalsaDatabase {
    storage: salsa::Storage<Self>,
    event_stats: Arc<SalsaEventStats>,
}

#[salsa::db]
impl salsa::Database for SalsaDatabase {}

impl Default for SalsaDatabase {
    fn default() -> Self {
        let log_events = std::env::var_os("TRUST_HIR_SALSA_EVENT_LOG").is_some();
        let collect_events =
            log_events || std::env::var_os("TRUST_HIR_SALSA_EVENT_METRICS").is_some();
        Self::with_event_observability(collect_events, log_events)
    }
}

impl SalsaDatabase {
    pub(super) fn with_event_observability(collect_events: bool, log_events: bool) -> Self {
        let event_stats = Arc::new(SalsaEventStats::default());
        let mut builder = salsa::Storage::builder();
        if collect_events || log_events {
            let callback_stats = Arc::clone(&event_stats);
            builder = builder.event_callback(Box::new(move |event| {
                record_salsa_event(&callback_stats, &event, collect_events, log_events);
            }));
        }
        let storage = builder.build();
        Self {
            storage,
            event_stats,
        }
    }

    pub(super) fn event_snapshot(&self) -> SalsaEventSnapshot {
        self.event_stats.snapshot()
    }

    pub(super) fn reset_event_stats(&self) {
        self.event_stats.reset();
    }
}

#[derive(Default, Clone)]
pub(super) struct SalsaState {
    pub(super) db: SalsaDatabase,
    pub(super) sources: FxHashMap<FileId, SourceInput>,
    pub(super) project_inputs: Option<ProjectInputs>,
    pub(super) synced_revision: u64,
    #[cfg(test)]
    pub(super) project_sync_count: u64,
}

impl SalsaState {
    #[cfg(test)]
    pub(super) fn with_event_observability(collect_events: bool, log_events: bool) -> Self {
        Self {
            db: SalsaDatabase::with_event_observability(collect_events, log_events),
            sources: FxHashMap::default(),
            project_inputs: None,
            synced_revision: 0,
            project_sync_count: 0,
        }
    }
}

#[derive(Debug, Default)]
struct SalsaEventStats {
    total: AtomicU64,
    cache_hits: AtomicU64,
    recomputes: AtomicU64,
    invalidations: AtomicU64,
    cancellation_checks: AtomicU64,
    cancellation_flags: AtomicU64,
    wait_blocks: AtomicU64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Aggregated Salsa runtime events for local observability.
pub struct SalsaEventSnapshot {
    /// Total number of observed Salsa events.
    pub total: u64,
    /// Number of memoized values validated without recomputation.
    pub cache_hits: u64,
    /// Number of query executions that recomputed values.
    pub recomputes: u64,
    /// Number of invalidation/discard events.
    pub invalidations: u64,
    /// Number of cooperative cancellation checks reached.
    pub cancellation_checks: u64,
    /// Number of cancellation-flag events triggered.
    pub cancellation_flags: u64,
    /// Number of events where a worker blocked on another worker.
    pub wait_blocks: u64,
}

impl SalsaEventStats {
    fn snapshot(&self) -> SalsaEventSnapshot {
        SalsaEventSnapshot {
            total: self.total.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            recomputes: self.recomputes.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
            cancellation_checks: self.cancellation_checks.load(Ordering::Relaxed),
            cancellation_flags: self.cancellation_flags.load(Ordering::Relaxed),
            wait_blocks: self.wait_blocks.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.total.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.recomputes.store(0, Ordering::Relaxed);
        self.invalidations.store(0, Ordering::Relaxed);
        self.cancellation_checks.store(0, Ordering::Relaxed);
        self.cancellation_flags.store(0, Ordering::Relaxed);
        self.wait_blocks.store(0, Ordering::Relaxed);
    }
}

fn record_salsa_event(
    stats: &SalsaEventStats,
    event: &salsa::Event,
    collect_events: bool,
    log_events: bool,
) {
    if collect_events {
        stats.total.fetch_add(1, Ordering::Relaxed);
        match &event.kind {
            salsa::EventKind::DidValidateMemoizedValue { .. } => {
                stats.cache_hits.fetch_add(1, Ordering::Relaxed);
            }
            salsa::EventKind::WillExecute { .. } => {
                stats.recomputes.fetch_add(1, Ordering::Relaxed);
            }
            salsa::EventKind::WillDiscardStaleOutput { .. }
            | salsa::EventKind::DidDiscard { .. }
            | salsa::EventKind::DidDiscardAccumulated { .. } => {
                stats.invalidations.fetch_add(1, Ordering::Relaxed);
            }
            salsa::EventKind::WillCheckCancellation => {
                stats.cancellation_checks.fetch_add(1, Ordering::Relaxed);
            }
            salsa::EventKind::DidSetCancellationFlag => {
                stats.cancellation_flags.fetch_add(1, Ordering::Relaxed);
            }
            salsa::EventKind::WillBlockOn { .. } => {
                stats.wait_blocks.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    if log_events {
        tracing::debug!(target: "trust_hir::salsa_event", ?event, "salsa event");
    }
}

pub(super) fn sync_project_inputs(state: &mut SalsaState) {
    #[cfg(test)]
    {
        state.project_sync_count = state.project_sync_count.saturating_add(1);
    }
    let mut files: Vec<(FileId, SourceInput)> = state
        .sources
        .iter()
        .map(|(file_id, source)| (*file_id, *source))
        .collect();
    files.sort_by_key(|(file_id, _)| file_id.0);
    if let Some(project) = state.project_inputs {
        project.set_files(&mut state.db).to(files);
    } else {
        state.project_inputs = Some(ProjectInputs::new(&state.db, files));
    }
}

pub(super) fn project_inputs(state: &SalsaState) -> ProjectInputs {
    state
        .project_inputs
        .expect("project inputs should be initialized")
}

#[salsa::input]
pub(super) struct SourceInput {
    #[returns(ref)]
    pub(super) text: String,
}

#[salsa::input]
pub(super) struct ProjectInputs {
    #[returns(ref)]
    pub(super) files: Vec<(FileId, SourceInput)>,
}

#[salsa::tracked(returns(ref))]
pub(super) fn parse_green(db: &dyn salsa::Database, input: SourceInput) -> GreenNode {
    let parsed = parse(input.text(db));
    parsed.syntax().green().into_owned()
}

#[salsa::tracked(returns(ref))]
pub(super) fn file_symbols_query(db: &dyn salsa::Database, input: SourceInput) -> Arc<SymbolTable> {
    let green = parse_green(db, input).clone();
    let root = SyntaxNode::new_root(green);
    let (symbols, _) = SymbolCollector::new().collect(&root);
    Arc::new(symbols)
}

#[salsa::tracked(returns(ref))]
pub(super) fn project_symbol_tables_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
) -> Arc<FxHashMap<FileId, Arc<SymbolTable>>> {
    let mut project_tables: FxHashMap<FileId, Arc<SymbolTable>> = FxHashMap::default();
    for (file_id, input) in project.files(db).iter().copied() {
        cancellation_checkpoint(db);
        project_tables.insert(file_id, file_symbols_query(db, input).clone());
    }
    Arc::new(project_tables)
}

#[salsa::tracked(returns(ref))]
pub(super) fn merged_project_symbols_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
) -> Arc<SymbolTable> {
    cancellation_checkpoint(db);
    let project_tables = project_symbol_tables_query(db, project);
    let mut symbols = project_tables
        .get(&file_id)
        .map(|table| (**table).clone())
        .unwrap_or_default();
    merge_project_symbols(file_id, &mut symbols, project_tables.as_ref());
    Arc::new(symbols)
}

#[salsa::tracked(returns(ref))]
pub(super) fn project_used_symbols_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
) -> Arc<FxHashSet<(FileId, SymbolId)>> {
    cancellation_checkpoint(db);
    let mut used = FxHashSet::default();
    for (file_id, input) in project.files(db).iter().copied() {
        cancellation_checkpoint(db);
        let root = SyntaxNode::new_root(parse_green(db, input).clone());
        let symbols = merged_project_symbols_query(db, project, file_id);
        let used_ids = collect_used_symbols(symbols.as_ref(), &root);
        for symbol_id in used_ids {
            let Some(symbol) = symbols.get(symbol_id) else {
                continue;
            };
            let key = if let Some(origin) = symbol.origin {
                (origin.file_id, origin.symbol_id)
            } else {
                (file_id, symbol_id)
            };
            used.insert(key);
        }
    }
    Arc::new(used)
}

#[salsa::tracked(returns(ref))]
pub(super) fn analyze_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
) -> Arc<FileAnalysis> {
    cancellation_checkpoint(db);
    let Some(ProjectState {
        target_input,
        project_source_inputs,
        project_tables,
    }) = collect_project_state(db, project, file_id)
    else {
        return empty_analysis();
    };

    let root = SyntaxNode::new_root(parse_green(db, target_input).clone());
    let (mut symbols, mut diagnostics, pending_types) =
        SymbolCollector::new().collect_for_project(&root);
    merge_project_symbols(file_id, &mut symbols, project_tables.as_ref());

    let mut builder = DiagnosticBuilder::new();
    resolve_pending_types_with_table(&symbols, pending_types, &mut builder);
    resolve_declared_var_types_with_project(&mut symbols, &root);
    check_global_external_links_with_project(&mut symbols, &root, &mut builder, file_id);
    diagnostics.extend(builder.finish());

    let mut builder = DiagnosticBuilder::new();
    check_class_semantics(&symbols, &root, &mut builder);
    check_abstract_instantiations(&symbols, &root, &mut builder);
    check_extends_implements_semantics(&symbols, &root, &mut builder);
    check_interface_conformance(&symbols, &root, &mut builder);
    check_property_accessors(&symbols, &mut builder);
    diagnostics.extend(builder.finish());

    let mut builder = DiagnosticBuilder::new();
    check_using_directives(&symbols, &mut builder);
    diagnostics.extend(builder.finish());

    let mut builder = DiagnosticBuilder::new();
    check_configuration_semantics(&symbols, &root, &mut builder);
    diagnostics.extend(builder.finish());

    let project_used = project_used_symbols_query(db, project);
    let mut builder = DiagnosticBuilder::new();
    type_check_file(&mut symbols, &root, &mut builder);
    check_unreachable_statements(&root, &mut builder);
    check_cyclomatic_complexity(&root, &mut builder);
    check_nondeterminism(&symbols, &mut builder);
    if has_global_variables(&symbols) {
        let project_roots = project_roots_from_inputs(db, &project_source_inputs);
        check_shared_global_task_hazards(&symbols, &project_roots, file_id, &mut builder);
    }
    add_unused_symbol_warnings(&symbols, file_id, project_used.as_ref(), &mut builder);
    diagnostics.extend(builder.finish());

    Arc::new(FileAnalysis {
        symbols: Arc::new(symbols),
        diagnostics: Arc::new(diagnostics),
    })
}

#[salsa::tracked(returns(ref))]
pub(super) fn diagnostics_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
) -> Arc<Vec<Diagnostic>> {
    analyze_query(db, project, file_id).diagnostics.clone()
}

#[salsa::tracked]
pub(super) fn type_of_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
    expr_id: u32,
) -> TypeId {
    cancellation_checkpoint(db);
    let Some(ProjectState { target_input, .. }) = collect_project_state(db, project, file_id)
    else {
        return TypeId::UNKNOWN;
    };

    let root = SyntaxNode::new_root(parse_green(db, target_input).clone());
    let Some(expr_node) = expression_by_id(&root, expr_id) else {
        return TypeId::UNKNOWN;
    };

    let mut symbols = merged_project_symbols_query(db, project, file_id)
        .as_ref()
        .clone();
    let context = expression_context(&symbols, &expr_node);
    resolve_declared_var_types_with_project(&mut symbols, &root);

    let mut diagnostics = DiagnosticBuilder::new();
    let mut checker = TypeChecker::new(&mut symbols, &mut diagnostics, context.scope_id);
    checker.set_return_type(context.return_type);
    checker.set_receiver_types(context.this_type, context.super_type);
    checker.expr().check_expression(&expr_node)
}

fn empty_analysis() -> Arc<FileAnalysis> {
    Arc::new(FileAnalysis {
        symbols: Arc::new(SymbolTable::default()),
        diagnostics: Arc::new(Vec::new()),
    })
}

#[inline]
fn cancellation_checkpoint(db: &dyn salsa::Database) {
    db.unwind_if_revision_cancelled();
}

struct ProjectState {
    target_input: SourceInput,
    project_source_inputs: FxHashMap<FileId, SourceInput>,
    project_tables: Arc<FxHashMap<FileId, Arc<SymbolTable>>>,
}

fn collect_project_state(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
) -> Option<ProjectState> {
    let files = project.files(db);
    let target_input = files
        .iter()
        .find_map(|(candidate_id, input)| (*candidate_id == file_id).then_some(*input))?;

    let mut project_source_inputs: FxHashMap<FileId, SourceInput> = FxHashMap::default();
    for (candidate_id, input) in files.iter().copied() {
        cancellation_checkpoint(db);
        project_source_inputs.insert(candidate_id, input);
    }
    let project_tables = project_symbol_tables_query(db, project).clone();

    Some(ProjectState {
        target_input,
        project_source_inputs,
        project_tables,
    })
}

fn has_global_variables(symbols: &SymbolTable) -> bool {
    symbols.iter().any(|symbol| {
        matches!(
            symbol.kind,
            SymbolKind::Variable {
                qualifier: VarQualifier::Global
            }
        )
    })
}

fn project_roots_from_inputs(
    db: &dyn salsa::Database,
    source_inputs: &FxHashMap<FileId, SourceInput>,
) -> Vec<(FileId, SyntaxNode)> {
    let mut ordered: Vec<(FileId, SourceInput)> = source_inputs
        .iter()
        .map(|(id, input)| (*id, *input))
        .collect();
    ordered.sort_by_key(|(id, _)| id.0);

    let mut roots = Vec::with_capacity(ordered.len());
    for (file_id, input) in ordered {
        cancellation_checkpoint(db);
        let root = SyntaxNode::new_root(parse_green(db, input).clone());
        roots.push((file_id, root));
    }
    roots
}

fn ordered_table_entries(
    tables: &FxHashMap<FileId, Arc<SymbolTable>>,
) -> Vec<(FileId, &Arc<SymbolTable>)> {
    let mut entries: Vec<(FileId, &Arc<SymbolTable>)> =
        tables.iter().map(|(id, table)| (*id, table)).collect();
    entries.sort_by_key(|(id, _)| id.0);
    entries
}

fn merge_project_symbols(
    file_id: FileId,
    symbols: &mut SymbolTable,
    tables: &FxHashMap<FileId, Arc<SymbolTable>>,
) {
    let ordered_tables = ordered_table_entries(tables);
    let mut importer = SymbolImporter::new(symbols, tables);
    for (other_id, table) in ordered_tables.iter().copied() {
        if other_id == file_id {
            continue;
        }
        importer.import_table(other_id, table);
    }
}
