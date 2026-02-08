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
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

#[salsa::db]
#[derive(Default, Clone)]
pub(super) struct SalsaDatabase {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for SalsaDatabase {
    fn salsa_event(&self, _event: &dyn Fn() -> salsa::Event) {}
}

#[derive(Default, Clone)]
pub(super) struct SalsaState {
    pub(super) db: SalsaDatabase,
    pub(super) sources: FxHashMap<FileId, SourceInput>,
    pub(super) project_inputs: Option<ProjectInputs>,
}

thread_local! {
    static SALSA_STATES: RefCell<FxHashMap<u64, Rc<RefCell<SalsaState>>>> = RefCell::new(FxHashMap::default());
}

static NEXT_SALSA_STATE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) fn allocate_state_id() -> u64 {
    NEXT_SALSA_STATE_ID.fetch_add(1, Ordering::Relaxed)
}

fn state_cell(state_id: u64) -> Rc<RefCell<SalsaState>> {
    SALSA_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states
            .entry(state_id)
            .or_insert_with(|| Rc::new(RefCell::new(SalsaState::default())))
            .clone()
    })
}

pub(super) fn with_state<R>(state_id: u64, f: impl FnOnce(&mut SalsaState) -> R) -> R {
    let state = state_cell(state_id);
    let mut state = state
        .try_borrow_mut()
        .expect("reentrant mutable Salsa state access");
    f(&mut state)
}

pub(super) fn with_state_read<R>(state_id: u64, f: impl FnOnce(&SalsaState) -> R) -> R {
    let state = state_cell(state_id);
    let state = state.borrow();
    f(&state)
}

pub(super) fn remove_state(state_id: u64) {
    SALSA_STATES.with(|states| {
        states.borrow_mut().remove(&state_id);
    });
}

pub(super) fn sync_project_inputs(state: &mut SalsaState) {
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
    #[return_ref]
    pub(super) text: String,
}

#[salsa::input]
pub(super) struct ProjectInputs {
    #[return_ref]
    pub(super) files: Vec<(FileId, SourceInput)>,
}

#[salsa::tracked(return_ref)]
pub(super) fn parse_green(db: &dyn salsa::Database, input: SourceInput) -> GreenNode {
    let parsed = parse(input.text(db));
    parsed.syntax().green().into_owned()
}

#[salsa::tracked(return_ref)]
pub(super) fn file_symbols_query(db: &dyn salsa::Database, input: SourceInput) -> Arc<SymbolTable> {
    let green = parse_green(db, input).clone();
    let root = SyntaxNode::new_root(green);
    let (symbols, _) = SymbolCollector::new().collect(&root);
    Arc::new(symbols)
}

#[salsa::tracked(return_ref)]
pub(super) fn analyze_query(
    db: &dyn salsa::Database,
    project: ProjectInputs,
    file_id: FileId,
) -> Arc<FileAnalysis> {
    let Some(ProjectState {
        target_input,
        project_source_inputs,
        project_sources,
        project_tables,
    }) = collect_project_state(db, project, file_id)
    else {
        return empty_analysis();
    };

    let root = SyntaxNode::new_root(parse_green(db, target_input).clone());
    let (mut symbols, mut diagnostics, pending_types) =
        SymbolCollector::new().collect_for_project(&root);
    merge_project_symbols(file_id, &mut symbols, &project_tables);

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

    let project_used = collect_project_used_symbols(db, &project_source_inputs, &project_tables);
    let mut builder = DiagnosticBuilder::new();
    type_check_file(&mut symbols, &root, &mut builder);
    check_unreachable_statements(&root, &mut builder);
    check_cyclomatic_complexity(&root, &mut builder);
    check_nondeterminism(&symbols, &mut builder);
    check_shared_global_task_hazards(&symbols, &project_sources, file_id, &mut builder);
    add_unused_symbol_warnings(&symbols, file_id, &project_used, &mut builder);
    diagnostics.extend(builder.finish());

    Arc::new(FileAnalysis {
        symbols: Arc::new(symbols),
        diagnostics: Arc::new(diagnostics),
    })
}

#[salsa::tracked(return_ref)]
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
    let Some(ProjectState {
        target_input,
        project_tables,
        ..
    }) = collect_project_state(db, project, file_id)
    else {
        return TypeId::UNKNOWN;
    };

    let root = SyntaxNode::new_root(parse_green(db, target_input).clone());
    let Some(expr_node) = expression_by_id(&root, expr_id) else {
        return TypeId::UNKNOWN;
    };

    let mut symbols = project_tables
        .get(&file_id)
        .map(|table| (**table).clone())
        .unwrap_or_default();
    let context = expression_context(&symbols, &expr_node);
    merge_project_symbols(file_id, &mut symbols, &project_tables);
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

struct ProjectState {
    target_input: SourceInput,
    project_source_inputs: FxHashMap<FileId, SourceInput>,
    project_sources: FxHashMap<FileId, Arc<String>>,
    project_tables: FxHashMap<FileId, Arc<SymbolTable>>,
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
    let mut project_sources: FxHashMap<FileId, Arc<String>> = FxHashMap::default();
    let mut project_tables: FxHashMap<FileId, Arc<SymbolTable>> = FxHashMap::default();
    for (candidate_id, input) in files.iter().copied() {
        project_source_inputs.insert(candidate_id, input);
        let source = Arc::new(input.text(db).clone());
        project_sources.insert(candidate_id, source);
        project_tables.insert(candidate_id, file_symbols_query(db, input).clone());
    }

    Some(ProjectState {
        target_input,
        project_source_inputs,
        project_sources,
        project_tables,
    })
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

fn collect_project_used_symbols(
    db: &dyn salsa::Database,
    source_inputs: &FxHashMap<FileId, SourceInput>,
    tables: &FxHashMap<FileId, Arc<SymbolTable>>,
) -> FxHashSet<(FileId, SymbolId)> {
    let mut used = FxHashSet::default();
    let ordered_tables = ordered_table_entries(tables);

    for (&file_id, input) in source_inputs {
        let root = SyntaxNode::new_root(parse_green(db, *input).clone());
        let mut symbols = tables
            .get(&file_id)
            .map(|table| (**table).clone())
            .unwrap_or_default();

        let mut importer = SymbolImporter::new(&mut symbols, tables);
        for (other_id, table) in ordered_tables.iter().copied() {
            if other_id == file_id {
                continue;
            }
            importer.import_table(other_id, table);
        }

        let used_ids = collect_used_symbols(&symbols, &root);
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

    used
}
