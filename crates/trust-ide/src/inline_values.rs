use rustc_hash::FxHashSet;
use text_size::TextRange;

use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{Symbol, VarQualifier};
use trust_hir::{Database, SourceDatabase, SymbolKind};
use trust_syntax::parser::parse;
use trust_syntax::syntax::SyntaxKind;

use crate::util::{resolve_target_at_position_with_context, symbol_is_constant, ResolvedTarget};
use crate::var_decl::{
    find_var_decl_for_range, initializer_from_var_decl, var_decl_info_for_symbol,
};

/// Inline value hint for a symbol reference.
#[derive(Debug, Clone)]
pub struct InlineValueHint {
    /// Range of the symbol reference in the source document.
    pub range: TextRange,
    /// Text to display inline (typically includes a leading space).
    pub text: String,
}

/// Inline value targets that can be populated from runtime values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineValueTarget {
    /// Range of the symbol reference in the source document.
    pub range: TextRange,
    /// Name of the referenced symbol.
    pub name: smol_str::SmolStr,
    /// Runtime scope to query for the value.
    pub scope: InlineValueScope,
    /// Owning POU type name (PROGRAM/FB/CLASS), if available.
    pub owner: Option<smol_str::SmolStr>,
}

/// Runtime scope for inline values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineValueScope {
    /// Local variables and parameters (including instance locals in methods).
    Local,
    /// Global variables (VAR_GLOBAL / VAR_EXTERNAL / VAR_ACCESS).
    Global,
    /// Retained globals (RETAIN/PERSISTENT).
    Retain,
}

/// Inline value results for a range.
#[derive(Debug, Clone, Default)]
pub struct InlineValueData {
    /// Static inline hints (constants, enum values).
    pub hints: Vec<InlineValueHint>,
    /// Runtime-populated targets.
    pub targets: Vec<InlineValueTarget>,
}

/// Computes inline value hints and runtime targets within the given range.
pub fn inline_value_data(db: &Database, file_id: FileId, range: TextRange) -> InlineValueData {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let mut hints = Vec::new();
    let mut targets = Vec::new();
    let mut seen_hints = FxHashSet::default();
    let mut seen_targets = FxHashSet::default();

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        let Some(ident_range) = ident_range_from_name_ref(&node) else {
            continue;
        };
        if ident_range.end() <= range.start() || ident_range.start() >= range.end() {
            continue;
        }
        let target = resolve_target_at_position_with_context(
            db,
            file_id,
            ident_range.start(),
            &source,
            &root,
            &symbols,
        );
        let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
            continue;
        };
        let Some(symbol) = symbols.get(symbol_id) else {
            continue;
        };
        if let Some(text) = inline_text_for_symbol(db, file_id, symbol) {
            if seen_hints.insert(ident_range) {
                hints.push(InlineValueHint {
                    range: ident_range,
                    text,
                });
            }
            continue;
        }
        if let Some(scope) = runtime_scope_for_symbol(&root, &source, symbol) {
            let owner = owning_pou_name(symbols.as_ref(), symbol);
            if seen_targets.insert(ident_range) {
                targets.push(InlineValueTarget {
                    range: ident_range,
                    name: symbol.name.clone(),
                    scope,
                    owner,
                });
            }
        }
    }

    InlineValueData { hints, targets }
}

/// Computes inline value hints for constant/enum references within the given range.
pub fn inline_value_hints(
    db: &Database,
    file_id: FileId,
    range: TextRange,
) -> Vec<InlineValueHint> {
    inline_value_data(db, file_id, range).hints
}

fn inline_text_for_symbol(db: &Database, file_id: FileId, symbol: &Symbol) -> Option<String> {
    match symbol.kind {
        SymbolKind::EnumValue { value } => Some(format!(" = {value}")),
        _ if symbol_is_constant(symbol) => inline_text_for_constant(db, file_id, symbol),
        _ => None,
    }
}

fn inline_text_for_constant(db: &Database, file_id: FileId, symbol: &Symbol) -> Option<String> {
    let (decl_file_id, decl_range) = if let Some(origin) = symbol.origin {
        let origin_symbols = db.file_symbols(origin.file_id);
        let origin_range = origin_symbols
            .get(origin.symbol_id)
            .map(|sym| sym.range)
            .unwrap_or(symbol.range);
        (origin.file_id, origin_range)
    } else {
        (file_id, symbol.range)
    };

    let decl_source = db.source_text(decl_file_id);
    let decl_root = parse(&decl_source).syntax();
    let var_decl = find_var_decl_for_range(&decl_root, decl_range)?;
    let initializer = initializer_from_var_decl(&decl_source, &var_decl)?;
    Some(format!(" = {initializer}"))
}

fn runtime_scope_for_symbol(
    root: &trust_syntax::syntax::SyntaxNode,
    source: &str,
    symbol: &Symbol,
) -> Option<InlineValueScope> {
    match symbol.kind {
        SymbolKind::Variable { qualifier } => {
            let retention = var_decl_info_for_symbol(root, source, symbol.range).retention;
            Some(scope_for_variable(qualifier, retention))
        }
        SymbolKind::Parameter { .. } => Some(InlineValueScope::Local),
        _ => None,
    }
}

fn owning_pou_name(
    symbols: &trust_hir::symbols::SymbolTable,
    symbol: &Symbol,
) -> Option<smol_str::SmolStr> {
    let mut current = symbol.parent?;
    while let Some(parent) = symbols.get(current) {
        match parent.kind {
            SymbolKind::Program | SymbolKind::FunctionBlock | SymbolKind::Class => {
                return Some(parent.name.clone())
            }
            _ => {}
        }
        if let Some(next) = parent.parent {
            current = next;
        } else {
            break;
        }
    }
    None
}

fn scope_for_variable(
    qualifier: VarQualifier,
    retention: Option<&'static str>,
) -> InlineValueScope {
    let is_retain = matches!(retention, Some("RETAIN" | "PERSISTENT"));
    match qualifier {
        VarQualifier::Global | VarQualifier::External | VarQualifier::Access => {
            if is_retain {
                InlineValueScope::Retain
            } else {
                InlineValueScope::Global
            }
        }
        _ => InlineValueScope::Local,
    }
}

fn ident_range_from_name_ref(node: &trust_syntax::syntax::SyntaxNode) -> Option<TextRange> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text_range())
}

#[cfg(test)]
mod tests {
    use super::*;
    use text_size::TextSize;

    #[test]
    fn inline_value_hints_for_external_constant() {
        let constants = r#"
CONFIGURATION Conf
VAR_GLOBAL CONSTANT
    ANSWER : INT := 42;
END_VAR
END_CONFIGURATION
"#;
        let program = r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
VAR_EXTERNAL CONSTANT
    ANSWER : INT;
END_VAR
    x := ANSWER;
END_PROGRAM
"#;
        let mut db = Database::new();
        let const_id = FileId(0);
        let prog_id = FileId(1);
        db.set_source_text(const_id, constants.to_string());
        db.set_source_text(prog_id, program.to_string());

        let start = TextSize::from(0u32);
        let end = TextSize::from(program.len() as u32);
        let hints = inline_value_hints(&db, prog_id, TextRange::new(start, end));

        assert!(hints.iter().any(|hint| hint.text == " = 42"));
    }

    #[test]
    fn inline_value_hints_for_var_temp_constant() {
        let source = r#"
FUNCTION_BLOCK Fb
VAR_TEMP
    CONSTANT T : INT := INT#7;
END_VAR
VAR_OUTPUT
    X : INT;
END_VAR
    X := T;
END_FUNCTION_BLOCK
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = TextSize::from(0u32);
        let end = TextSize::from(source.len() as u32);
        let hints = inline_value_hints(&db, file_id, TextRange::new(start, end));

        assert!(hints.iter().any(|hint| hint.text == " = INT#7"));
    }
}
