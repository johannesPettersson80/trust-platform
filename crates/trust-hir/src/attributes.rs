//! The attributes a declaration carries in the pragma before it, `{attribute 'name'}`.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::openot_authoring::{parse_attribute_entries_from_text, AttributeEntry};

/// The refusal of a bare value `name` of the enumeration `type_name`, declared
/// `{attribute 'qualified_only'}` (specification 02 §3.1), which names the qualified form.
pub(crate) fn unqualified_value_message(name: &str, type_name: &str) -> String {
    format!(
        "'{name}' is a value of {type_name}, which is {{attribute 'qualified_only'}}: write {type_name}.{name}"
    )
}

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
    attribute_entry(node, key).is_some()
}

/// Whether the `VAR_GLOBAL` list `list` carries the flag `key`: in the pragma before the list, or in
/// the pragma before the innermost `NAMESPACE` that holds the list. A CODESYS global variable list
/// arrives as such a namespace with its attributes before it (specification 22: the PLCopen import),
/// so a list's `qualified_only` (specification 04, "Rules", 7) and `cross_task` (specification 28
/// section 6) keep their meaning there.
#[must_use]
pub fn list_declares_attribute(list: &SyntaxNode, key: &str) -> bool {
    declares_attribute(list, key)
        || list
            .ancestors()
            .find(|node| node.kind() == SyntaxKind::Namespace)
            .is_some_and(|namespace| declares_attribute(&namespace, key))
}

/// The entry `key` of the pragma trivia preceding `node`'s first token, with its value and the pragma's range, found as
/// [`declares_attribute`] finds it; `None` when no such pragma names `key`.
#[must_use]
pub fn attribute_entry(node: &SyntaxNode, key: &str) -> Option<AttributeEntry> {
    let read = |token: &SyntaxToken| {
        if token.kind() != SyntaxKind::Pragma {
            return None;
        }
        parse_attribute_entries_from_text(token.text(), token.text_range())
            .into_iter()
            .find(|entry| entry.key == key)
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
        if let Some(entry) = read(&token) {
            return Some(entry);
        }
    }
    let keyword = first?;
    // The trivia immediately preceding that token, however the parser
    // nested it: backwards over trivia tokens, then through the trailing
    // trivia of the construct the parser parked the pragma in — the previous
    // POU or declaration, or the namespace head. Rowan's sibling walk is
    // parent-scoped, so the walk climbs parents to stay in document order.
    let mut cursor = rowan::NodeOrToken::Token(keyword);
    loop {
        let prev = prev_in_document_order(cursor.clone())?;
        match prev {
            rowan::NodeOrToken::Token(token) => {
                if !token.kind().is_trivia() {
                    return None;
                }
                if let Some(entry) = read(&token) {
                    return Some(entry);
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
                    if let Some(entry) = read(&token) {
                        return Some(entry);
                    }
                }
                return None;
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
