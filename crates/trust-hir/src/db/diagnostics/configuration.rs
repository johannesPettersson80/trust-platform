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
        check_task_priority(symbols, &task, diagnostics);
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

fn check_task_priority(
    symbols: &SymbolTable,
    task: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
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
    for (field, range) in &fields.unknown_fields {
        diagnostics.error(
            DiagnosticCode::InvalidTaskConfig,
            *range,
            format!("TASK '{task_name}' has unknown initializer field '{field}'"),
        );
    }
    for (field, range) in &fields.duplicate_fields {
        diagnostics.error(
            DiagnosticCode::InvalidTaskConfig,
            *range,
            format!("TASK '{task_name}' has duplicate initializer field '{field}'"),
        );
    }
    if fields.priority_expr.is_none() {
        diagnostics.error(
            DiagnosticCode::InvalidTaskConfig,
            task_range,
            format!("TASK '{task_name}' requires PRIORITY in the task init"),
        );
        return;
    }

    if let Some(expr) = fields.priority_expr {
        if parse_unsigned_int_literal(&expr).is_none_or(|value| value > u64::from(u32::MAX)) {
            let expression = expr.text().to_string();
            let message = if expression.trim_start().starts_with('-') {
                format!("TASK PRIORITY must be non-negative (task '{task_name}')")
            } else if expression
                .replace('_', "")
                .trim()
                .chars()
                .all(|character| character.is_ascii_digit())
            {
                format!("TASK '{task_name}' PRIORITY is outside the supported u32 range")
            } else {
                format!("TASK '{task_name}' PRIORITY must be an unsigned integer literal")
            };
            diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                message,
            );
        }
    }

    if let Some(expr) = fields.single_expr {
        if !is_bool_storage_reference(symbols, &expr) {
            let detail = name_from_node(&expr)
                .filter(|(name, _)| symbols.lookup(name.as_str()).is_none())
                .map(|(name, _)| format!("references unknown variable '{name}'"))
                .unwrap_or_else(|| "must name a visible BOOL storage variable".to_owned());
            diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                format!("TASK '{task_name}' SINGLE {detail}"),
            );
        }
    }

    if let Some(expr) = fields.interval_expr {
        if !is_nonnegative_time_literal(&expr) {
            let detail = if expr.text().to_string().contains('-') {
                "must be non-negative"
            } else {
                "must be a TIME literal"
            };
            diagnostics.error(
                DiagnosticCode::InvalidTaskConfig,
                expr.text_range(),
                format!("TASK '{task_name}' INTERVAL {detail}"),
            );
        }
    }
}

#[derive(Default)]
struct TaskInitFields {
    priority_expr: Option<SyntaxNode>,
    single_expr: Option<SyntaxNode>,
    interval_expr: Option<SyntaxNode>,
    unknown_fields: Vec<(SmolStr, TextRange)>,
    duplicate_fields: Vec<(SmolStr, TextRange)>,
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

        let field = name.to_ascii_uppercase();
        let duplicate = match field.as_str() {
            "PRIORITY" => fields.priority_expr.is_some(),
            "SINGLE" => fields.single_expr.is_some(),
            "INTERVAL" => fields.interval_expr.is_some(),
            _ => {
                fields
                    .unknown_fields
                    .push((name.clone(), name_node.text_range()));
                false
            }
        };
        if duplicate {
            fields
                .duplicate_fields
                .push((name.clone(), name_node.text_range()));
        }

        if field == "PRIORITY" {
            fields.priority_expr = expr_node.clone();
        }
        if field == "SINGLE" {
            fields.single_expr = expr_node.clone();
        }
        if field == "INTERVAL" {
            fields.interval_expr = expr_node;
        }

        idx = j;
    }
    fields
}

fn parse_unsigned_int_literal(node: &SyntaxNode) -> Option<u64> {
    if node.kind() != SyntaxKind::Literal {
        return None;
    }
    let mut tokens = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia());
    let token = tokens.next()?;
    if token.kind() != SyntaxKind::IntLiteral || tokens.next().is_some() {
        return None;
    }
    let text = token.text().replace('_', "");
    text.parse::<u64>().ok()
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum LiteralKind {
    Bool,
    Time,
    Other,
}

#[cfg(test)]
fn literal_kind(node: &SyntaxNode) -> Option<LiteralKind> {
    if node.kind() != SyntaxKind::Literal {
        return None;
    }
    let mut saw_literal = false;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::KwTrue | SyntaxKind::KwFalse => return Some(LiteralKind::Bool),
            SyntaxKind::TimeLiteral => return Some(LiteralKind::Time),
            SyntaxKind::IntLiteral
            | SyntaxKind::RealLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::WideStringLiteral
            | SyntaxKind::DateLiteral
            | SyntaxKind::TimeOfDayLiteral
            | SyntaxKind::DateAndTimeLiteral => saw_literal = true,
            _ => {}
        }
    }
    saw_literal.then_some(LiteralKind::Other)
}

fn is_nonnegative_time_literal(node: &SyntaxNode) -> bool {
    if node.kind() != SyntaxKind::Literal {
        return false;
    }

    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::TimeLiteral)
        .is_some_and(|token| !token.text().contains('-'))
}

fn is_bool_storage_reference(symbols: &SymbolTable, node: &SyntaxNode) -> bool {
    if node.kind() != SyntaxKind::NameRef {
        return false;
    }
    let Some((name, _)) = name_from_node(node) else {
        return false;
    };
    let Some(symbol_id) = symbols.lookup(name.as_str()) else {
        return false;
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return false;
    };
    if !matches!(
        symbol.kind,
        SymbolKind::Variable {
            qualifier: VarQualifier::Global | VarQualifier::External
        }
    ) {
        return false;
    }
    matches!(
        symbols.type_by_id(symbols.resolve_alias_type(symbol.type_id)),
        Some(Type::Bool)
    )
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

#[cfg(test)]
#[path = "configuration/contract_tests.rs"]
mod contract_tests;
