//! Which declarations are Rust POUs (specification 48 RU-23): a `FUNCTION`, `FUNCTION_BLOCK` or
//! `PROGRAM` whose declaration carries `{attribute 'rust'}`, as the build's generated declarations
//! mark them. Its body is a Rust item of the project's crate, so its declaration holds no
//! statements, and the checks that read a body, of an unassigned result or of unused inputs, do not
//! apply to it. The symbol table marks it (RU-31), and the build, the analysis and the tools all ask
//! this one rule.

use text_size::TextRange;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::attributes::{attribute_entry, declares_attribute};

/// The attribute that marks a Rust POU, `{attribute 'rust'}`.
pub const RUST_ATTRIBUTE: &str = "rust";

/// Whether the declaration `node` is a Rust POU: a function, function block or program whose
/// declaration carries `{attribute 'rust'}`, in the pragma before its keyword as
/// [`declares_attribute`] reads it.
#[must_use]
pub fn is_rust_pou(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Function | SyntaxKind::FunctionBlock | SyntaxKind::Program
    ) && declares_attribute(node, RUST_ATTRIBUTE)
}

/// Whether the declaration `node` is a Rust POU or lies in one, as a method of a Rust function
/// block does (RU-32): its body is Rust code too, so the checks that read a body do not apply.
#[must_use]
pub fn is_in_rust_pou(node: &SyntaxNode) -> bool {
    node.ancestors().any(|ancestor| is_rust_pou(&ancestor))
}

/// The declarations that may carry a pragma and are no POU a Rust item can be.
const OTHER_DECLARATIONS: [SyntaxKind; 11] = [
    SyntaxKind::Class,
    SyntaxKind::Method,
    SyntaxKind::Property,
    SyntaxKind::Interface,
    SyntaxKind::Action,
    SyntaxKind::Namespace,
    SyntaxKind::Configuration,
    SyntaxKind::Resource,
    SyntaxKind::TypeDecl,
    SyntaxKind::VarBlock,
    SyntaxKind::VarDecl,
];

/// What the declarations of `root` break of specification 48's rules for Rust POUs, each with its
/// range and message: a statement in a Rust POU or in one of its methods (RU-23), an action or a
/// property of one and an `EXTENDS` of one (RU-34), and `{attribute 'rust'}` on a declaration that
/// is no function, function block or program.
#[must_use]
pub fn rust_pou_errors(root: &SyntaxNode) -> Vec<(TextRange, String)> {
    let mut errors = Vec::new();
    for node in root.descendants() {
        if is_rust_pou(&node) {
            let name = pou_name(&node);
            errors.extend(body_errors(&node, &name));
        } else if OTHER_DECLARATIONS.contains(&node.kind())
            && let Some(entry) = attribute_entry(&node, RUST_ATTRIBUTE)
        {
            errors.push((
                entry.range,
                "`{attribute 'rust'}` marks a function, function block or program whose body is a \
                 Rust item (specification 48 RU-23)"
                    .to_string(),
            ));
        }
    }
    errors
}

/// The errors of the Rust POU `pou`, named `name`.
fn body_errors(pou: &SyntaxNode, name: &str) -> Vec<(TextRange, String)> {
    let mut errors = Vec::new();
    for child in pou.children() {
        match child.kind() {
            SyntaxKind::Action | SyntaxKind::Property => errors.push((
                child.text_range(),
                format!(
                    "{name} is a Rust POU: it has no actions or properties, which a method gives \
                     instead (specification 48 RU-34)"
                ),
            )),
            SyntaxKind::ExtendsClause => errors.push((
                child.text_range(),
                format!(
                    "{name} is a Rust POU: it extends no function block or class, whose bodies \
                     and methods a body in Rust cannot join (specification 48 RU-34)"
                ),
            )),
            _ => {}
        }
    }
    // The statements of the POU's body and of its methods' bodies: a Rust POU's body is its Rust
    // item, and a method of it is a Rust method (RU-32).
    let statement = pou
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::StmtList)
        .filter(|list| {
            list.parent()
                .is_some_and(|parent| parent == *pou || parent.kind() == SyntaxKind::Method)
        })
        .find_map(|list| list.children().next());
    if let Some(statement) = statement {
        errors.push((
            statement.text_range(),
            format!(
                "{name} is a Rust POU (`{{attribute 'rust'}}`): its body is its Rust item, so its \
                 declaration holds no statements (specification 48 RU-23)"
            ),
        ));
    }
    errors
}

/// The declared name of the POU `node`, as written.
fn pou_name(node: &SyntaxNode) -> String {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| name.text().to_string().trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "rust_pous_tests.rs"]
mod tests;
