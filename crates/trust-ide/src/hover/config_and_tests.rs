#[cfg(test)]
mod hover_docs_tests {
    use super::*;
    use trust_hir::db::{Database, FileId, SourceDatabase};

    #[test]
    fn test_hover_standard_function_doc() {
        let source = r#"
PROGRAM Main
VAR
    x : INT;
END_VAR
    x := AB|S(1);
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let result = hover(&db, file_id, TextSize::from(cursor as u32)).expect("hover result");
        assert!(result.contents.contains("IEC 61131-3"));
    }

    #[test]
    fn test_hover_typed_literal_doc() {
        let source = r#"
PROGRAM Main
VAR
    x : TIME;
END_VAR
    x := T#|1s;
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let result = hover(&db, file_id, TextSize::from(cursor as u32)).expect("hover result");
        assert!(result.contents.contains("Table 8"));
    }

    #[test]
    fn test_hover_namespace_using_info() {
        let source = r#"
NAMESPACE Lib
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
USING Lib;
VAR
    x : INT;
END_VAR
    x := Fo|o();
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let result = hover(&db, file_id, TextSize::from(cursor as u32)).expect("hover result");
        assert!(result.contents.contains("Namespace: Lib"));
        assert!(result.contents.contains("USING Lib"));
    }

    #[test]
    fn test_hover_namespace_ambiguity_info() {
        let source = r#"
NAMESPACE LibA
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

NAMESPACE LibB
FUNCTION Foo : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
USING LibA;
USING LibB;
VAR
    x : INT;
END_VAR
    x := Fo|o();
END_PROGRAM
"#;
        let cursor = source.find('|').expect("cursor");
        let mut cleaned = source.to_string();
        cleaned.remove(cursor);

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, cleaned);

        let result = hover(&db, file_id, TextSize::from(cursor as u32)).expect("hover result");
        assert!(result.contents.contains("Ambiguous reference"));
        assert!(result.contents.contains("LibA.Foo"));
        assert!(result.contents.contains("LibB.Foo"));
        assert!(result.contents.contains("USING"));
    }
}

fn qualified_names_in_clause(clause: &SyntaxNode) -> Vec<String> {
    let mut names = Vec::new();
    for node in clause
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::QualifiedName)
    {
        if let Some(name) = qualified_name_from_node(&node) {
            names.push(name);
        }
    }
    if !names.is_empty() {
        return names;
    }

    for node in clause
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Name)
    {
        if node
            .parent()
            .is_some_and(|parent| parent.kind() == SyntaxKind::QualifiedName)
        {
            continue;
        }
        if let Some(name) = qualified_name_from_node(&node) {
            names.push(name);
        }
    }

    names
}

fn qualified_name_from_node(node: &SyntaxNode) -> Option<String> {
    let target = match node.kind() {
        SyntaxKind::QualifiedName => node.clone(),
        SyntaxKind::Name => {
            if let Some(parent) = node.parent() {
                if parent.kind() == SyntaxKind::QualifiedName {
                    parent
                } else {
                    node.clone()
                }
            } else {
                node.clone()
            }
        }
        _ => return None,
    };

    match target.kind() {
        SyntaxKind::Name => ident_token_in_name(&target).map(|token| token.text().to_string()),
        SyntaxKind::QualifiedName => {
            let mut parts = Vec::new();
            for child in target.children().filter(|n| n.kind() == SyntaxKind::Name) {
                if let Some(ident) = ident_token_in_name(&child) {
                    parts.push(ident.text().to_string());
                }
            }
            (!parts.is_empty()).then_some(parts.join("."))
        }
        _ => None,
    }
}

fn find_named_node(
    root: &SyntaxNode,
    symbol_range: TextRange,
    kind: SyntaxKind,
) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == kind)
        .find(|node| name_range_from_node(node) == Some(symbol_range))
}

fn format_type_definition(symbols: &SymbolTable, symbol: &Symbol) -> String {
    let name = symbol.name.as_str();
    let Some(ty) = symbols.type_by_id(symbol.type_id) else {
        return format!("TYPE {}", name);
    };

    match ty {
        Type::Struct { fields, .. } => {
            let mut lines = Vec::new();
            lines.push(format!("TYPE {} :", name));
            lines.push("STRUCT".to_string());
            for field in fields.iter() {
                let field_type_name = format_type_ref(symbols, field.type_id);
                lines.push(format!("    {} : {};", field.name, field_type_name));
            }
            lines.push("END_STRUCT".to_string());
            lines.push("END_TYPE".to_string());
            lines.join("\n")
        }
        Type::Union { variants, .. } => {
            let mut lines = Vec::new();
            lines.push(format!("TYPE {} :", name));
            lines.push("UNION".to_string());
            for variant in variants.iter() {
                let field_type_name = format_type_ref(symbols, variant.type_id);
                lines.push(format!("    {} : {};", variant.name, field_type_name));
            }
            lines.push("END_UNION".to_string());
            lines.push("END_TYPE".to_string());
            lines.join("\n")
        }
        Type::Enum { values, .. } => {
            let mut lines = Vec::new();
            lines.push(format!("TYPE {} :", name));
            lines.push("(".to_string());
            for (idx, (value_name, value)) in values.iter().enumerate() {
                let mut line = format!("    {} := {}", value_name, value);
                if idx + 1 < values.len() {
                    line.push(',');
                }
                lines.push(line);
            }
            lines.push(");".to_string());
            lines.push("END_TYPE".to_string());
            lines.join("\n")
        }
        Type::Array {
            dimensions,
            element,
        } => {
            let dims: Vec<String> = dimensions
                .iter()
                .map(ArrayDimensionExt::display_bounds)
                .collect();
            let element_name = format_type_ref(symbols, *element);
            format!(
                "TYPE {} : ARRAY[{}] OF {};\nEND_TYPE",
                name,
                dims.join(", "),
                element_name
            )
        }
        Type::Pointer { target } => format!(
            "TYPE {} : POINTER TO {};\nEND_TYPE",
            name,
            format_type_ref(symbols, *target)
        ),
        Type::Reference { target } => format!(
            "TYPE {} : REF_TO {};\nEND_TYPE",
            name,
            format_type_ref(symbols, *target)
        ),
        Type::Subrange { base, lower, upper } => {
            let base_name = format_type_ref(symbols, *base);
            format!(
                "TYPE {} : {}({}..{});\nEND_TYPE",
                name, base_name, lower, upper
            )
        }
        Type::Alias { target, .. } => format!(
            "TYPE {} : {};\nEND_TYPE",
            name,
            format_type_ref(symbols, *target)
        ),
        _ => format!(
            "TYPE {} : {};\nEND_TYPE",
            name,
            format_type_ref(symbols, symbol.type_id)
        ),
    }
}

fn format_type_ref(symbols: &SymbolTable, type_id: TypeId) -> String {
    if let Some(name) = TypeId::builtin_name(type_id) {
        return name.to_string();
    }
    match symbols.type_by_id(type_id) {
        Some(Type::Array {
            dimensions,
            element,
        }) => {
            let dims: Vec<String> = dimensions
                .iter()
                .map(ArrayDimensionExt::display_bounds)
                .collect();
            format!(
                "ARRAY[{}] OF {}",
                dims.join(", "),
                format_type_ref(symbols, *element)
            )
        }
        Some(Type::Pointer { target }) => {
            format!("POINTER TO {}", format_type_ref(symbols, *target))
        }
        Some(Type::Reference { target }) => format!("REF_TO {}", format_type_ref(symbols, *target)),
        Some(Type::Subrange { base, lower, upper }) => {
            let base_name = format_type_ref(symbols, *base);
            format!("{}({}..{})", base_name, lower, upper)
        }
        Some(Type::Struct { name, .. })
        | Some(Type::Union { name, .. })
        | Some(Type::Enum { name, .. })
        | Some(Type::FunctionBlock { name })
        | Some(Type::Class { name })
        | Some(Type::Interface { name })
        | Some(Type::Alias { name, .. }) => name.to_string(),
        Some(other) => format_type(other),
        None => "?".to_string(),
    }
}

fn format_field(name: &str, type_name: &str) -> String {
    let mut result = String::new();
    result.push_str("```st\n");
    result.push_str(&format!("FIELD {} : {}", name, type_name));
    result.push_str("\n```");
    result
}

fn type_name_for_id(symbols: &SymbolTable, type_id: TypeId) -> Option<String> {
    if let Some(name) = TypeId::builtin_name(type_id) {
        return Some(name.to_string());
    }
    symbols
        .type_by_id(type_id)
        .map(|_| format_type_ref(symbols, type_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_type() {
        assert_eq!(format_type(&Type::Int), "INT");
        assert_eq!(format_type(&Type::Bool), "BOOL");
        assert_eq!(
            format_type(&Type::String { max_len: Some(80) }),
            "STRING[80]"
        );
    }
}
