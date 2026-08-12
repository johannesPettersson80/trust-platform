use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

pub(crate) fn contains_textual_action(syntax: &SyntaxNode) -> bool {
    syntax
        .descendants()
        .any(|node| node.kind() == SyntaxKind::Action)
}
