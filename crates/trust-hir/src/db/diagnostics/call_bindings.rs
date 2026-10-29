use super::super::*;
use super::is_expression_kind;

/// Reports each named actual without a value (`Value := )`, `Done =>,`) unless the project uses the
/// CODESYS profile. A formal call assigns a value to an input with `:=` and an output to a variable
/// with `=>` (IEC 61131-3 Ed.3 §6.6.1.4.2); CODESYS reads an empty one as a parameter left unbound
/// or an output discarded (docs/IEC_DEVIATIONS.md DEV-032). The parser accepts both, so the error
/// is reported here, where it was before: at the delimiter after the operator.
pub(in crate::db) fn check_empty_named_actuals(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    if symbols.dialect().is_codesys() {
        return;
    }
    for arg in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Arg)
    {
        let Some(operator) = arg
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| matches!(token.kind(), SyntaxKind::Assign | SyntaxKind::Arrow))
        else {
            continue;
        };
        if arg.children().any(|child| is_expression_kind(child.kind())) {
            continue;
        }
        let mut next = operator.next_token();
        while let Some(token) = next.as_ref().filter(|token| token.kind().is_trivia()) {
            next = token.next_token();
        }
        let at = next.map_or(operator.text_range().end(), |token| {
            token.text_range().start()
        });
        diagnostics.error(
            DiagnosticCode::UnexpectedToken,
            TextRange::empty(at),
            "expected expression",
        );
    }
}
