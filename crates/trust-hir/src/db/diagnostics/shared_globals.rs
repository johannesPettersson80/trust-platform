use super::super::queries::*;
use super::super::*;
use super::configuration::{collect_tasks_in_scope, normalize_task_name, program_config_task_name};
use super::context::{
    expression_context, find_symbol_by_name_range, is_pou_kind, namespace_path_for_symbol,
    normalized_name,
};

const MAX_TASKS_IN_MESSAGE: usize = 3;
const MAX_RELATED_TASKS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TaskId(SmolStr);

#[derive(Debug, Clone)]
struct TaskInfo {
    label: SmolStr,
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

pub(in crate::db) fn check_shared_global_task_hazards(
    symbols: &SymbolTable,
    project_roots: &[(FileId, SyntaxNode)],
    file_id: FileId,
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

    let program_accesses = collect_program_accesses(symbols, project_roots, &globals_by_key);
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
                let Some(program_id) = resolve_program_type(symbols, &type_parts) else {
                    continue;
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
                    range: task_range,
                });
            }
        }
    }

    (program_tasks, task_info)
}

fn collect_program_accesses(
    symbols: &SymbolTable,
    roots: &[(FileId, SyntaxNode)],
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
) -> FxHashMap<SymbolId, ProgramAccess> {
    let mut accesses: FxHashMap<SymbolId, ProgramAccess> = FxHashMap::default();

    for (_file_id, root) in roots {
        for program in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Program)
        {
            let Some((name, range)) = name_from_node(&program) else {
                continue;
            };
            let Some(program_id) = find_symbol_by_name_range(symbols, name.as_str(), range) else {
                continue;
            };
            let mut access = ProgramAccess {
                reads: FxHashSet::default(),
                writes: FxHashSet::default(),
            };
            collect_pou_accesses(symbols, globals_by_key, &program, &mut access);
            if !access.reads.is_empty() || !access.writes.is_empty() {
                accesses.insert(program_id, access);
            }
        }
    }

    accesses
}

fn collect_pou_accesses(
    symbols: &SymbolTable,
    globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
    pou: &SyntaxNode,
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
        else {
            continue;
        };
        access.record(symbol_id, is_write_context(&node));
    }

    for node in pou
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NameRef)
    {
        if node
            .ancestors()
            .any(|ancestor| ancestor.kind() == SyntaxKind::FieldExpr)
        {
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
        access.record(symbol_id, is_write_context(&node));
    }
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
        .or_else(|| symbols.lookup_any(name.as_str()))?;
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
                .or_else(|| symbols.lookup_any(parts[0].as_str()))
        } else {
            symbols.resolve_qualified(&parts[..len])
        };
        if let Some(symbol_id) = symbol_id {
            if let Some(global_id) = resolve_global_symbol(symbols, globals_by_key, symbol_id) {
                return Some(global_id);
            }
        }
    }
    None
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

fn is_write_context(expr: &SyntaxNode) -> bool {
    let mut current = expr.clone();
    while let Some(parent) = current.parent() {
        if parent.kind() == SyntaxKind::AssignStmt {
            if let Some(first_child) = parent.first_child() {
                return first_child.text_range() == current.text_range();
            }
            return false;
        }
        if matches!(
            parent.kind(),
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr | SyntaxKind::DerefExpr
        ) {
            current = parent;
            continue;
        }
        break;
    }
    false
}

fn resolve_program_type(symbols: &SymbolTable, parts: &[SmolStr]) -> Option<SymbolId> {
    let symbol_id = symbols.resolve_qualified(parts).or_else(|| {
        if parts.len() == 1 {
            symbols.lookup_any(parts[0].as_str())
        } else {
            None
        }
    })?;
    symbols
        .get(symbol_id)
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Program))?;
    Some(symbol_id)
}
