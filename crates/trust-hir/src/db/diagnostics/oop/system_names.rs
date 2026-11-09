//! CODESYS's system names: the system interface `__SYSTEM.IQueryInterface` in an EXTENDS or
//! IMPLEMENTS list (specification 04), which marks an interface for `__QUERYINTERFACE` and adds
//! nothing here, so the analysis skips it like a name it never reads and refuses only what CODESYS
//! refuses too; and the system type `__SYSTEM.ExceptionCode` in a declaration (specification 06
//! §15).

use super::super::super::*;

/// The one system interface truST accepts.
const QUERY_INTERFACE: &str = "IQueryInterface";

/// Refuses a `__SYSTEM` name other than `IQueryInterface` in an EXTENDS or IMPLEMENTS list,
/// `__SYSTEM.IQueryInterface` as the base a function block or class extends, and a type other than
/// `__SYSTEM.ExceptionCode`.
pub(super) fn check_system_names(root: &SyntaxNode, diagnostics: &mut DiagnosticBuilder) {
    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::SystemName)
    {
        // A name missing after `__SYSTEM.` is the parser's error already.
        let Some(name) = node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::Ident)
        else {
            continue;
        };
        if node
            .parent()
            .is_some_and(|parent| parent.kind() == SyntaxKind::TypeRef)
        {
            if crate::symbols::system_type_name(name.text()).is_none() {
                diagnostics.error(
                    DiagnosticCode::UndefinedType,
                    node.text_range(),
                    format!(
                        "unknown system type '__SYSTEM.{}': truST knows __SYSTEM.ExceptionCode",
                        name.text()
                    ),
                );
            }
            continue;
        }
        if !name.text().eq_ignore_ascii_case(QUERY_INTERFACE) {
            diagnostics.error(
                DiagnosticCode::UndefinedType,
                node.text_range(),
                format!(
                    "unknown system interface '__SYSTEM.{}': truST knows __SYSTEM.{QUERY_INTERFACE}",
                    name.text()
                ),
            );
            continue;
        }
        let clause = node.parent();
        let owner = clause.as_ref().and_then(SyntaxNode::parent);
        if clause.is_some_and(|clause| clause.kind() == SyntaxKind::ExtendsClause)
            && owner.is_some_and(|owner| owner.kind() != SyntaxKind::Interface)
        {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                node.text_range(),
                format!(
                    "only an INTERFACE EXTENDS __SYSTEM.{QUERY_INTERFACE}; a FUNCTION_BLOCK or CLASS IMPLEMENTS it"
                ),
            );
        }
    }
}
