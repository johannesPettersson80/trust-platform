//! Refactoring helpers for Structured Text.
//!
//! This module provides cross-file refactor primitives that go beyond rename.

use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{SymbolKind, SymbolTable};
use trust_hir::{is_reserved_keyword, is_valid_identifier, Database, SourceDatabase, SymbolId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::utilities;
use crate::references::{find_references, FindReferencesOptions};
use crate::rename::{RenameResult, TextEdit};
use crate::util::{
    ident_token_in_name, is_type_name_node, name_from_name_node, qualified_name_from_field_expr,
    qualified_name_parts_from_node, resolve_target_at_position,
    resolve_target_at_position_with_context, resolve_type_symbol_at_node, ResolvedTarget,
};

/// Result of an inline refactor request.
#[derive(Debug, Clone)]
pub struct InlineResult {
    /// Edits required to inline the target.
    pub edits: RenameResult,
    /// The inline target name.
    pub name: SmolStr,
    /// The inline target kind.
    pub kind: InlineTargetKind,
}

struct InlineExprInfo {
    text: String,
    kind: SyntaxKind,
    is_const_expr: bool,
    is_path_like: bool,
    requires_local_scope: bool,
}

/// The inline target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineTargetKind {
    /// Inline a variable.
    Variable,
    /// Inline a constant.
    Constant,
}

/// Result of an extract refactor request.
#[derive(Debug, Clone)]
pub struct ExtractResult {
    /// Edits required to perform the extraction.
    pub edits: RenameResult,
    /// The extracted symbol name.
    pub name: SmolStr,
    /// The extracted symbol kind.
    pub kind: ExtractTargetKind,
}

/// The extract target kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractTargetKind {
    /// Extract a METHOD.
    Method,
    /// Extract a PROPERTY (GET-only).
    Property,
    /// Extract a FUNCTION (POU).
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractParamDirection {
    Input,
    InOut,
}

#[derive(Debug, Clone)]
struct ExtractParam {
    name: SmolStr,
    type_name: SmolStr,
    direction: ExtractParamDirection,
    first_pos: TextSize,
}

/// Parses a dotted namespace path into parts, validating identifiers.
pub fn parse_namespace_path(path: &str) -> Option<Vec<SmolStr>> {
    utilities::parse_namespace_path(path)
}

/// Returns the full namespace path (including the namespace itself).
pub(crate) fn namespace_full_path(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<Vec<SmolStr>> {
    utilities::namespace_full_path(symbols, symbol_id)
}

fn symbol_qualified_name(symbols: &SymbolTable, symbol_id: SymbolId) -> Option<String> {
    utilities::symbol_qualified_name(symbols, symbol_id)
}

/// Moves a namespace path by rewriting `USING` directives and qualified names.
///
/// The namespace leaf name must remain unchanged; this does not relocate declarations.
pub fn move_namespace_path(
    db: &Database,
    old_path: &[SmolStr],
    new_path: &[SmolStr],
) -> Option<RenameResult> {
    if old_path.is_empty() || new_path.is_empty() {
        return None;
    }
    if !old_path
        .last()
        .zip(new_path.last())
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b.as_str()))
    {
        return None;
    }

    let mut result = RenameResult::new();

    for file_id in db.file_ids() {
        apply_move_in_file(db, file_id, old_path, new_path, &mut result);
    }

    if result.edit_count() == 0 {
        None
    } else {
        Some(result)
    }
}

/// Generates stub implementations for missing interface members on a class/function block.
///
/// Returns edits that insert method/property stubs before END_CLASS/END_FUNCTION_BLOCK.
pub fn generate_interface_stubs(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let owner_node = find_enclosing_owner_node(
        &root,
        position,
        &[SyntaxKind::Class, SyntaxKind::FunctionBlock],
    )?;
    let implements_clause = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::ImplementsClause)?;
    let owner_name_node = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let owner_ident = ident_token_in_name(&owner_name_node)?;
    let owner_symbol = symbols.iter().find(|symbol| {
        symbol.range == owner_ident.text_range()
            && matches!(symbol.kind, SymbolKind::Class | SymbolKind::FunctionBlock)
    })?;
    let owner_id = owner_symbol.id;

    let implemented = collect_implementation_members(&symbols, owner_id);
    let interface_names = implements_clause_names(&implements_clause);
    let stubs =
        collect_missing_interface_stubs(db, &symbols, &interface_names, &implemented, file_id);

    if stubs.is_empty() {
        return None;
    }

    let member_indent = member_indent_for_owner(&source, &owner_node);
    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_stub_insert_text(&source, insert_offset, &stubs, &member_indent);

    let mut result = RenameResult::new();
    result.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(result)
}

/// Inlines a variable/constant at the given position with safety checks.
pub fn inline_symbol(db: &Database, file_id: FileId, position: TextSize) -> Option<InlineResult> {
    let symbols = db.file_symbols_with_project(file_id);

    let target = resolve_target_at_position(db, file_id, position)?;
    let ResolvedTarget::Symbol(symbol_id) = target else {
        return None;
    };
    let symbol = symbols.get(symbol_id)?;

    let (kind, allow_inline) = match symbol.kind {
        SymbolKind::Constant => (InlineTargetKind::Constant, true),
        SymbolKind::Variable { qualifier } => {
            let allowed = matches!(
                qualifier,
                trust_hir::symbols::VarQualifier::Local
                    | trust_hir::symbols::VarQualifier::Temp
                    | trust_hir::symbols::VarQualifier::Static
            );
            (InlineTargetKind::Variable, allowed)
        }
        _ => (InlineTargetKind::Variable, false),
    };

    if !allow_inline {
        return None;
    }

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
    let expr = initializer_expr_in_var_decl(&var_decl)?;
    let expr_info = inline_expr_info(db, decl_file_id, &decl_source, &decl_root, &expr)?;

    if !expr_info.is_const_expr {
        return None;
    }

    let references = find_references(
        db,
        file_id,
        position,
        FindReferencesOptions {
            include_declaration: false,
        },
    );
    if references.is_empty() {
        return None;
    }

    if references.iter().any(|reference| reference.is_write) {
        return None;
    }

    if references.iter().any(|reference| {
        reference_has_disallowed_context(
            db,
            reference.file_id,
            reference.range,
            expr_info.is_path_like,
        )
    }) {
        return None;
    }

    if references
        .iter()
        .any(|reference| reference.file_id != decl_file_id)
        && expr_info.requires_local_scope
    {
        return None;
    }

    let replacement = wrap_expression_for_inline(expr_info.kind, &expr_info.text);

    let removal_range = var_decl_removal_range(&decl_source, &decl_root, decl_range)?;

    let mut edits = RenameResult::new();
    for reference in references {
        edits.add_edit(
            reference.file_id,
            TextEdit {
                range: reference.range,
                new_text: replacement.clone(),
            },
        );
    }
    edits.add_edit(
        decl_file_id,
        TextEdit {
            range: removal_range,
            new_text: String::new(),
        },
    );

    Some(InlineResult {
        edits,
        name: symbol.name.clone(),
        kind,
    })
}

/// Extracts selected statements into a new METHOD on the enclosing CLASS/FUNCTION_BLOCK.
pub fn extract_method(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;

    let stmt_range = statement_range_for_selection(&source, &root, selection)?;
    let method_node = find_enclosing_owner_node(&root, selection.start(), &[SyntaxKind::Method])?;
    let owner_node = find_enclosing_owner_node(
        &root,
        selection.start(),
        &[SyntaxKind::Class, SyntaxKind::FunctionBlock],
    )?;
    if !range_contains(owner_node.text_range(), method_node.text_range()) {
        return None;
    }

    let statements = text_for_range(&source, stmt_range);
    if statements.is_empty() {
        return None;
    }

    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;
    let method_id = owner_symbol_id(&symbols, &method_node)?;
    let name = unique_member_name(&symbols, owner_id, "ExtractedMethod");

    let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
        symbol.parent == Some(method_id)
    });
    let member_indent = member_indent_for_owner(&source, &owner_node);
    let indent_unit = indent_unit_for(&member_indent);
    let param_blocks = build_param_blocks(&params, &member_indent, indent_unit);
    let call_args = build_formal_args(&params);
    let body_indent = format!("{member_indent}{indent_unit}");
    let body_text = reindent_block(&statements, &body_indent);
    let method_text = build_method_extract_text(&name, &member_indent, &param_blocks, &body_text);

    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_insert_text(&source, insert_offset, &method_text);

    let call_indent = line_indent_at_offset(&source, stmt_range.start());
    let call_text = call_replace_text(&source, stmt_range, &call_indent, &name, &call_args);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: stmt_range,
            new_text: call_text,
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Method,
    })
}

/// Extracts a selected expression into a GET-only PROPERTY on the enclosing CLASS.
pub fn extract_property(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;
    let expr_node = expression_node_for_selection(&root, selection)?;

    if is_write_context(&expr_node) || is_call_name(&expr_node) {
        return None;
    }

    let owner_node = find_enclosing_owner_node(&root, selection.start(), &[SyntaxKind::Class])?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;

    let target = resolve_target_at_position_with_context(
        db,
        file_id,
        selection.start(),
        &source,
        &root,
        &symbols,
    )?;
    let type_name = match target {
        ResolvedTarget::Symbol(symbol_id) => symbols.type_name(symbols.get(symbol_id)?.type_id),
        ResolvedTarget::Field(field) => symbols.type_name(field.type_id),
    }?;

    let name = unique_member_name(&symbols, owner_id, "ExtractedProperty");
    let expr_text = text_for_range(&source, expr_node.text_range());
    if expr_text.is_empty() {
        return None;
    }

    let member_indent = member_indent_for_owner(&source, &owner_node);
    let indent_unit = indent_unit_for(&member_indent);
    let body_indent = format!("{member_indent}{indent_unit}");
    let property_text = build_property_extract_text(
        &name,
        type_name.as_str(),
        &expr_text,
        &member_indent,
        &body_indent,
    );

    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_insert_text(&source, insert_offset, &property_text);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: selection,
            new_text: name.to_string(),
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Property,
    })
}

/// Extracts selected statements into a new FUNCTION POU.
pub fn extract_pou(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;

    let owner_node = find_enclosing_owner_node(
        &root,
        selection.start(),
        &[
            SyntaxKind::Program,
            SyntaxKind::Function,
            SyntaxKind::FunctionBlock,
        ],
    )?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;
    let name = unique_top_level_name(&symbols, "ExtractedFunction");

    if let Some(expr_node) = expression_node_for_selection(&root, selection) {
        if is_write_context(&expr_node) || is_call_name(&expr_node) {
            return None;
        }
        let expr_id = db.expr_id_at_offset(file_id, u32::from(expr_node.text_range().start()))?;
        let type_name = symbols.type_name(db.type_of(file_id, expr_id))?;
        let expr_text = text_for_range(&source, expr_node.text_range());
        if expr_text.is_empty() {
            return None;
        }
        let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
            symbol.parent == Some(owner_id)
        });
        let indent_unit = indent_unit_for("");
        let param_blocks = build_param_blocks(&params, "", indent_unit);
        let call_args = build_formal_args(&params);
        let function_text = build_function_extract_text(
            &name,
            type_name.as_str(),
            &param_blocks,
            "",
            indent_unit,
            Some(&expr_text),
        );

        let insert_offset = usize::from(owner_node.text_range().end());
        let insert_text = build_insert_text(&source, insert_offset, &function_text);

        let call_text = build_call_expression(&name, &call_args);

        let mut edits = RenameResult::new();
        edits.add_edit(
            file_id,
            TextEdit {
                range: selection,
                new_text: call_text,
            },
        );
        edits.add_edit(
            file_id,
            TextEdit {
                range: TextRange::new(
                    TextSize::from(insert_offset as u32),
                    TextSize::from(insert_offset as u32),
                ),
                new_text: insert_text,
            },
        );

        return Some(ExtractResult {
            edits,
            name,
            kind: ExtractTargetKind::Function,
        });
    }

    let stmt_range = statement_range_for_selection(&source, &root, selection)?;
    let statements = text_for_range(&source, stmt_range);
    if statements.is_empty() {
        return None;
    }

    let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
        symbol.parent == Some(owner_id)
    });
    let indent_unit = indent_unit_for("");
    let param_blocks = build_param_blocks(&params, "", indent_unit);
    let call_args = build_formal_args(&params);
    let body_text = reindent_block(&statements, indent_unit);
    let function_text =
        build_function_extract_text(&name, "BOOL", &param_blocks, &body_text, indent_unit, None);

    let insert_offset = usize::from(owner_node.text_range().end());
    let insert_text = build_insert_text(&source, insert_offset, &function_text);

    let call_indent = line_indent_at_offset(&source, stmt_range.start());
    let call_text = call_replace_text(&source, stmt_range, &call_indent, &name, &call_args);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: stmt_range,
            new_text: call_text,
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Function,
    })
}

/// Converts a FUNCTION to a FUNCTION_BLOCK.
pub fn convert_function_to_function_block(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let node = find_enclosing_owner_node(&root, position, &[SyntaxKind::Function])?;

    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let function_name = name_from_name_node(&name_node)?;
    let symbols = db.file_symbols_with_project(file_id);
    let function_id = owner_symbol_id(&symbols, &node)?;
    let function_type =
        symbol_qualified_name(&symbols, function_id).unwrap_or_else(|| function_name.to_string());
    let mut output_name = None::<String>;

    if has_recursive_call(&source, &node, function_name.as_str()) {
        return None;
    }

    let mut edits = RenameResult::new();
    if let Some(token) = keyword_token(&node, SyntaxKind::KwFunction) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "FUNCTION_BLOCK".to_string(),
            },
        );
    }
    if let Some(token) = keyword_token(&node, SyntaxKind::KwEndFunction) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "END_FUNCTION_BLOCK".to_string(),
            },
        );
    }

    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(&source, child.text_range()));
    if let Some(range) = function_return_type_range(&node) {
        edits.add_edit(
            file_id,
            TextEdit {
                range,
                new_text: String::new(),
            },
        );
    }

    if let Some(return_type) = return_type {
        if has_var_output_block(&node) {
            return None;
        }
        let output_name_local = unique_local_name(&symbols, &node, "result");
        output_name = Some(output_name_local.clone());
        let insert_offset = var_block_insert_offset(&node)?;
        let indent = line_indent_at_offset(&source, TextSize::from(insert_offset as u32));
        let indent_unit = indent_unit_for(&indent);
        let var_block_text =
            build_var_output_block(&indent, indent_unit, &output_name_local, &return_type);
        edits.add_edit(
            file_id,
            TextEdit {
                range: TextRange::new(
                    TextSize::from(insert_offset as u32),
                    TextSize::from(insert_offset as u32),
                ),
                new_text: var_block_text,
            },
        );

        if let Some(stmt_list) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::StmtList)
        {
            replace_name_refs(
                &source,
                &stmt_list,
                function_name.as_str(),
                output_name_local.as_str(),
                &mut edits,
                file_id,
            );
        }
    }

    let call_context = FunctionCallUpdateContext {
        function_file_id: file_id,
        function_node: &node,
        function_name: &function_name,
        function_id,
        function_type: function_type.as_str(),
        output_name: output_name.as_deref(),
    };
    update_function_call_sites(db, call_context, &mut edits)?;

    (edits.edit_count() > 0).then_some(edits)
}

/// Converts a FUNCTION_BLOCK to a FUNCTION when it has a single VAR_OUTPUT variable.
pub fn convert_function_block_to_function(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let node = find_enclosing_owner_node(&root, position, &[SyntaxKind::FunctionBlock])?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &node)?;
    let owner_name = symbol_qualified_name(&symbols, owner_id)?;
    if function_block_has_type_references(db, owner_name.as_str()) {
        return None;
    }
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let output = output_var_info(&source, &root, &node)?;
    let function_name = name_from_name_node(&name_node)?;

    let mut edits = RenameResult::new();
    if let Some(token) = keyword_token(&node, SyntaxKind::KwFunctionBlock) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "FUNCTION".to_string(),
            },
        );
    }
    if let Some(token) = keyword_token(&node, SyntaxKind::KwEndFunctionBlock) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "END_FUNCTION".to_string(),
            },
        );
    }

    let name_end = name_node.text_range().end();
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(name_end, name_end),
            new_text: format!(" : {}", output.type_name),
        },
    );

    edits.add_edit(
        file_id,
        TextEdit {
            range: output.removal_range,
            new_text: String::new(),
        },
    );

    if let Some(stmt_list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::StmtList)
    {
        replace_name_refs(
            &source,
            &stmt_list,
            output.name.as_str(),
            function_name.as_str(),
            &mut edits,
            file_id,
        );
    }

    (edits.edit_count() > 0).then_some(edits)
}

fn apply_move_in_file(
    db: &Database,
    file_id: FileId,
    old_path: &[SmolStr],
    new_path: &[SmolStr],
    result: &mut RenameResult,
) {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    for scope in symbols.scopes() {
        for using in &scope.using_directives {
            if !path_eq_ignore_ascii_case(&using.path, old_path) {
                continue;
            }
            let new_text = join_namespace_path(new_path);
            result.add_edit(
                file_id,
                TextEdit {
                    range: using.range,
                    new_text,
                },
            );
        }
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Namespace)
    {
        let name_node = node
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName));
        let Some(name_node) = name_node else {
            continue;
        };
        let Some(parts) = qualified_name_parts_from_node(&name_node) else {
            continue;
        };
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }
        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&name_node),
                new_text,
            },
        );
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::QualifiedName)
    {
        if node
            .ancestors()
            .skip(1)
            .any(|ancestor| ancestor.kind() == SyntaxKind::UsingDirective)
        {
            continue;
        }
        if node
            .parent()
            .map(|parent| parent.kind() == SyntaxKind::Namespace)
            .unwrap_or(false)
        {
            continue;
        }

        let parts = qualified_name_parts(&node);
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }

        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&node),
                new_text,
            },
        );
    }

    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::FieldExpr)
    {
        let Some(parts) = qualified_name_from_field_expr(&node) else {
            continue;
        };
        if !path_starts_with_ignore_ascii_case(&parts, old_path) {
            continue;
        }
        if symbols.resolve_qualified(&parts).is_none() {
            continue;
        }
        let mut updated = new_path.to_vec();
        updated.extend_from_slice(&parts[old_path.len()..]);
        let new_text = join_namespace_path(&updated);
        result.add_edit(
            file_id,
            TextEdit {
                range: node_token_range(&node),
                new_text,
            },
        );
    }
}

#[derive(Debug, Clone)]
struct ImplementedMembers {
    methods: FxHashSet<SmolStr>,
    properties: FxHashSet<SmolStr>,
}

#[derive(Debug, Clone)]
struct InterfaceStub {
    name_key: SmolStr,
    kind: InterfaceStubKind,
}

#[derive(Debug, Clone)]
enum InterfaceStubKind {
    Method(MethodStub),
    Property(PropertyStub),
}

#[derive(Debug, Clone)]
struct MethodStub {
    name: SmolStr,
    return_type: Option<String>,
    var_blocks: Vec<String>,
}

#[derive(Debug, Clone)]
struct PropertyStub {
    name: SmolStr,
    type_name: Option<String>,
    has_get: bool,
    has_set: bool,
}

fn collect_missing_interface_stubs(
    db: &Database,
    symbols: &SymbolTable,
    interfaces: &[Vec<SmolStr>],
    implemented: &ImplementedMembers,
    fallback_file_id: FileId,
) -> Vec<InterfaceStub> {
    let mut stubs = Vec::new();
    let mut seen = FxHashSet::default();

    for parts in interfaces {
        if parts.is_empty() {
            continue;
        }
        let interface_id = symbols
            .resolve_qualified(parts)
            .or_else(|| symbols.resolve_by_name(&join_namespace_path(parts)));
        let Some(interface_id) = interface_id else {
            continue;
        };
        collect_interface_stubs(
            db,
            symbols,
            interface_id,
            fallback_file_id,
            implemented,
            &mut seen,
            &mut stubs,
        );
    }

    stubs
}

fn collect_interface_stubs(
    db: &Database,
    symbols: &SymbolTable,
    interface_id: SymbolId,
    fallback_file_id: FileId,
    implemented: &ImplementedMembers,
    seen: &mut FxHashSet<SmolStr>,
    out: &mut Vec<InterfaceStub>,
) {
    let mut stack = vec![interface_id];
    let mut visited = FxHashSet::default();

    while let Some(current) = stack.pop() {
        if !visited.insert(current) {
            continue;
        }

        let Some(interface_symbol) = symbols.get(current) else {
            continue;
        };
        if !matches!(interface_symbol.kind, SymbolKind::Interface) {
            continue;
        }

        let interface_file_id = interface_symbol
            .origin
            .map(|origin| origin.file_id)
            .unwrap_or(fallback_file_id);
        let interface_source = db.source_text(interface_file_id);
        let interface_root = parse(&interface_source).syntax();
        if let Some(interface_node) =
            find_interface_node_for_symbol(&interface_root, interface_symbol.range)
        {
            for child in interface_node.children() {
                match child.kind() {
                    SyntaxKind::Method => {
                        let Some(stub) =
                            method_stub_from_interface(&interface_source, &child, implemented)
                        else {
                            continue;
                        };
                        if seen.insert(stub.name_key.clone()) {
                            out.push(stub);
                        }
                    }
                    SyntaxKind::Property => {
                        let Some(stub) =
                            property_stub_from_interface(&interface_source, &child, implemented)
                        else {
                            continue;
                        };
                        if seen.insert(stub.name_key.clone()) {
                            out.push(stub);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(base_name) = symbols.extends_name(current) {
            if let Some(base_id) = symbols.resolve_by_name(base_name.as_str()) {
                stack.push(base_id);
            }
        }
    }
}

fn method_stub_from_interface(
    source: &str,
    node: &SyntaxNode,
    implemented: &ImplementedMembers,
) -> Option<InterfaceStub> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let name = name_from_name_node(&name_node)?;
    let key = normalize_member_name(name.as_str());
    if implemented.methods.contains(&key) {
        return None;
    }

    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(source, child.text_range()));
    let var_blocks = node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .map(|block| text_for_range(source, block.text_range()))
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>();

    Some(InterfaceStub {
        name_key: key,
        kind: InterfaceStubKind::Method(MethodStub {
            name,
            return_type,
            var_blocks,
        }),
    })
}

fn property_stub_from_interface(
    source: &str,
    node: &SyntaxNode,
    implemented: &ImplementedMembers,
) -> Option<InterfaceStub> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let name = name_from_name_node(&name_node)?;
    let key = normalize_member_name(name.as_str());
    if implemented.properties.contains(&key) {
        return None;
    }

    let type_name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(source, child.text_range()));
    let has_get = node
        .children()
        .any(|child| child.kind() == SyntaxKind::PropertyGet);
    let has_set = node
        .children()
        .any(|child| child.kind() == SyntaxKind::PropertySet);

    Some(InterfaceStub {
        name_key: key,
        kind: InterfaceStubKind::Property(PropertyStub {
            name,
            type_name,
            has_get,
            has_set,
        }),
    })
}

fn collect_implementation_members(symbols: &SymbolTable, owner_id: SymbolId) -> ImplementedMembers {
    let mut methods = FxHashSet::default();
    let mut properties = FxHashSet::default();
    let mut visited = FxHashSet::default();
    let mut current = Some(owner_id);

    while let Some(symbol_id) = current {
        if !visited.insert(symbol_id) {
            break;
        }

        for sym in symbols.iter() {
            if sym.parent != Some(symbol_id) {
                continue;
            }
            match sym.kind {
                SymbolKind::Method { .. } => {
                    if sym.modifiers.is_abstract {
                        continue;
                    }
                    methods.insert(normalize_member_name(sym.name.as_str()));
                }
                SymbolKind::Property { .. } => {
                    properties.insert(normalize_member_name(sym.name.as_str()));
                }
                _ => {}
            }
        }

        current = symbols
            .extends_name(symbol_id)
            .and_then(|base_name| symbols.resolve_by_name(base_name.as_str()));
    }

    ImplementedMembers {
        methods,
        properties,
    }
}

fn find_interface_node_for_symbol(root: &SyntaxNode, name_range: TextRange) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::Interface)
        .find(|interface_node| {
            interface_node
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(|node| ident_token_in_name(&node))
                .any(|ident| ident.text_range() == name_range)
        })
}

fn implements_clause_names(node: &SyntaxNode) -> Vec<Vec<SmolStr>> {
    let mut names = Vec::new();
    for child in node.children() {
        if !matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName) {
            continue;
        }
        if let Some(parts) = qualified_name_parts_from_node(&child) {
            names.push(parts);
        }
    }
    names
}

fn owner_end_token_offset(node: &SyntaxNode) -> Option<usize> {
    let end_kind = match node.kind() {
        SyntaxKind::Class => SyntaxKind::KwEndClass,
        SyntaxKind::FunctionBlock => SyntaxKind::KwEndFunctionBlock,
        _ => return None,
    };
    let token = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == end_kind)?;
    Some(usize::from(token.text_range().start()))
}

fn member_indent_for_owner(source: &str, owner_node: &SyntaxNode) -> String {
    if let Some(member) = owner_node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Method | SyntaxKind::Property))
    {
        return line_indent_at_offset(source, member.text_range().start());
    }

    let owner_indent = line_indent_at_offset(source, owner_node.text_range().start());
    let indent_unit = if owner_indent.contains('\t') {
        "\t"
    } else {
        "    "
    };
    format!("{owner_indent}{indent_unit}")
}

fn build_stub_insert_text(
    source: &str,
    insert_offset: usize,
    stubs: &[InterfaceStub],
    member_indent: &str,
) -> String {
    let indent_unit = if member_indent.contains('\t') {
        "\t"
    } else {
        "    "
    };
    let child_indent = format!("{member_indent}{indent_unit}");

    let mut chunks = Vec::new();
    for stub in stubs {
        let text = match &stub.kind {
            InterfaceStubKind::Method(method) => {
                build_method_stub(method, member_indent, &child_indent)
            }
            InterfaceStubKind::Property(property) => {
                build_property_stub(property, member_indent, &child_indent)
            }
        };
        chunks.push(text);
    }

    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }

    insert.push_str(&chunks.join("\n\n"));
    if !insert.ends_with('\n') {
        insert.push('\n');
    }
    insert
}

fn build_insert_text(source: &str, insert_offset: usize, block: &str) -> String {
    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }
    insert.push('\n');
    insert.push_str(block);
    if !insert.ends_with('\n') {
        insert.push('\n');
    }
    insert
}

fn build_method_extract_text(name: &str, indent: &str, params: &str, body: &str) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{indent}METHOD {name}"));
    if !params.is_empty() {
        lines.push(params.to_string());
    }
    if !body.trim().is_empty() {
        lines.push(body.to_string());
    }
    lines.push(format!("{indent}END_METHOD"));
    lines.join("\n")
}

fn build_property_extract_text(
    name: &str,
    type_name: &str,
    expr: &str,
    indent: &str,
    body_indent: &str,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{indent}PROPERTY {name} : {type_name}"));
    lines.push(format!("{indent}GET"));
    lines.push(format!("{body_indent}{name} := {expr};"));
    lines.push(format!("{indent}END_GET"));
    lines.push(format!("{indent}END_PROPERTY"));
    lines.join("\n")
}

fn build_function_extract_text(
    name: &str,
    return_type: &str,
    params: &str,
    body: &str,
    body_indent: &str,
    result_expr: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("FUNCTION {name} : {return_type}"));
    if !params.is_empty() {
        lines.push(params.to_string());
    }
    if !body.trim().is_empty() {
        lines.push(body.to_string());
    }
    let result = result_expr.unwrap_or("TRUE");
    lines.push(format!("{body_indent}{name} := {result};"));
    lines.push("END_FUNCTION".to_string());
    lines.join("\n")
}

fn build_var_output_block(indent: &str, indent_unit: &str, name: &str, type_name: &str) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}VAR_OUTPUT"));
    lines.push(format!("{child_indent}{name} : {type_name};"));
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_var_block(indent: &str, indent_unit: &str, name: &str, type_name: &str) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}VAR"));
    lines.push(format!("{child_indent}{name} : {type_name};"));
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_method_stub(stub: &MethodStub, indent: &str, child_indent: &str) -> String {
    let mut lines = Vec::new();
    let mut header = format!("{indent}METHOD PUBLIC {}", stub.name);
    if let Some(return_type) = &stub.return_type {
        header.push_str(&format!(" : {}", return_type));
    }
    lines.push(header);

    for block in &stub.var_blocks {
        let block = reindent_block(block, indent);
        if !block.is_empty() {
            lines.push(block);
        }
    }

    lines.push(format!("{child_indent}// TODO: implement"));
    lines.push(format!("{indent}END_METHOD"));
    lines.join("\n")
}

fn build_property_stub(stub: &PropertyStub, indent: &str, child_indent: &str) -> String {
    let mut lines = Vec::new();
    let type_suffix = stub
        .type_name
        .as_ref()
        .map(|ty| format!(" : {}", ty))
        .unwrap_or_default();
    lines.push(format!(
        "{indent}PROPERTY PUBLIC {}{}",
        stub.name, type_suffix
    ));
    if stub.has_get {
        lines.push(format!("{indent}GET"));
        lines.push(format!("{child_indent}// TODO: implement"));
        lines.push(format!("{indent}END_GET"));
    }
    if stub.has_set {
        lines.push(format!("{indent}SET"));
        lines.push(format!("{child_indent}// TODO: implement"));
        lines.push(format!("{indent}END_SET"));
    }
    lines.push(format!("{indent}END_PROPERTY"));
    lines.join("\n")
}

fn indent_unit_for(indent: &str) -> &str {
    utilities::indent_unit_for(indent)
}

fn reindent_block(block: &str, indent: &str) -> String {
    utilities::reindent_block(block, indent)
}

fn line_indent_at_offset(source: &str, offset: TextSize) -> String {
    utilities::line_indent_at_offset(source, offset)
}

fn build_formal_args(params: &[ExtractParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let args = params
        .iter()
        .map(|param| format!("{} := {}", param.name, param.name))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({args})")
}

fn build_param_blocks(params: &[ExtractParam], indent: &str, indent_unit: &str) -> String {
    let mut blocks = Vec::new();

    let inputs: Vec<_> = params
        .iter()
        .filter(|param| param.direction == ExtractParamDirection::Input)
        .collect();
    if !inputs.is_empty() {
        blocks.push(build_param_block("VAR_INPUT", &inputs, indent, indent_unit));
    }

    let in_outs: Vec<_> = params
        .iter()
        .filter(|param| param.direction == ExtractParamDirection::InOut)
        .collect();
    if !in_outs.is_empty() {
        blocks.push(build_param_block(
            "VAR_IN_OUT",
            &in_outs,
            indent,
            indent_unit,
        ));
    }

    blocks.join("\n")
}

fn build_param_block(
    label: &str,
    params: &[&ExtractParam],
    indent: &str,
    indent_unit: &str,
) -> String {
    let child_indent = format!("{indent}{indent_unit}");
    let mut lines = Vec::new();
    lines.push(format!("{indent}{label}"));
    for param in params {
        lines.push(format!(
            "{child_indent}{} : {};",
            param.name, param.type_name
        ));
    }
    lines.push(format!("{indent}END_VAR"));
    lines.join("\n")
}

fn build_call_expression(name: &str, args: &str) -> String {
    let args = if args.is_empty() { "()" } else { args };
    format!("{name}{args}")
}

fn call_replace_text(
    source: &str,
    range: TextRange,
    indent: &str,
    name: &str,
    args: &str,
) -> String {
    let args = if args.is_empty() { "()" } else { args };
    let mut text = format!("{indent}{name}{args};");
    if let Some(suffix) = source.get(usize::from(range.end())..) {
        if suffix.starts_with('\n') || suffix.starts_with("\r\n") {
            text.push('\n');
        }
    }
    text
}

fn collect_extract_params<F>(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    selection: TextRange,
    capture: F,
) -> Vec<ExtractParam>
where
    F: Fn(&trust_hir::symbols::Symbol) -> bool,
{
    let symbols = db.file_symbols_with_project(file_id);
    let declared = declared_symbols_in_range(&symbols, selection);
    let mut params: FxHashMap<SymbolId, ExtractParam> = FxHashMap::default();

    for name_ref in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        let range = node_token_range(&name_ref);
        if !range_contains(selection, range) {
            continue;
        }
        if is_call_name(&name_ref) {
            continue;
        }
        let target = resolve_target_at_position_with_context(
            db,
            file_id,
            range.start(),
            source,
            root,
            &symbols,
        );
        let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
            continue;
        };
        if declared.contains(&symbol_id) {
            continue;
        }
        let Some(symbol) = symbols.get(symbol_id) else {
            continue;
        };
        if !capture(symbol) {
            continue;
        }
        if !matches!(
            symbol.kind,
            SymbolKind::Variable { .. } | SymbolKind::Parameter { .. } | SymbolKind::Constant
        ) {
            continue;
        }
        let Some(type_name) = symbols.type_name(symbol.type_id) else {
            continue;
        };

        let entry = params.entry(symbol_id).or_insert(ExtractParam {
            name: symbol.name.clone(),
            type_name,
            direction: ExtractParamDirection::Input,
            first_pos: range.start(),
        });
        if range.start() < entry.first_pos {
            entry.first_pos = range.start();
        }
        if is_write_context(&name_ref) {
            entry.direction = ExtractParamDirection::InOut;
        }
    }

    let mut params: Vec<_> = params.into_values().collect();
    params.sort_by_key(|param| param.first_pos);
    params
}

fn declared_symbols_in_range(symbols: &SymbolTable, range: TextRange) -> FxHashSet<SymbolId> {
    symbols
        .iter()
        .filter(|symbol| range_contains(range, symbol.range))
        .map(|symbol| symbol.id)
        .collect()
}

fn trim_range_to_non_whitespace(source: &str, range: TextRange) -> Option<TextRange> {
    utilities::trim_range_to_non_whitespace(source, range)
}

fn range_contains(outer: TextRange, inner: TextRange) -> bool {
    utilities::range_contains(outer, inner)
}

fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    utilities::ranges_overlap(a, b)
}

fn enclosing_stmt_list(root: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    let start_token = root.token_at_offset(range.start()).right_biased()?;
    let end_token = root.token_at_offset(range.end()).left_biased()?;
    let start_list = start_token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::StmtList)?;
    let end_list = end_token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::StmtList)?;
    if start_list.text_range() == end_list.text_range() {
        Some(start_list)
    } else {
        None
    }
}

fn statement_range_for_selection(
    source: &str,
    root: &SyntaxNode,
    selection: TextRange,
) -> Option<TextRange> {
    let stmt_list = enclosing_stmt_list(root, selection)?;
    let mut selected = Vec::new();
    for child in stmt_list
        .children()
        .filter(|node| is_statement_kind(node.kind()))
    {
        if ranges_overlap(child.text_range(), selection) {
            selected.push(child);
        }
    }
    if selected.is_empty() {
        return None;
    }
    let start = selected.first()?.text_range().start();
    let end = selected.last()?.text_range().end();
    let covered = TextRange::new(start, end);
    let trimmed = trim_range_to_non_whitespace(source, selection)?;
    let covered_trimmed = trim_range_to_non_whitespace(source, covered)?;
    if trimmed != covered_trimmed {
        return None;
    }
    Some(covered)
}

fn expression_node_for_selection(root: &SyntaxNode, selection: TextRange) -> Option<SyntaxNode> {
    let token = root.token_at_offset(selection.start()).right_biased()?;
    for expr in token
        .parent_ancestors()
        .filter(|node| is_expression_kind(node.kind()))
    {
        let expr_range = node_token_range(&expr);
        if expr_range == selection {
            return Some(expr);
        }
    }
    None
}

fn keyword_token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn function_return_type_range(node: &SyntaxNode) -> Option<TextRange> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let type_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)?;
    let name_end = name_node.text_range().end();
    let type_start = type_node.text_range().start();
    let colon = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            token.kind() == SyntaxKind::Colon
                && token.text_range().start() >= name_end
                && token.text_range().end() <= type_start
        })?;
    Some(TextRange::new(
        colon.text_range().start(),
        type_node.text_range().end(),
    ))
}

fn has_var_output_block(node: &SyntaxNode) -> bool {
    node.children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .any(|block| var_block_kind(&block) == Some(SyntaxKind::KwVarOutput))
}

fn var_block_insert_offset(node: &SyntaxNode) -> Option<usize> {
    for child in node.children() {
        if matches!(child.kind(), SyntaxKind::VarBlock | SyntaxKind::StmtList) {
            return Some(usize::from(child.text_range().start()));
        }
    }
    Some(usize::from(node.text_range().end()))
}

fn var_block_kind(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn replace_name_refs(
    source: &str,
    stmt_list: &SyntaxNode,
    from: &str,
    to: &str,
    edits: &mut RenameResult,
    file_id: FileId,
) {
    for node in stmt_list
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        if is_call_name(&node) {
            continue;
        }
        let range = node_token_range(&node);
        let text = text_for_range(source, range);
        if text.eq_ignore_ascii_case(from) {
            edits.add_edit(
                file_id,
                TextEdit {
                    range,
                    new_text: to.to_string(),
                },
            );
        }
    }
}

fn owner_symbol_id(symbols: &SymbolTable, owner_node: &SyntaxNode) -> Option<SymbolId> {
    let name_node = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let ident = ident_token_in_name(&name_node)?;
    symbols
        .iter()
        .find(|symbol| symbol.range == ident.text_range())
        .map(|symbol| symbol.id)
}

fn unique_member_name(symbols: &SymbolTable, owner_id: SymbolId, base: &str) -> SmolStr {
    let mut index = 0;
    loop {
        let candidate = if index == 0 {
            base.to_string()
        } else {
            format!("{base}{index}")
        };
        if is_valid_identifier(&candidate)
            && !is_reserved_keyword(&candidate)
            && !symbols.iter().any(|symbol| {
                symbol.parent == Some(owner_id)
                    && symbol.name.eq_ignore_ascii_case(candidate.as_str())
            })
        {
            return SmolStr::new(candidate);
        }
        index += 1;
    }
}

fn unique_top_level_name(symbols: &SymbolTable, base: &str) -> SmolStr {
    let mut index = 0;
    loop {
        let candidate = if index == 0 {
            base.to_string()
        } else {
            format!("{base}{index}")
        };
        if is_valid_identifier(&candidate)
            && !is_reserved_keyword(&candidate)
            && !symbols
                .iter()
                .any(|symbol| symbol.name.eq_ignore_ascii_case(candidate.as_str()))
        {
            return SmolStr::new(candidate);
        }
        index += 1;
    }
}

fn unique_local_name(symbols: &SymbolTable, owner_node: &SyntaxNode, base: &str) -> String {
    if let Some(owner_id) = owner_symbol_id(symbols, owner_node) {
        unique_member_name(symbols, owner_id, base).to_string()
    } else {
        base.to_string()
    }
}

fn is_call_name(expr: &SyntaxNode) -> bool {
    let Some(parent) = expr.parent() else {
        return false;
    };
    if parent.kind() != SyntaxKind::CallExpr {
        return false;
    }
    parent
        .first_child()
        .is_some_and(|child| child.text_range() == expr.text_range())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OwnerKey {
    file_id: FileId,
    range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StatementKey {
    file_id: FileId,
    range: TextRange,
}

enum CallContextKind {
    Statement,
    Expression,
}

struct CallContext {
    kind: CallContextKind,
    stmt_range: TextRange,
    insert_offset: usize,
    indent: String,
}

fn has_recursive_call(source: &str, function_node: &SyntaxNode, function_name: &str) -> bool {
    let Some(stmt_list) = function_node
        .children()
        .find(|child| child.kind() == SyntaxKind::StmtList)
    else {
        return false;
    };
    for name_ref in stmt_list
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        if !is_call_name(&name_ref) {
            continue;
        }
        let range = node_token_range(&name_ref);
        let text = text_for_range(source, range);
        if text.eq_ignore_ascii_case(function_name) {
            return true;
        }
    }
    false
}

fn call_callee_node(call_expr: &SyntaxNode) -> Option<SyntaxNode> {
    call_expr.children().find(|child| {
        matches!(
            child.kind(),
            SyntaxKind::NameRef
                | SyntaxKind::FieldExpr
                | SyntaxKind::QualifiedName
                | SyntaxKind::Name
        )
    })
}

fn call_expr_args_text(source: &str, call_expr: &SyntaxNode) -> String {
    if let Some(arg_list) = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)
    {
        let text = text_for_range(source, arg_list.text_range());
        if !text.is_empty() {
            return text;
        }
    }
    "()".to_string()
}

fn call_expr_context(source: &str, call_expr: &SyntaxNode) -> Option<CallContext> {
    let stmt = if let Some(parent) = call_expr.parent() {
        match parent.kind() {
            SyntaxKind::ExprStmt => Some(parent),
            _ => None,
        }
    } else {
        None
    };

    if let Some(stmt) = stmt {
        let stmt_range = stmt.text_range();
        let indent = line_indent_at_offset(source, stmt_range.start());
        return Some(CallContext {
            kind: CallContextKind::Statement,
            stmt_range,
            insert_offset: usize::from(stmt_range.start()),
            indent,
        });
    }

    if let Some(assign_stmt) = call_expr
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
    {
        if let Some(rhs_expr) = assign_rhs_expr(&assign_stmt) {
            if node_token_range(&rhs_expr) == node_token_range(call_expr) {
                let stmt_range = assign_stmt.text_range();
                let indent = line_indent_at_offset(source, stmt_range.start());
                return Some(CallContext {
                    kind: CallContextKind::Expression,
                    stmt_range,
                    insert_offset: usize::from(stmt_range.start()),
                    indent,
                });
            }
        }
        return None;
    }

    if let Some(return_stmt) = call_expr
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::ReturnStmt)
    {
        let expr = return_stmt
            .children()
            .find(|node| is_expression_kind(node.kind()))?;
        if node_token_range(&expr) == node_token_range(call_expr) {
            let stmt_range = return_stmt.text_range();
            let indent = line_indent_at_offset(source, stmt_range.start());
            return Some(CallContext {
                kind: CallContextKind::Expression,
                stmt_range,
                insert_offset: usize::from(stmt_range.start()),
                indent,
            });
        }
        return None;
    }

    None
}

fn call_targets_function(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    symbols: &SymbolTable,
    call_expr: &SyntaxNode,
    function_id: SymbolId,
) -> Option<TextRange> {
    let callee = call_callee_node(call_expr)?;
    let callee_range = node_token_range(&callee);
    let target = resolve_target_at_position_with_context(
        db,
        file_id,
        callee_range.start(),
        source,
        root,
        symbols,
    );
    if let Some(ResolvedTarget::Symbol(symbol_id)) = target {
        if symbol_id == function_id {
            return Some(callee_range);
        }
    }

    if let Some(parts) = match callee.kind() {
        SyntaxKind::FieldExpr => qualified_name_from_field_expr(&callee),
        SyntaxKind::QualifiedName | SyntaxKind::Name => qualified_name_parts_from_node(&callee),
        _ => None,
    } {
        if symbols.resolve_qualified(&parts) == Some(function_id) {
            return Some(callee_range);
        }
    }

    None
}

fn assign_rhs_expr(assign_stmt: &SyntaxNode) -> Option<SyntaxNode> {
    let mut exprs = assign_stmt
        .children()
        .filter(|node| is_expression_kind(node.kind()));
    let _lhs = exprs.next()?;
    let rhs = exprs.next()?;
    if exprs.next().is_some() {
        return None;
    }
    Some(rhs)
}

fn build_prefix_insert_text(source: &str, insert_offset: usize, line: &str) -> String {
    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }
    insert.push_str(line);
    if !line.ends_with('\n') {
        insert.push('\n');
    }
    insert
}

struct FunctionCallUpdateContext<'a> {
    function_file_id: FileId,
    function_node: &'a SyntaxNode,
    function_name: &'a SmolStr,
    function_id: SymbolId,
    function_type: &'a str,
    output_name: Option<&'a str>,
}

fn update_function_call_sites(
    db: &Database,
    context: FunctionCallUpdateContext<'_>,
    edits: &mut RenameResult,
) -> Option<()> {
    let function_range = context.function_node.text_range();
    let mut owner_instances: FxHashMap<OwnerKey, SmolStr> = FxHashMap::default();
    let mut inserted_calls: FxHashSet<StatementKey> = FxHashSet::default();

    for ref_file_id in db.file_ids() {
        let source = db.source_text(ref_file_id);
        let root = parse(&source).syntax();
        let symbols = db.file_symbols_with_project(ref_file_id);

        for call_expr in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::CallExpr)
        {
            if ref_file_id == context.function_file_id
                && range_contains(function_range, call_expr.text_range())
            {
                continue;
            }

            let Some(callee_range) = call_targets_function(
                db,
                ref_file_id,
                &source,
                &root,
                &symbols,
                &call_expr,
                context.function_id,
            ) else {
                continue;
            };

            let call_context = call_expr_context(&source, &call_expr)?;
            if matches!(call_context.kind, CallContextKind::Expression)
                && context.output_name.is_none()
            {
                return None;
            }

            let owner = call_expr.ancestors().find(|node| {
                matches!(
                    node.kind(),
                    SyntaxKind::Program
                        | SyntaxKind::Function
                        | SyntaxKind::FunctionBlock
                        | SyntaxKind::Method
                        | SyntaxKind::Action
                )
            })?;
            let owner_key = OwnerKey {
                file_id: ref_file_id,
                range: owner.text_range(),
            };
            let instance_name = if let Some(name) = owner_instances.get(&owner_key) {
                name.clone()
            } else {
                let base = format!("{}Instance", context.function_name);
                let name = SmolStr::new(unique_local_name(&symbols, &owner, &base));
                let insert_offset = var_block_insert_offset(&owner)?;
                let indent = line_indent_at_offset(&source, TextSize::from(insert_offset as u32));
                let indent_unit = indent_unit_for(&indent);
                let var_block =
                    build_var_block(&indent, indent_unit, name.as_str(), context.function_type);
                let insert_text = build_insert_text(&source, insert_offset, &var_block);
                edits.add_edit(
                    ref_file_id,
                    TextEdit {
                        range: TextRange::new(
                            TextSize::from(insert_offset as u32),
                            TextSize::from(insert_offset as u32),
                        ),
                        new_text: insert_text,
                    },
                );
                owner_instances.insert(owner_key, name.clone());
                name
            };

            match call_context.kind {
                CallContextKind::Statement => {
                    edits.add_edit(
                        ref_file_id,
                        TextEdit {
                            range: callee_range,
                            new_text: instance_name.to_string(),
                        },
                    );
                }
                CallContextKind::Expression => {
                    let output_name = context.output_name?;
                    let args_text = call_expr_args_text(&source, &call_expr);
                    let call_line =
                        format!("{}{}{};", call_context.indent, instance_name, args_text);
                    let statement_key = StatementKey {
                        file_id: ref_file_id,
                        range: call_context.stmt_range,
                    };
                    if inserted_calls.insert(statement_key) {
                        edits.add_edit(
                            ref_file_id,
                            TextEdit {
                                range: TextRange::new(
                                    TextSize::from(call_context.insert_offset as u32),
                                    TextSize::from(call_context.insert_offset as u32),
                                ),
                                new_text: build_prefix_insert_text(
                                    &source,
                                    call_context.insert_offset,
                                    &call_line,
                                ),
                            },
                        );
                    }
                    edits.add_edit(
                        ref_file_id,
                        TextEdit {
                            range: call_expr.text_range(),
                            new_text: format!("{instance_name}.{output_name}"),
                        },
                    );
                }
            }
        }
    }

    Some(())
}

fn function_block_has_type_references(db: &Database, owner_name: &str) -> bool {
    for file_id in db.file_ids() {
        let source = db.source_text(file_id);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(file_id);
        for name_node in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Name)
        {
            if !is_type_name_node(&name_node) {
                continue;
            }
            let Some(symbol_id) = resolve_type_symbol_at_node(&symbols, &root, &name_node) else {
                continue;
            };
            let Some(candidate) = symbol_qualified_name(&symbols, symbol_id) else {
                continue;
            };
            if candidate.eq_ignore_ascii_case(owner_name) {
                return true;
            }
        }
    }
    false
}

struct OutputVarInfo {
    name: SmolStr,
    type_name: String,
    removal_range: TextRange,
}

fn output_var_info(source: &str, _root: &SyntaxNode, node: &SyntaxNode) -> Option<OutputVarInfo> {
    let block = node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .find(|block| var_block_kind(block) == Some(SyntaxKind::KwVarOutput))?;
    let decls: Vec<_> = block
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarDecl)
        .collect();
    if decls.len() != 1 {
        return None;
    }
    let decl = decls.first()?;
    let names: Vec<_> = decl
        .children()
        .filter(|child| child.kind() == SyntaxKind::Name)
        .filter_map(|node| ident_token_in_name(&node))
        .collect();
    if names.len() != 1 {
        return None;
    }
    let ident = names.first()?;
    let type_node = decl
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)?;
    let type_name = text_for_range(source, type_node.text_range());
    if type_name.is_empty() {
        return None;
    }
    let removal_range = extend_range_to_line_end(source, block.text_range());
    Some(OutputVarInfo {
        name: SmolStr::new(ident.text()),
        type_name,
        removal_range,
    })
}

fn is_statement_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::AssignStmt
            | SyntaxKind::IfStmt
            | SyntaxKind::CaseStmt
            | SyntaxKind::ForStmt
            | SyntaxKind::WhileStmt
            | SyntaxKind::RepeatStmt
            | SyntaxKind::ReturnStmt
            | SyntaxKind::ExitStmt
            | SyntaxKind::ContinueStmt
            | SyntaxKind::JmpStmt
            | SyntaxKind::LabelStmt
            | SyntaxKind::ExprStmt
            | SyntaxKind::EmptyStmt
    )
}

fn find_enclosing_owner_node(
    root: &SyntaxNode,
    position: TextSize,
    kinds: &[SyntaxKind],
) -> Option<SyntaxNode> {
    let token = root.token_at_offset(position).right_biased()?;
    token
        .parent_ancestors()
        .find(|node| kinds.contains(&node.kind()))
}

fn initializer_expr_in_var_decl(var_decl: &SyntaxNode) -> Option<SyntaxNode> {
    var_decl
        .children()
        .find(|node| is_expression_kind(node.kind()))
}

fn inline_expr_info(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    expr: &SyntaxNode,
) -> Option<InlineExprInfo> {
    let text = text_for_range(source, expr.text_range());
    if text.is_empty() {
        return None;
    }
    let symbols = db.file_symbols_with_project(file_id);
    let context = ConstExprContext {
        db,
        file_id,
        source,
        root,
        symbols: &symbols,
    };
    let const_info = expression_const_info(&context, expr);
    Some(InlineExprInfo {
        text,
        kind: expr.kind(),
        is_const_expr: const_info.is_const,
        is_path_like: expression_is_path_like(expr),
        requires_local_scope: const_info.requires_local_scope,
    })
}

struct ConstExprContext<'a> {
    db: &'a Database,
    file_id: FileId,
    source: &'a str,
    root: &'a SyntaxNode,
    symbols: &'a SymbolTable,
}

struct ConstExprInfo {
    is_const: bool,
    requires_local_scope: bool,
}

fn expression_const_info(context: &ConstExprContext<'_>, expr: &SyntaxNode) -> ConstExprInfo {
    match expr.kind() {
        SyntaxKind::Literal => ConstExprInfo {
            is_const: true,
            requires_local_scope: false,
        },
        SyntaxKind::NameRef => name_ref_const_info(context, expr),
        SyntaxKind::FieldExpr => field_expr_const_info(context, expr),
        SyntaxKind::ParenExpr | SyntaxKind::UnaryExpr | SyntaxKind::BinaryExpr => {
            let mut requires_local_scope = false;
            for child in expr
                .children()
                .filter(|child| is_expression_kind(child.kind()))
            {
                let info = expression_const_info(context, &child);
                if !info.is_const {
                    return ConstExprInfo {
                        is_const: false,
                        requires_local_scope: false,
                    };
                }
                if info.requires_local_scope {
                    requires_local_scope = true;
                }
            }
            ConstExprInfo {
                is_const: true,
                requires_local_scope,
            }
        }
        _ => ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        },
    }
}

fn name_ref_const_info(context: &ConstExprContext<'_>, node: &SyntaxNode) -> ConstExprInfo {
    let offset = node.text_range().start();
    let target = resolve_target_at_position_with_context(
        context.db,
        context.file_id,
        offset,
        context.source,
        context.root,
        context.symbols,
    );
    let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let Some(symbol) = context.symbols.get(symbol_id) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    if !matches!(
        symbol.kind,
        SymbolKind::Constant | SymbolKind::EnumValue { .. }
    ) {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    }
    ConstExprInfo {
        is_const: true,
        requires_local_scope: symbol.parent.is_some(),
    }
}

fn field_expr_const_info(context: &ConstExprContext<'_>, node: &SyntaxNode) -> ConstExprInfo {
    let Some(parts) = qualified_name_from_field_expr(node) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let Some(symbol_id) = context.symbols.resolve_qualified(&parts) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let Some(symbol) = context.symbols.get(symbol_id) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    if !matches!(
        symbol.kind,
        SymbolKind::Constant | SymbolKind::EnumValue { .. }
    ) {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    }
    ConstExprInfo {
        is_const: true,
        requires_local_scope: false,
    }
}

fn expression_is_path_like(expr: &SyntaxNode) -> bool {
    match expr.kind() {
        SyntaxKind::NameRef | SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => true,
        SyntaxKind::ParenExpr => expr
            .children()
            .filter(|child| is_expression_kind(child.kind()))
            .any(|child| expression_is_path_like(&child)),
        _ => false,
    }
}

fn wrap_expression_for_inline(kind: SyntaxKind, expr_text: &str) -> String {
    match kind {
        SyntaxKind::Literal
        | SyntaxKind::NameRef
        | SyntaxKind::ParenExpr
        | SyntaxKind::FieldExpr
        | SyntaxKind::IndexExpr => expr_text.to_string(),
        _ => format!("({expr_text})"),
    }
}

fn reference_has_disallowed_context(
    db: &Database,
    file_id: FileId,
    range: TextRange,
    is_path_like: bool,
) -> bool {
    let source = db.source_text(file_id);
    let root = parse(&source).syntax();
    let Some(token) = root.token_at_offset(range.start()).right_biased() else {
        return false;
    };
    if token.text_range() != range {
        return false;
    }
    let name_ref = token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::NameRef);
    let Some(name_ref) = name_ref else {
        return false;
    };
    let Some(parent) = name_ref.parent() else {
        return false;
    };
    let is_base = parent
        .children()
        .next()
        .map(|child| child.text_range() == name_ref.text_range())
        .unwrap_or(false);
    match parent.kind() {
        SyntaxKind::CallExpr => is_base,
        SyntaxKind::AddrExpr | SyntaxKind::DerefExpr => is_base,
        SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => is_base && !is_path_like,
        _ => false,
    }
}

fn var_decl_removal_range(
    source: &str,
    root: &SyntaxNode,
    symbol_range: TextRange,
) -> Option<TextRange> {
    let var_decl = find_var_decl_for_range(root, symbol_range)?;
    let names: Vec<SyntaxToken> = var_decl
        .children()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .filter_map(|node| ident_token_in_name(&node))
        .collect();
    if names.is_empty() {
        return None;
    }
    let index = names
        .iter()
        .position(|token| token.text_range() == symbol_range)?;

    if names.len() == 1 {
        return Some(extend_range_to_line_end(source, var_decl.text_range()));
    }

    if index + 1 < names.len() {
        let end = names[index + 1].text_range().start();
        return Some(TextRange::new(symbol_range.start(), end));
    }

    let tokens: Vec<SyntaxToken> = var_decl
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .collect();
    let target_index = tokens
        .iter()
        .position(|token| token.text_range() == symbol_range)?;
    let comma = tokens[..target_index]
        .iter()
        .rev()
        .find(|token| token.kind() == SyntaxKind::Comma)?;

    let mut end = var_decl.text_range().end();
    for token in tokens.iter().skip(target_index + 1) {
        if token.kind().is_trivia() {
            end = token.text_range().end();
            continue;
        }
        end = token.text_range().start();
        break;
    }

    Some(TextRange::new(comma.text_range().start(), end))
}

fn find_var_decl_for_range(root: &SyntaxNode, symbol_range: TextRange) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|var_decl| {
            var_decl
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(|node| ident_token_in_name(&node))
                .any(|ident| ident.text_range() == symbol_range)
        })
}

fn extend_range_to_line_end(source: &str, range: TextRange) -> TextRange {
    let mut end = usize::from(range.end());
    let bytes = source.as_bytes();
    while end < bytes.len() {
        match bytes[end] {
            b'\n' => {
                end += 1;
                break;
            }
            b'\r' => {
                end += 1;
                if end < bytes.len() && bytes[end] == b'\n' {
                    end += 1;
                }
                break;
            }
            _ => end += 1,
        }
    }
    TextRange::new(range.start(), TextSize::from(end as u32))
}

fn text_for_range(source: &str, range: TextRange) -> String {
    utilities::text_for_range(source, range)
}

fn normalize_member_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_lowercase())
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Literal
            | SyntaxKind::NameRef
            | SyntaxKind::BinaryExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::DerefExpr
            | SyntaxKind::AddrExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::ThisExpr
            | SyntaxKind::SuperExpr
            | SyntaxKind::SizeOfExpr
    )
}

fn qualified_name_parts(node: &SyntaxNode) -> Vec<SmolStr> {
    utilities::qualified_name_parts(node)
}

fn path_eq_ignore_ascii_case(a: &[SmolStr], b: &[SmolStr]) -> bool {
    utilities::path_eq_ignore_ascii_case(a, b)
}

fn path_starts_with_ignore_ascii_case(path: &[SmolStr], prefix: &[SmolStr]) -> bool {
    utilities::path_starts_with_ignore_ascii_case(path, prefix)
}

fn join_namespace_path(parts: &[SmolStr]) -> String {
    utilities::join_namespace_path(parts)
}

fn node_token_range(node: &SyntaxNode) -> text_size::TextRange {
    utilities::node_token_range(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_namespace_updates_using_and_qualified_names() {
        let source = r#"
NAMESPACE LibA
TYPE Foo : INT;
END_TYPE
FUNCTION FooFunc : INT
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    USING LibA;
    VAR
        x : LibA.Foo;
    END_VAR
    x := LibA.FooFunc();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let old_path = parse_namespace_path("LibA").expect("old path");
        let new_path = parse_namespace_path("Company.LibA").expect("new path");

        let result = move_namespace_path(&db, &old_path, &new_path).expect("rename result");
        let edits = result.edits.get(&file_id).expect("file edits");

        let using_edit = edits
            .iter()
            .find(|edit| edit.new_text == "Company.LibA")
            .expect("using edit");
        let using_start: usize = using_edit.range.start().into();
        let using_end: usize = using_edit.range.end().into();
        assert!(source[using_start..using_end].contains("LibA"));

        assert!(edits.iter().any(|edit| edit.new_text == "Company.LibA.Foo"));

        let qualified_edit = edits
            .iter()
            .find(|edit| edit.new_text == "Company.LibA.FooFunc")
            .expect("field expr edit");
        let qualified_start: usize = qualified_edit.range.start().into();
        let qualified_end: usize = qualified_edit.range.end().into();
        assert!(source[qualified_start..qualified_end].contains("LibA.FooFunc"));
    }

    #[test]
    fn generate_interface_stubs_inserts_missing_members() {
        let source = r#"
INTERFACE IControl
    METHOD Start
    END_METHOD

    PROPERTY Status : INT
        GET
        END_GET
    END_PROPERTY
END_INTERFACE

CLASS Pump IMPLEMENTS IControl
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("IMPLEMENTS IControl").expect("implements");
        let result =
            generate_interface_stubs(&db, file_id, TextSize::from(offset as u32)).expect("stubs");
        let edits = result.edits.get(&file_id).expect("file edits");
        let insert = edits
            .iter()
            .find(|edit| !edit.new_text.is_empty())
            .expect("insert edit");
        assert!(insert.new_text.contains("METHOD PUBLIC Start"));
        assert!(insert.new_text.contains("PROPERTY PUBLIC Status"));
    }

    #[test]
    fn inline_variable_with_literal_initializer() {
        let source = r#"
PROGRAM Test
    VAR
        x : INT := 1 + 2;
    END_VAR
    y := x;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("x;").expect("x ref");
        let result = inline_symbol(&db, file_id, TextSize::from(offset as u32)).expect("inline");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text.contains("1 + 2")));
        assert!(edits.iter().any(|edit| edit.new_text.is_empty()));
    }

    #[test]
    fn inline_constant_across_files() {
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

        let offset = program.find("ANSWER").expect("constant ref");
        let target = resolve_target_at_position(&db, prog_id, TextSize::from(offset as u32))
            .expect("target");
        let ResolvedTarget::Symbol(symbol_id) = target else {
            panic!("expected symbol target");
        };
        let symbols = db.file_symbols_with_project(prog_id);
        let symbol = symbols.get(symbol_id).expect("symbol");
        let origin = symbol.origin.expect("origin");
        let decl_file_id = origin.file_id;
        assert_eq!(decl_file_id, const_id);
        let decl_source = db.source_text(decl_file_id);
        let decl_root = parse(&decl_source).syntax();
        let decl_range = db
            .file_symbols(origin.file_id)
            .get(origin.symbol_id)
            .map(|sym| sym.range)
            .unwrap_or(symbol.range);
        let var_decl = find_var_decl_for_range(&decl_root, decl_range).expect("var decl");
        let expr = initializer_expr_in_var_decl(&var_decl).expect("initializer");
        let expr_info =
            inline_expr_info(&db, decl_file_id, &decl_source, &decl_root, &expr).expect("expr");
        assert!(expr_info.is_const_expr);
        let references = find_references(
            &db,
            prog_id,
            TextSize::from(offset as u32),
            FindReferencesOptions {
                include_declaration: false,
            },
        );
        assert!(!references.is_empty(), "references");

        let result = inline_symbol(&db, prog_id, TextSize::from(offset as u32)).expect("inline");
        let const_edits = result.edits.edits.get(&const_id).expect("const edits");
        let prog_edits = result.edits.edits.get(&prog_id).expect("program edits");
        assert!(prog_edits.iter().any(|edit| edit.new_text == "42"));
        assert!(const_edits.iter().any(|edit| edit.new_text.is_empty()));
    }

    #[test]
    fn extract_method_creates_method_and_call() {
        let source = r#"
CLASS Controller
    METHOD Run
        VAR
            x : INT;
            y : INT;
        END_VAR
        x := 1;
        y := x + 1;
    END_METHOD
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x := 1;").expect("start");
        let end = source.find("y := x + 1;").expect("end") + "y := x + 1;".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_method(&db, file_id, range).expect("extract method");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("METHOD ExtractedMethod")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_IN_OUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedMethod(x := x")));
    }

    #[test]
    fn extract_property_creates_property() {
        let source = r#"
CLASS Controller
    VAR
        speed : INT;
    END_VAR
    METHOD Run
        VAR
            x : INT;
        END_VAR
        x := speed + 1;
    END_METHOD
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("speed + 1").expect("start");
        let end = start + "speed + 1".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_property(&db, file_id, range).expect("extract property");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("PROPERTY ExtractedProperty")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedProperty := speed + 1")));
    }

    #[test]
    fn extract_pou_creates_function() {
        let source = r#"
PROGRAM Main
    VAR
        x : INT;
        y : INT;
    END_VAR
    x := 1;
    y := x + 1;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x := 1;").expect("start");
        let end = source.find("y := x + 1;").expect("end") + "y := x + 1;".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_pou(&db, file_id, range).expect("extract pou");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION ExtractedFunction")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_IN_OUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction(x := x")));
    }

    #[test]
    fn extract_pou_expression_infers_return_type() {
        let source = r#"
PROGRAM Main
    VAR
        x : INT;
        y : INT;
    END_VAR
    y := x + 1;
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let start = source.find("x + 1").expect("start");
        let end = start + "x + 1".len();
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let result = extract_pou(&db, file_id, range).expect("extract pou");
        let edits = result.edits.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION ExtractedFunction : INT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction := x + 1")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("ExtractedFunction(x := x)")));
    }

    #[test]
    fn convert_function_to_function_block_updates_calls() {
        let source = r#"
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION

PROGRAM Main
    Foo();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION Foo").expect("function");
        let result =
            convert_function_to_function_block(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FUNCTION_BLOCK")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR_OUTPUT")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance")));
    }

    #[test]
    fn convert_function_to_function_block_updates_expression_calls() {
        let source = r#"
NAMESPACE LibA
FUNCTION Foo : INT
    Foo := 1;
END_FUNCTION
END_NAMESPACE

PROGRAM Main
    VAR
        x : INT;
    END_VAR
    x := LibA.Foo();
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION Foo").expect("function");
        let result =
            convert_function_to_function_block(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("VAR") && edit.new_text.contains("LibA.Foo")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance.result")));
        assert!(edits
            .iter()
            .any(|edit| edit.new_text.contains("FooInstance(")));
    }

    #[test]
    fn convert_function_block_to_function_updates_signature() {
        let source = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let offset = source.find("FUNCTION_BLOCK Fb").expect("function block");
        let result =
            convert_function_block_to_function(&db, file_id, TextSize::from(offset as u32))
                .expect("convert");
        let edits = result.edits.get(&file_id).expect("file edits");
        assert!(edits.iter().any(|edit| edit.new_text.contains("FUNCTION")));
        assert!(edits.iter().any(|edit| edit.new_text.contains(": INT")));
    }

    #[test]
    fn convert_function_block_to_function_requires_no_instances() {
        let fb = r#"
FUNCTION_BLOCK Fb
    VAR_OUTPUT
        result : INT;
    END_VAR
    result := 1;
END_FUNCTION_BLOCK
"#;
        let program = r#"
PROGRAM Main
    VAR
        fb : Fb;
    END_VAR
    fb();
END_PROGRAM
"#;
        let mut db = Database::new();
        let fb_id = FileId(0);
        let program_id = FileId(1);
        db.set_source_text(fb_id, fb.to_string());
        db.set_source_text(program_id, program.to_string());

        let offset = fb.find("FUNCTION_BLOCK Fb").expect("function block");
        let result = convert_function_block_to_function(&db, fb_id, TextSize::from(offset as u32));
        assert!(result.is_none(), "expected conversion to be unavailable");
    }
}
