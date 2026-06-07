//! Inlay hints for Structured Text.
//!
//! Provides parameter name hints for positional call arguments.

use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::openot_authoring::{self as openot_vocab, OotKind};
use trust_hir::symbols::ParamDirection;
use trust_hir::Database;
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::signature_help::signature_for_call_expr;
use crate::util::name_from_name_node;

/// Kinds of inlay hints produced by trust-ide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// Parameter name hint.
    Parameter,
    /// OpenOT telemetry emission hint.
    Telemetry,
}

/// A single inlay hint in ST source.
#[derive(Debug, Clone)]
pub struct InlayHint {
    /// The hint position.
    pub position: TextSize,
    /// The hint label text.
    pub label: SmolStr,
    /// The hint kind.
    pub kind: InlayHintKind,
}

/// Computes inlay hints within a source range.
pub fn inlay_hints(db: &Database, file_id: FileId, range: TextRange) -> Vec<InlayHint> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();

    let mut hints = Vec::new();
    hints.extend(openot_attribute_hints(&root, range));
    for call_expr in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::CallExpr)
    {
        if !range_intersects(call_expr.text_range(), range) {
            continue;
        }

        let arg_list = call_expr
            .children()
            .find(|child| child.kind() == SyntaxKind::ArgList);
        let Some(arg_list) = arg_list else {
            continue;
        };

        let args = collect_call_args(&arg_list);
        if args.is_empty() {
            continue;
        }

        let Some(signature) = signature_for_call_expr(db, file_id, &source, &root, &call_expr)
        else {
            continue;
        };

        for (index, arg) in args.iter().enumerate() {
            if arg.name.is_some() {
                continue;
            }
            let Some(param) = signature.params.get(index) else {
                break;
            };
            if is_execution_param(&param.name) {
                continue;
            }
            if !range.contains(arg.range.start()) {
                continue;
            }

            let op = match param.direction {
                ParamDirection::Out => "=>",
                ParamDirection::In | ParamDirection::InOut => ":=",
            };
            let label = SmolStr::new(format!("{} {}", param.name, op));
            hints.push(InlayHint {
                position: arg.range.start(),
                label,
                kind: InlayHintKind::Parameter,
            });
        }
    }

    hints
}

fn openot_attribute_hints(root: &SyntaxNode, range: TextRange) -> Vec<InlayHint> {
    let mut hints = Vec::new();
    for var_decl in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        if !range_intersects(var_decl.text_range(), range) {
            continue;
        }
        let attrs = openot_vocab::parse_attribute_map_from_node(&var_decl);
        let Some(kind) = attrs.kind() else {
            continue;
        };
        let Some(type_ref) = var_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        else {
            continue;
        };
        let Some(label) = openot_hint_label(kind, &attrs) else {
            continue;
        };
        hints.push(InlayHint {
            position: type_ref.text_range().end(),
            label: SmolStr::new(label),
            kind: InlayHintKind::Telemetry,
        });
    }
    hints
}

fn openot_hint_label(kind: OotKind, attrs: &openot_vocab::AttributeMap) -> Option<String> {
    match kind {
        OotKind::Value => {
            let mut label = if let Some(deadband) = attrs.get("deadband") {
                format!("ValueChanged on delta>{deadband}")
            } else {
                "ValueChanged on change".to_string()
            };
            if let Some(unit) = attrs.get("unit") {
                label.push(' ');
                label.push_str(unit);
            }
            Some(label)
        }
        OotKind::State => Some("StateTransition on change".to_string()),
        OotKind::Alarm => Some("ConditionActive/Cleared on edge".to_string()),
        OotKind::Message => Some("Message on TRUE edge".to_string()),
    }
}

#[derive(Debug, Clone)]
struct ArgInfo {
    name: Option<SmolStr>,
    range: TextRange,
}

fn collect_call_args(arg_list: &SyntaxNode) -> Vec<ArgInfo> {
    let mut args = Vec::new();
    for arg in arg_list.children().filter(|n| n.kind() == SyntaxKind::Arg) {
        let name = arg
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|child| name_from_name_node(&child));
        args.push(ArgInfo {
            name,
            range: arg.text_range(),
        });
    }
    args
}

fn range_intersects(a: TextRange, b: TextRange) -> bool {
    a.start() < b.end() && b.start() < a.end()
}

fn is_execution_param(name: &str) -> bool {
    name.eq_ignore_ascii_case("EN") || name.eq_ignore_ascii_case("ENO")
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_hir::db::{Database, FileId, SourceDatabase};

    #[test]
    fn inlay_hints_provide_parameter_names_for_positional_args() {
        let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
END_PROGRAM
"#;
        let start = source.find("Add(1").expect("call");
        let end = source.find(");").expect("call end");
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let hints = inlay_hints(&db, file_id, range);
        assert_eq!(hints.len(), 2);
        assert!(hints
            .iter()
            .any(|hint| hint.label.as_str().starts_with("A")));
        assert!(hints
            .iter()
            .any(|hint| hint.label.as_str().starts_with("B")));
    }

    #[test]
    fn inlay_hints_allow_named_args_after_positional() {
        let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, B := 2);
END_PROGRAM
"#;
        let start = source.find("Add(1").expect("call");
        let end = source.find(");").expect("call end");
        let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));

        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let hints = inlay_hints(&db, file_id, range);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].label.as_str().starts_with("A"));
    }
}
