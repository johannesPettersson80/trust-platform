use super::super::queries::*;
use super::super::*;
use trust_syntax::syntax::SyntaxElement;

pub(in crate::db) fn check_configuration_semantics(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for config in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Configuration)
    {
        check_scope_tasks_and_programs(symbols, &config, diagnostics);
    }

    for resource in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Resource)
    {
        check_scope_tasks_and_programs(symbols, &resource, diagnostics);
    }
}

fn check_scope_tasks_and_programs(
    symbols: &SymbolTable,
    scope: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    let tasks = collect_tasks_in_scope(scope);

    for task in scope
        .children()
        .filter(|node| node.kind() == SyntaxKind::TaskConfig)
    {
        check_task_priority(&task, diagnostics);
    }

    for program in scope
        .children()
        .filter(|node| node.kind() == SyntaxKind::ProgramConfig)
    {
        if let Some((task_name, range)) = program_config_task_name(&program) {
            let normalized = normalize_task_name(task_name.as_str());
            if !tasks.contains_key(&normalized) {
                diagnostics.error(
                    DiagnosticCode::UnknownTask,
                    range,
                    format!("unknown task '{task_name}'"),
                );
            }
        }

        if let Some((instance, type_parts)) = program_config_instance_and_type(&program) {
            match resolve_program_type(symbols, &type_parts) {
                ProgramTypeResolution::Program(_) => {}
                ProgramTypeResolution::WrongKind(symbol_name) => {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        range_for_program_name(&program).unwrap_or_else(|| program.text_range()),
                        format!("PROGRAM instance type '{symbol_name}' is not a PROGRAM"),
                    );
                }
                ProgramTypeResolution::Missing => {
                    diagnostics.error(
                        DiagnosticCode::UndefinedType,
                        range_for_program_name(&program).unwrap_or_else(|| program.text_range()),
                        format!("unknown program type for '{instance}'"),
                    );
                }
            }
        }
    }
}

pub(super) fn collect_tasks_in_scope(scope: &SyntaxNode) -> FxHashMap<SmolStr, TextRange> {
    let mut tasks = FxHashMap::default();
    for task in scope
        .children()
        .filter(|node| node.kind() == SyntaxKind::TaskConfig)
    {
        if let Some((name, range)) = name_from_node(&task) {
            tasks.insert(normalize_task_name(name.as_str()), range);
        }
    }
    tasks
}

fn check_task_priority(task: &SyntaxNode, diagnostics: &mut DiagnosticBuilder) {
    let Some((task_name, task_range)) = name_from_node(task) else {
        return;
    };
    let Some(task_init) = task
        .children()
        .find(|node| node.kind() == SyntaxKind::TaskInit)
    else {
        diagnostics.error(
            DiagnosticCode::InvalidTaskConfig,
            task_range,
            format!("TASK '{task_name}' requires PRIORITY in the task init"),
        );
        return;
    };

    let fields = task_init_fields(&task_init);
    if fields.priority_expr.is_none() {
        diagnostics.error(
            DiagnosticCode::InvalidTaskConfig,
            task_range,
            format!("TASK '{task_name}' requires PRIORITY in the task init"),
        );
        return;
    }

    if let Some(expr) = fields.priority_expr {
        if parse_unsigned_int_literal(&expr).is_none() {
            diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                format!("TASK '{task_name}' PRIORITY must be an unsigned integer literal"),
            );
        }
    }

    if let Some(expr) = fields.single_expr {
        match literal_kind(&expr) {
            Some(LiteralKind::Bool) => {}
            Some(_) => diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                format!("TASK '{task_name}' SINGLE must be a BOOL literal"),
            ),
            None => {}
        }
    }

    if let Some(expr) = fields.interval_expr {
        match literal_kind(&expr) {
            Some(LiteralKind::Time) => {}
            Some(_) => diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                format!("TASK '{task_name}' INTERVAL must be a TIME literal"),
            ),
            None => {}
        }
    }
}

#[derive(Default)]
struct TaskInitFields {
    priority_expr: Option<SyntaxNode>,
    single_expr: Option<SyntaxNode>,
    interval_expr: Option<SyntaxNode>,
}

fn task_init_fields(node: &SyntaxNode) -> TaskInitFields {
    let mut fields = TaskInitFields::default();
    let elements: Vec<SyntaxElement> = node.children_with_tokens().collect();
    let mut idx = 0;
    while idx < elements.len() {
        let Some(name_node) = elements[idx]
            .as_node()
            .filter(|node| node.kind() == SyntaxKind::Name)
        else {
            idx += 1;
            continue;
        };
        let Some(assign) = elements
            .get(idx + 1)
            .and_then(|element| element.as_token())
            .filter(|token| token.kind() == SyntaxKind::Assign)
        else {
            idx += 1;
            continue;
        };
        let _ = assign;
        let Some((name, _)) = name_from_node(name_node) else {
            idx += 1;
            continue;
        };
        let mut expr_node = None;
        let mut j = idx + 2;
        while j < elements.len() {
            if let Some(node) = elements[j].as_node() {
                expr_node = Some(node.clone());
                break;
            }
            if let Some(token) = elements[j].as_token() {
                if matches!(token.kind(), SyntaxKind::Comma | SyntaxKind::RParen) {
                    break;
                }
            }
            j += 1;
        }

        if name.eq_ignore_ascii_case("PRIORITY") {
            fields.priority_expr = expr_node.clone();
        }
        if name.eq_ignore_ascii_case("SINGLE") {
            fields.single_expr = expr_node.clone();
        }
        if name.eq_ignore_ascii_case("INTERVAL") {
            fields.interval_expr = expr_node;
        }

        idx = j;
    }
    fields
}

fn parse_unsigned_int_literal(node: &SyntaxNode) -> Option<u64> {
    let token = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::IntLiteral)?;
    let text = token.text().replace('_', "");
    text.parse::<u64>().ok()
}

#[derive(Clone, Copy)]
enum LiteralKind {
    Bool,
    Time,
    Other,
}

fn literal_kind(node: &SyntaxNode) -> Option<LiteralKind> {
    let mut saw_literal = false;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::KwTrue | SyntaxKind::KwFalse => {
                return Some(LiteralKind::Bool);
            }
            SyntaxKind::TimeLiteral => {
                return Some(LiteralKind::Time);
            }
            SyntaxKind::IntLiteral
            | SyntaxKind::RealLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::WideStringLiteral
            | SyntaxKind::DateLiteral
            | SyntaxKind::TimeOfDayLiteral
            | SyntaxKind::DateAndTimeLiteral => {
                saw_literal = true;
            }
            _ => {}
        }
    }
    if saw_literal {
        Some(LiteralKind::Other)
    } else {
        None
    }
}

pub(super) fn program_config_task_name(node: &SyntaxNode) -> Option<(SmolStr, TextRange)> {
    let elements: Vec<SyntaxElement> = node.children_with_tokens().collect();
    let mut idx = 0;
    while idx < elements.len() {
        if let Some(token) = elements[idx].as_token() {
            if token.kind() == SyntaxKind::KwWith {
                for element in elements.iter().skip(idx + 1) {
                    if let Some(name_node) = element
                        .as_node()
                        .filter(|node| node.kind() == SyntaxKind::Name)
                    {
                        return name_from_node(name_node);
                    }
                }
            }
        }
        idx += 1;
    }
    None
}

fn range_for_program_name(node: &SyntaxNode) -> Option<TextRange> {
    name_from_node(node).map(|(_, range)| range)
}

pub(super) enum ProgramTypeResolution {
    Program(SymbolId),
    WrongKind(SmolStr),
    Missing,
}

pub(super) fn resolve_program_type(
    symbols: &SymbolTable,
    parts: &[SmolStr],
) -> ProgramTypeResolution {
    let Some(symbol_id) = (if parts.len() == 1 {
        symbols.lookup(parts[0].as_str())
    } else {
        symbols.resolve_qualified(parts)
    }) else {
        return ProgramTypeResolution::Missing;
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return ProgramTypeResolution::Missing;
    };
    if matches!(symbol.kind, SymbolKind::Program) {
        ProgramTypeResolution::Program(symbol_id)
    } else {
        ProgramTypeResolution::WrongKind(symbol.name.clone())
    }
}

pub(super) fn normalize_task_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}
