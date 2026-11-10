use super::super::queries::*;
use super::super::*;
use super::configuration::{
    collect_tasks_in_scope, normalize_task_name, program_config_task_name, resolve_program_type,
    ProgramTypeResolution,
};
use super::context::{
    expression_context, find_symbol_by_name_range, is_pou_kind, namespace_path_for_symbol,
    normalized_name,
};
use crate::access::{access_of, Access};
use crate::attributes::declares_attribute;

const MAX_TASKS_IN_MESSAGE: usize = 3;
const MAX_RELATED_TASKS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TaskId(SmolStr);

#[derive(Debug, Clone)]
struct TaskInfo {
    label: SmolStr,
    /// The task's name as declared, without its configuration's and resource's.
    name: SmolStr,
    range: TextRange,
}

#[derive(Debug, Clone)]
struct ProgramAccess {
    reads: FxHashSet<SymbolId>,
    writes: FxHashSet<SymbolId>,
}

impl ProgramAccess {
    fn record(&mut self, symbol_id: SymbolId, is_write: bool) {
        if is_write {
            self.writes.insert(symbol_id);
        } else {
            self.reads.insert(symbol_id);
        }
    }
}

#[derive(Debug, Clone)]
struct GlobalUsage {
    reads: FxHashSet<TaskId>,
    writes: FxHashSet<TaskId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GlobalKey {
    namespace: Vec<SmolStr>,
    name: SmolStr,
}

#[derive(Debug, Clone)]
struct GlobalCandidate {
    symbol_id: SymbolId,
    origin: SymbolOrigin,
    range: TextRange,
    name: SmolStr,
}

/// W012 for the globals that `file_id` declares (specification 28 §6). `written_arguments` gives a
/// file's argument expressions that its calls bind to output and in-out parameters
/// (`FileAnalysis::written_arguments`), from which `crate::access` tells a write.
pub(in crate::db) fn check_shared_global_task_hazards(
    symbols: &SymbolTable,
    project_roots: &[(FileId, SyntaxNode)],
    file_id: FileId,
    written_arguments: &dyn Fn(FileId) -> Arc<FxHashSet<(u32, u32)>>,
    call_facts: &dyn Fn(FileId) -> Option<CallFacts>,
    diagnostics: &mut DiagnosticBuilder,
) {
    let (globals_by_key, globals_by_id) = collect_global_candidates(symbols, file_id);
    if globals_by_id.is_empty() {
        return;
    }

    let (program_tasks, task_info) = collect_program_task_assignments(symbols, project_roots);
    if program_tasks.is_empty() {
        return;
    }

    let program_accesses = collect_program_reach(
        symbols,
        project_roots,
        &globals_by_key,
        written_arguments,
        call_facts,
    )
    .accesses;
    if program_accesses.is_empty() {
        return;
    }

    let usage_by_global = collect_global_usage(&program_tasks, &program_accesses);

    for (global_id, usage) in usage_by_global {
        if usage.writes.is_empty() {
            continue;
        }

        let mut access_tasks = usage.reads.clone();
        access_tasks.extend(usage.writes.iter().cloned());
        if access_tasks.len() <= 1 {
            continue;
        }

        let Some(global) = globals_by_id.get(&global_id) else {
            continue;
        };
        if global.origin.file_id != file_id {
            continue;
        }
        // One task writing a value that one access reads and writes is how tasks share data
        // safely; several writers, or data that one access does not cover, are not (as CODESYS
        // Static Analysis SA0006 and SA0103 judge them).
        let data = symbols
            .get(global.symbol_id)
            .map_or(DataKind::Unknown, |symbol| {
                data_kind(symbols, symbol.type_id)
            });
        if usage.writes.len() == 1 && data != DataKind::NonAtomic {
            continue;
        }
        if declared_cross_task(project_roots, global) {
            continue;
        }

        let access_list = format_task_list(&access_tasks, &task_info);
        let write_list = format_task_list(&usage.writes, &task_info);
        let message = format!(
            "shared global '{}' accessed by multiple tasks ({access_list}) with writes in ({write_list})",
            global.name
        );
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::SharedGlobalTaskHazard,
            global.range,
            message,
        );

        for task_id in usage.writes.iter().take(MAX_RELATED_TASKS) {
            if let Some(info) = task_info.get(task_id) {
                diagnostic = diagnostic.with_related(
                    info.range,
                    format!("TASK '{}' participates in shared writes", info.label),
                );
            }
        }

        diagnostics.add(diagnostic);
    }
}

/// The owner of each global that `file_id` declares (control-studio-runtime-protocol annex E2.3),
/// by its name with its namespaces, in that order: the one task whose programs are the only ones
/// that write it, through their own bodies or through the POUs their calls reach (`reach`). Programs
/// of several tasks, or of a task and none, make it shared, and so does a POU that no program
/// reaches through a resolved call, such as a method an interface's call may run. A program that no
/// configuration runs writes nothing that counts; in a project without a configuration every
/// program runs outside every task (specification 18 §2.2).
pub(in crate::db) fn shared_global_owners(
    symbols: &SymbolTable,
    project_roots: &[(FileId, SyntaxNode)],
    file_id: FileId,
    written_arguments: &dyn Fn(FileId) -> Arc<FxHashSet<(u32, u32)>>,
    call_facts: &dyn Fn(FileId) -> Option<CallFacts>,
) -> Vec<GlobalOwnership> {
    let (globals_by_key, globals_by_id) = collect_global_candidates(symbols, file_id);
    let mut declared: Vec<&GlobalCandidate> = globals_by_id
        .values()
        .filter(|global| global.origin.file_id == file_id)
        .collect();
    if declared.is_empty() {
        return Vec::new();
    }
    declared.sort_by_key(|global| global.range.start());
    let (program_tasks, task_info) = collect_program_task_assignments(symbols, project_roots);
    let background = collect_background_programs(symbols, project_roots);
    let ProgramReach {
        accesses: program_accesses,
        unreached_writes: elsewhere,
    } = collect_program_reach(
        symbols,
        project_roots,
        &globals_by_key,
        written_arguments,
        call_facts,
    );
    declared
        .into_iter()
        .map(|global| {
            let mut tasks: Vec<&TaskId> = Vec::new();
            let mut outside_every_task = false;
            for (program, access) in &program_accesses {
                if !access.writes.contains(&global.symbol_id) {
                    continue;
                }
                for task in program_tasks.get(program).into_iter().flatten() {
                    if !tasks.contains(&task) {
                        tasks.push(task);
                    }
                }
                outside_every_task |= background.contains(program);
            }
            let owner = if elsewhere.contains(&global.symbol_id) {
                GlobalOwner::Shared
            } else {
                match (tasks.as_slice(), outside_every_task) {
                    ([], false) => GlobalOwner::Unwritten,
                    ([], true) => GlobalOwner::Background,
                    ([task], false) => GlobalOwner::Task(
                        task_info
                            .get(*task)
                            .map_or_else(|| task.0.clone(), |info| info.name.clone()),
                    ),
                    _ => GlobalOwner::Shared,
                }
            };
            let mut name = namespace_path_for_symbol(symbols, global.symbol_id)
                .iter()
                .map(SmolStr::as_str)
                .collect::<Vec<_>>()
                .join(".");
            if !name.is_empty() {
                name.push('.');
            }
            name.push_str(global.name.as_str());
            GlobalOwnership {
                name: SmolStr::new(name),
                owner,
            }
        })
        .collect()
}

/// The programs that run outside every task: each that a configuration's program declaration
/// gives no task, and every program of a project without a configuration (specification 18
/// §2.2).
fn collect_background_programs(
    symbols: &SymbolTable,
    roots: &[(FileId, SyntaxNode)],
) -> FxHashSet<SymbolId> {
    let mut background = FxHashSet::default();
    let configured = roots.iter().any(|(_, root)| {
        root.descendants()
            .any(|node| node.kind() == SyntaxKind::Configuration)
    });
    for (file_id, root) in roots {
        if configured {
            for program in root.descendants().filter(|node| {
                node.kind() == SyntaxKind::ProgramConfig && program_config_task_name(node).is_none()
            }) {
                let Some((_, type_parts)) = program_config_instance_and_type(&program) else {
                    continue;
                };
                if let ProgramTypeResolution::Program(program_id) =
                    resolve_program_type(symbols, &type_parts)
                {
                    background.insert(program_id);
                }
            }
        } else {
            for program in root
                .descendants()
                .filter(|node| node.kind() == SyntaxKind::Program)
            {
                let Some((name, range)) = name_from_node(&program) else {
                    continue;
                };
                if let Some(program_id) = find_symbol_by_name_range(symbols, name.as_str(), range)
                    .or_else(|| {
                        find_imported_symbol_by_name_range(symbols, *file_id, name.as_str(), range)
                    })
                {
                    background.insert(program_id);
                }
            }
        }
    }
    background
}

/// How a global's value is read and written (specification 28 section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataKind {
    /// One access on every truST target: an elementary value of 32 bits or less, or an
    /// enumeration or subrange of one.
    Atomic,
    /// More than one access on some target: strings, arrays, structures, unions, function block
    /// and class instances and the interfaces that designate them, and 64-bit values. truST also
    /// runs on 32-bit chips, where a 64-bit access takes two, and it stores TIME, DATE, TOD and DT
    /// in eight bytes (specification 02).
    NonAtomic,
    /// The type did not resolve, and nothing is guessed from it (specification 28 section 9).
    Unknown,
}

/// A pointer or reference counts as its target: the tasks share the data they reach through it.
fn data_kind(symbols: &SymbolTable, type_id: TypeId) -> DataKind {
    match symbols.type_by_id(symbols.resolve_alias_type(type_id)) {
        Some(
            Type::Bool
            | Type::SInt
            | Type::Int
            | Type::DInt
            | Type::USInt
            | Type::UInt
            | Type::UDInt
            | Type::Real
            | Type::Byte
            | Type::Word
            | Type::DWord
            | Type::Char
            | Type::WChar,
        ) => DataKind::Atomic,
        Some(Type::Enum { base, .. } | Type::Subrange { base, .. }) => data_kind(symbols, *base),
        Some(Type::Pointer { target } | Type::Reference { target }) => data_kind(symbols, *target),
        Some(Type::Unknown) | None => DataKind::Unknown,
        Some(_) => DataKind::NonAtomic,
    }
}

/// Whether the global's declaration, or the `VAR_GLOBAL` list that holds it, carries
/// `{attribute 'cross_task'}`: the project declares that the global is shared between tasks by
/// design, and owns the protocol (specification 28 section 6).
fn declared_cross_task(roots: &[(FileId, SyntaxNode)], global: &GlobalCandidate) -> bool {
    let Some((_, root)) = roots
        .iter()
        .find(|(file_id, _)| *file_id == global.origin.file_id)
    else {
        return false;
    };
    if !root.text_range().contains_range(global.range) {
        return false;
    }
    let node = match root.covering_element(global.range) {
        rowan::NodeOrToken::Node(node) => Some(node),
        rowan::NodeOrToken::Token(token) => token.parent(),
    };
    let Some(declaration) =
        node.and_then(|node| node.ancestors().find(|n| n.kind() == SyntaxKind::VarDecl))
    else {
        return false;
    };
    declares_attribute(&declaration, "cross_task")
        || declaration
            .ancestors()
            .find(|n| n.kind() == SyntaxKind::VarBlock)
            .is_some_and(|list| declares_attribute(&list, "cross_task"))
}

fn collect_global_candidates(
    symbols: &SymbolTable,
    file_id: FileId,
) -> (
    FxHashMap<GlobalKey, GlobalCandidate>,
    FxHashMap<SymbolId, GlobalCandidate>,
) {
    let mut by_key = FxHashMap::default();
    let mut by_id = FxHashMap::default();

    for symbol in symbols.iter() {
        if !matches!(
            symbol.kind,
            SymbolKind::Variable {
                qualifier: VarQualifier::Global
            }
        ) {
            continue;
        }

        let origin = symbol.origin.unwrap_or(SymbolOrigin {
            file_id,
            symbol_id: symbol.id,
        });
        let key = GlobalKey {
            namespace: namespace_path_for_symbol(symbols, symbol.id),
            name: normalized_name(symbol.name.as_str()),
        };
        let candidate = GlobalCandidate {
            symbol_id: symbol.id,
            origin,
            range: symbol.range,
            name: symbol.name.clone(),
        };
        by_key.insert(key, candidate.clone());
        by_id.insert(symbol.id, candidate);
    }

    (by_key, by_id)
}

fn collect_program_task_assignments(
    symbols: &SymbolTable,
    roots: &[(FileId, SyntaxNode)],
) -> (
    FxHashMap<SymbolId, FxHashSet<TaskId>>,
    FxHashMap<TaskId, TaskInfo>,
) {
    let mut program_tasks: FxHashMap<SymbolId, FxHashSet<TaskId>> = FxHashMap::default();
    let mut task_info: FxHashMap<TaskId, TaskInfo> = FxHashMap::default();

    for (_file_id, root) in roots {
        for scope in root.descendants().filter(|node| {
            matches!(
                node.kind(),
                SyntaxKind::Configuration | SyntaxKind::Resource
            )
        }) {
            let tasks = collect_tasks_in_scope(&scope);
            if tasks.is_empty() {
                continue;
            }
            let (config_name, resource_name) = scope_labels(&scope);

            for program in scope
                .children()
                .filter(|node| node.kind() == SyntaxKind::ProgramConfig)
            {
                let Some((task_name, _)) = program_config_task_name(&program) else {
                    continue;
                };
                let normalized_task = normalize_task_name(task_name.as_str());
                let Some(task_range) = tasks.get(&normalized_task).copied() else {
                    continue;
                };
                let Some((_, type_parts)) = program_config_instance_and_type(&program) else {
                    continue;
                };
                let program_id = match resolve_program_type(symbols, &type_parts) {
                    ProgramTypeResolution::Program(program_id) => program_id,
                    ProgramTypeResolution::WrongKind(_) | ProgramTypeResolution::Missing => {
                        continue;
                    }
                };

                let (task_id, task_label) = task_id_and_label(
                    &normalized_task,
                    task_name.as_str(),
                    config_name.as_ref(),
                    resource_name.as_ref(),
                );
                program_tasks
                    .entry(program_id)
                    .or_default()
                    .insert(task_id.clone());
                task_info.entry(task_id).or_insert(TaskInfo {
                    label: task_label,
                    name: task_name.clone(),
                    range: task_range,
                });
            }
        }
    }

    (program_tasks, task_info)
}

/// Each program's accesses to the globals, with those of every POU it reaches through its calls,
/// and the globals that POUs no program reaches write (specification 28 §6; `reach`).
struct ProgramReach {
    accesses: FxHashMap<SymbolId, ProgramAccess>,
    unreached_writes: FxHashSet<SymbolId>,
}

fn collect_program_reach(
    symbols: &SymbolTable,
    roots: &[(FileId, SyntaxNode)],
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    written_arguments: &dyn Fn(FileId) -> Arc<FxHashSet<(u32, u32)>>,
    call_facts: &dyn Fn(FileId) -> Option<CallFacts>,
) -> ProgramReach {
    let graph = CallGraph::build(
        symbols,
        roots,
        globals_by_key,
        written_arguments,
        call_facts,
    );
    let mut accesses: FxHashMap<SymbolId, ProgramAccess> = FxHashMap::default();
    let mut reached_by_programs = FxHashSet::default();
    for (file_id, root) in roots {
        for program in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Program)
        {
            let Some((name, range)) = name_from_node(&program) else {
                continue;
            };
            let Some(program_id) = find_symbol_by_name_range(symbols, name.as_str(), range)
                .or_else(|| {
                    find_imported_symbol_by_name_range(symbols, *file_id, name.as_str(), range)
                })
            else {
                continue;
            };
            let Some(key) = reach::pou_key(*file_id, &program) else {
                continue;
            };
            let reached = graph.reached(key);
            let access = graph.access_of(&reached);
            reached_by_programs.extend(reached);
            if !access.reads.is_empty() || !access.writes.is_empty() {
                accesses.insert(program_id, access);
            }
        }
    }
    ProgramReach {
        accesses,
        unreached_writes: graph.writes_outside(&reached_by_programs),
    }
}

fn find_imported_symbol_by_name_range(
    symbols: &SymbolTable,
    file_id: FileId,
    name: &str,
    range: TextRange,
) -> Option<SymbolId> {
    symbols
        .iter()
        .find(|symbol| {
            symbol
                .origin
                .is_some_and(|origin| origin.file_id == file_id)
                && symbol.name.eq_ignore_ascii_case(name)
                && symbol.range == range
        })
        .map(|symbol| symbol.id)
}

fn collect_pou_accesses(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    pou: &SyntaxNode,
    written: &FxHashSet<(u32, u32)>,
    access: &mut ProgramAccess,
) {
    for node in pou
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        if is_nested_field_expr(&node) || !belongs_to_pou(&node, pou) {
            continue;
        }
        let scope_id = expression_context(symbols, &node).scope_id;
        let Some(symbol_id) = resolve_field_expr_global(symbols, globals_by_key, &node, scope_id)
            .or_else(|| resolve_path_root_global(symbols, globals_by_key, &node, scope_id))
        else {
            continue;
        };
        access.record(symbol_id, writes(&node, written));
    }

    for node in pou
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NameRef)
    {
        if is_in_field_path(&node) {
            continue;
        }
        if !belongs_to_pou(&node, pou) {
            continue;
        }
        let scope_id = expression_context(symbols, &node).scope_id;
        let Some(symbol_id) = resolve_name_ref_global(symbols, globals_by_key, &node, scope_id)
        else {
            continue;
        };
        access.record(symbol_id, writes(&node, written));
    }

    // A FOR loop's control variable is a name of the loop's own, which the loop assigns.
    for node in pou.descendants().filter(|n| {
        n.kind() == SyntaxKind::Name && n.parent().is_some_and(|p| p.kind() == SyntaxKind::ForStmt)
    }) {
        if !belongs_to_pou(&node, pou) {
            continue;
        }
        let scope_id = expression_context(symbols, &node).scope_id;
        let Some(symbol_id) = resolve_name_ref_global(symbols, globals_by_key, &node, scope_id)
        else {
            continue;
        };
        access.record(symbol_id, writes(&node, written));
    }
}

/// Whether a use writes the global it names (specification 28 §6): it writes it (`crate::access`),
/// or it calls it, a call of the global instance or of a method of it, which may change it.
fn writes(node: &SyntaxNode, written: &FxHashSet<(u32, u32)>) -> bool {
    matches!(access_of(node, written), Access::Write | Access::Call)
}

fn collect_global_usage(
    program_tasks: &FxHashMap<SymbolId, FxHashSet<TaskId>>,
    program_accesses: &FxHashMap<SymbolId, ProgramAccess>,
) -> FxHashMap<SymbolId, GlobalUsage> {
    let mut usage: FxHashMap<SymbolId, GlobalUsage> = FxHashMap::default();

    for (program_id, tasks) in program_tasks {
        let Some(access) = program_accesses.get(program_id) else {
            continue;
        };
        for symbol_id in access.reads.iter() {
            let entry = usage.entry(*symbol_id).or_insert_with(|| GlobalUsage {
                reads: FxHashSet::default(),
                writes: FxHashSet::default(),
            });
            entry.reads.extend(tasks.iter().cloned());
        }
        for symbol_id in access.writes.iter() {
            let entry = usage.entry(*symbol_id).or_insert_with(|| GlobalUsage {
                reads: FxHashSet::default(),
                writes: FxHashSet::default(),
            });
            entry.writes.extend(tasks.iter().cloned());
        }
    }

    usage
}

fn resolve_name_ref_global(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    node: &SyntaxNode,
    scope_id: ScopeId,
) -> Option<SymbolId> {
    let name = name_from_name_ref(node)?;
    let symbol_id = symbols
        .resolve(name.as_str(), scope_id)
        .or_else(|| symbols.lookup(name.as_str()))?;
    resolve_global_symbol(symbols, globals_by_key, symbol_id)
}

fn resolve_field_expr_global(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    node: &SyntaxNode,
    scope_id: ScopeId,
) -> Option<SymbolId> {
    let parts = qualified_name_from_field_expr(node)?;
    for len in (1..=parts.len()).rev() {
        let symbol_id = if len == 1 {
            symbols
                .resolve(parts[0].as_str(), scope_id)
                .or_else(|| symbols.lookup(parts[0].as_str()))
        } else {
            symbols.resolve_qualified(&parts[..len])
        };
        if let Some(symbol_id) = symbol_id
            && let Some(global_id) = resolve_global_symbol(symbols, globals_by_key, symbol_id)
        {
            return Some(global_id);
        }
    }
    None
}

/// The global at the root of a field path whose base is not a plain or qualified name, such as
/// `Ref^.field`, `Arr[0].field` or `(Ref^).field`: specification 28 §6 counts a use "through a
/// field, an array element or a dereference of the global" as a use of the global. The base is
/// followed through dereferences, parentheses, indexes and fields to the name at its root.
fn resolve_path_root_global(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    field_expr: &SyntaxNode,
    scope_id: ScopeId,
) -> Option<SymbolId> {
    let mut current = field_expr.first_child()?;
    loop {
        match current.kind() {
            SyntaxKind::NameRef => {
                return resolve_name_ref_global(symbols, globals_by_key, &current, scope_id);
            }
            SyntaxKind::FieldExpr => {
                if let Some(global) =
                    resolve_field_expr_global(symbols, globals_by_key, &current, scope_id)
                {
                    return Some(global);
                }
                current = current.first_child()?;
            }
            SyntaxKind::DerefExpr | SyntaxKind::IndexExpr | SyntaxKind::ParenExpr => {
                current = current.first_child()?;
            }
            _ => return None,
        }
    }
}

/// Whether a name is part of a field path, its root or one of its members, which the field loop
/// resolves as a whole. A name inside an index of such a path is a use of its own: an index is
/// read (specification 28 §6).
fn is_in_field_path(name_ref: &SyntaxNode) -> bool {
    let mut current = name_ref.clone();
    while let Some(parent) = current.parent() {
        match parent.kind() {
            SyntaxKind::FieldExpr => return true,
            SyntaxKind::DerefExpr | SyntaxKind::ParenExpr => {}
            SyntaxKind::IndexExpr if parent.first_child().as_ref() == Some(&current) => {}
            _ => return false,
        }
        current = parent;
    }
    false
}

fn resolve_global_symbol(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    symbol_id: SymbolId,
) -> Option<SymbolId> {
    let symbol = symbols.get(symbol_id)?;
    match symbol.kind {
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        } => Some(symbol_id),
        SymbolKind::Variable {
            qualifier: VarQualifier::External,
        } => {
            let key = GlobalKey {
                namespace: namespace_path_for_symbol(symbols, symbol_id),
                name: normalized_name(symbol.name.as_str()),
            };
            globals_by_key
                .get(&key)
                .map(|candidate| candidate.symbol_id)
        }
        _ => None,
    }
}

fn scope_labels(scope: &SyntaxNode) -> (Option<SmolStr>, Option<SmolStr>) {
    match scope.kind() {
        SyntaxKind::Configuration => {
            let config_name = name_from_node(scope).map(|(name, _)| name);
            (config_name, None)
        }
        SyntaxKind::Resource => {
            let resource_name = name_from_node(scope).map(|(name, _)| name);
            let config_name = scope
                .ancestors()
                .find(|node| node.kind() == SyntaxKind::Configuration)
                .and_then(|node| name_from_node(&node).map(|(name, _)| name));
            (config_name, resource_name)
        }
        _ => (None, None),
    }
}

fn task_id_and_label(
    normalized_task: &SmolStr,
    task_name: &str,
    config_name: Option<&SmolStr>,
    resource_name: Option<&SmolStr>,
) -> (TaskId, SmolStr) {
    let mut id_parts: Vec<String> = Vec::new();
    let mut label_parts: Vec<String> = Vec::new();

    if let Some(config) = config_name {
        id_parts.push(config.as_str().to_ascii_uppercase());
        label_parts.push(config.to_string());
    }
    if let Some(resource) = resource_name {
        id_parts.push(resource.as_str().to_ascii_uppercase());
        label_parts.push(resource.to_string());
    }

    id_parts.push(normalized_task.as_str().to_string());
    label_parts.push(task_name.to_string());

    let id = SmolStr::new(id_parts.join("/"));
    let label = SmolStr::new(label_parts.join("/"));
    (TaskId(id), label)
}

fn format_task_list(tasks: &FxHashSet<TaskId>, info: &FxHashMap<TaskId, TaskInfo>) -> String {
    let mut labels: Vec<String> = tasks
        .iter()
        .map(|task_id| {
            info.get(task_id)
                .map(|entry| entry.label.to_string())
                .unwrap_or_else(|| task_id.0.to_string())
        })
        .collect();
    labels.sort();
    if labels.len() > MAX_TASKS_IN_MESSAGE {
        let remaining = labels.len() - MAX_TASKS_IN_MESSAGE;
        labels.truncate(MAX_TASKS_IN_MESSAGE);
        labels.push(format!("+{remaining} more"));
    }
    labels.join(", ")
}

fn qualified_name_from_field_expr(node: &SyntaxNode) -> Option<Vec<SmolStr>> {
    if node.kind() != SyntaxKind::FieldExpr {
        return None;
    }
    let mut parts: Vec<SmolStr> = Vec::new();
    let mut current = node.clone();
    loop {
        let mut children = current.children();
        let base = children.next()?;
        let member = children.next()?;
        let member_name = name_from_name_ref(&member)?;
        parts.push(member_name);
        match base.kind() {
            SyntaxKind::FieldExpr => {
                current = base;
            }
            SyntaxKind::NameRef => {
                let base_name = name_from_name_ref(&base)?;
                parts.push(base_name);
                break;
            }
            _ => return None,
        }
    }
    parts.reverse();
    Some(parts)
}

fn name_from_name_ref(node: &SyntaxNode) -> Option<SmolStr> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| SmolStr::new(t.text()))
}

fn is_nested_field_expr(node: &SyntaxNode) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == SyntaxKind::FieldExpr)
}

fn belongs_to_pou(node: &SyntaxNode, pou: &SyntaxNode) -> bool {
    node.ancestors()
        .find(|ancestor| is_pou_kind(ancestor.kind()))
        .map(|ancestor| ancestor == *pou)
        .unwrap_or(false)
}

#[path = "shared_globals/reach.rs"]
mod reach;
pub(in crate::db) use reach::CallFacts;
use reach::CallGraph;

#[cfg(test)]
#[path = "shared_globals/contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "shared_globals/owner_tests.rs"]
mod owner_tests;
