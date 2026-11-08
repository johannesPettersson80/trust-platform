//! The attributes a declaration carries in the pragma before it, `{attribute 'name'}`.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::openot_authoring::parse_attribute_entries_from_text;

/// Whether the pragma trivia preceding `node`'s first token carries the flag
/// `key`: a POU's `reads_host_clock` (specification 07, "Declared Clock
/// Sources"), a global's or a `VAR_GLOBAL` list's `cross_task` (specification
/// 28 section 6), a program's `test` (specification 22, "Test POU discovery"). The scan walks backwards over trivia and over the trailing
/// trivia of the element the parser parked it in: a pragma before a
/// declaration may sit in the leading trivia of its node, in the trailing
/// trivia of the preceding construct (the previous POU or declaration in a
/// file, the namespace head), so the first token's immediate trivia is what
/// counts. A pragma behind that token — inside a body or a list — never
/// declares the node.
pub fn declares_attribute(node: &SyntaxNode, key: &str) -> bool {
    let reads = |token: &SyntaxToken| {
        token.kind() == SyntaxKind::Pragma
            && parse_attribute_entries_from_text(token.text(), token.text_range())
                .iter()
                .any(|entry| entry.key == key)
    };
    // The trivia the node itself holds before its first token — the whole
    // leading trivia when the declaration opens a file or follows a
    // construct the parser closed cleanly — however deep the parser nested
    // that token, as a declaration may wrap its name in a node of its own.
    let mut first = None;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if !token.kind().is_trivia() {
            first = Some(token);
            break;
        }
        if reads(&token) {
            return true;
        }
    }
    let Some(keyword) = first else {
        return false;
    };
    // The trivia immediately preceding that token, however the parser
    // nested it: backwards over trivia tokens, then through the trailing
    // trivia of the construct the parser parked the pragma in — the previous
    // POU or declaration, or the namespace head. Rowan's sibling walk is
    // parent-scoped, so the walk climbs parents to stay in document order.
    let mut cursor = rowan::NodeOrToken::Token(keyword);
    loop {
        let Some(prev) = prev_in_document_order(cursor.clone()) else {
            return false;
        };
        match prev {
            rowan::NodeOrToken::Token(token) => {
                if !token.kind().is_trivia() {
                    return false;
                }
                if reads(&token) {
                    return true;
                }
                cursor = rowan::NodeOrToken::Token(token);
            }
            rowan::NodeOrToken::Node(node) => {
                // The previous construct swallowed the trivia that precedes
                // this declaration as its own trailing trivia. Look only
                // there: beyond its last non-trivia token lies an earlier
                // construct, separated from this declaration by a keyword.
                let children: Vec<_> = node.children_with_tokens().collect();
                for element in children.into_iter().rev() {
                    let Some(token) = element.into_token() else {
                        break;
                    };
                    if !token.kind().is_trivia() {
                        break;
                    }
                    if reads(&token) {
                        return true;
                    }
                }
                return false;
            }
        }
    }
}

/// The previous element in document order: the sibling walk inside one
/// parent, climbing through the parent chain when the element is its
/// parent's first child.
fn prev_in_document_order(
    element: rowan::NodeOrToken<SyntaxNode, SyntaxToken>,
) -> Option<rowan::NodeOrToken<SyntaxNode, SyntaxToken>> {
    if let Some(prev) = element.prev_sibling_or_token() {
        return Some(prev);
    }
    element
        .parent()
        .map(rowan::NodeOrToken::Node)
        .and_then(prev_in_document_order)
}
