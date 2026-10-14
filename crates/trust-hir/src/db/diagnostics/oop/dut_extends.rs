use super::super::super::queries::*;
use super::super::super::*;
use super::super::context::find_symbol_by_name_range;

/// CODESYS DUT inheritance (`TYPE x EXTENDS y`, vendor extension): validate
/// that the base is a STRUCT type, reject cycles, and reject field shadowing.
pub(in crate::db) fn check_dut_extends_semantics(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for type_decl in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::TypeDecl)
    {
        let Some(clause) = type_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::ExtendsClause)
        else {
            continue;
        };
        let clause_range = qualified_name_parts(&clause)
            .map(|(_, range)| range)
            .unwrap_or_else(|| clause.text_range());

        let Some((type_name, name_range)) = type_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|child| name_from_node(&child))
        else {
            continue;
        };
        let Some(symbol_id) = find_symbol_by_name_range(symbols, type_name.as_str(), name_range)
        else {
            continue;
        };

        let has_struct_body = type_decl
            .children()
            .any(|child| child.kind() == SyntaxKind::StructDef);
        if !has_struct_body {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                clause_range,
                "EXTENDS is only supported on STRUCT type declarations",
            );
            continue;
        }

        let Some((parts, _)) = qualified_name_parts(&clause) else {
            continue;
        };
        let names: Vec<SmolStr> = parts.iter().map(|(name, _)| name.clone()).collect();
        let base_name = qualified_name_string(&names);

        // An unresolvable base is not reported anywhere else: the EXTENDS
        // clause is the only place the name appears, so diagnose it here.
        let Some(base_id) = symbols.resolve_global_or_qualified_name(&base_name) else {
            diagnostics.error(
                DiagnosticCode::UndefinedType,
                clause_range,
                format!("cannot resolve type '{base_name}'"),
            );
            continue;
        };
        let Some(base_symbol) = symbols.get(base_id) else {
            continue;
        };
        if !matches!(base_symbol.kind, SymbolKind::Type) {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                clause_range,
                format!(
                    "EXTENDS base '{}' is not a structured type",
                    base_symbol.name
                ),
            );
            continue;
        }
        let base_type = symbols.resolve_alias_type(base_symbol.type_id);
        if !matches!(symbols.type_by_id(base_type), Some(Type::Struct { .. })) {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                clause_range,
                format!("EXTENDS base '{}' is not a STRUCT type", base_symbol.name),
            );
            continue;
        }

        if type_inheritance_cycle(symbols, symbol_id) {
            diagnostics.error(
                DiagnosticCode::CyclicDependency,
                clause_range,
                "type inheritance cycle detected",
            );
            continue;
        }

        let Some(symbol) = symbols.get(symbol_id) else {
            continue;
        };
        let derived_type = symbols.resolve_alias_type(symbol.type_id);
        let Some(Type::Struct { fields, .. }) = symbols.type_by_id(derived_type) else {
            continue;
        };
        for field in fields {
            if let Some(source_name) = find_inherited_field_owner(symbols, symbol_id, &field.name) {
                diagnostics.error(
                    DiagnosticCode::DuplicateDeclaration,
                    name_range,
                    format!(
                        "field '{}' redeclares inherited field from '{}'",
                        field.name, source_name
                    ),
                );
            }
        }
    }
}

fn type_inheritance_cycle(symbols: &SymbolTable, start: SymbolId) -> bool {
    let mut visited: Vec<SymbolId> = Vec::new();
    let mut current = start;
    loop {
        if visited.contains(&current) {
            return true;
        }
        visited.push(current);
        let Some(parent_name) = symbols.extends_name(current).cloned() else {
            return false;
        };
        let Some(parent_id) = symbols.resolve_global_or_qualified_name(parent_name.as_str()) else {
            return false;
        };
        current = parent_id;
    }
}

/// Returns the declaring type's name if `field_name` is declared anywhere in
/// the inheritance chain above `derived_id`.
fn find_inherited_field_owner(
    symbols: &SymbolTable,
    derived_id: SymbolId,
    field_name: &str,
) -> Option<SmolStr> {
    let mut visited: Vec<SymbolId> = Vec::new();
    let mut current = derived_id;
    loop {
        if visited.contains(&current) {
            return None;
        }
        visited.push(current);
        let parent_name = symbols.extends_name(current).cloned()?;
        let parent_id = symbols.resolve_global_or_qualified_name(parent_name.as_str())?;
        let parent_symbol = symbols.get(parent_id)?;
        let parent_type = symbols.resolve_alias_type(parent_symbol.type_id);
        if let Some(Type::Struct { fields, .. }) = symbols.type_by_id(parent_type) {
            if fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(field_name))
            {
                return Some(parent_symbol.name.clone());
            }
        }
        current = parent_id;
    }
}
