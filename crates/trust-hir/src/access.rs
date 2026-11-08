//! How a use of a name reaches what it names: a read, a write or a call (specification 28 §6). The
//! one rule for W012's cross-task check and for the access kind of each reference (specification
//! 14 §6.3.2).

use rustc_hash::FxHashSet;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

/// The way a use reaches what it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    /// The use reads it.
    Read,
    /// The use writes it: the target of an assignment or of a FOR loop's control variable, or an
    /// argument that a call binds to an output parameter (`=>`) or an in-out parameter.
    Write,
    /// The use calls it: a call's callee, or the instance whose body or method a call runs, which
    /// the call may change.
    Call,
}

/// The access of `node`, a name, a member access or a FOR loop's control variable. A field,
/// element or dereference of a value reaches the value itself, so `gArr[1] := 0` writes `gArr`,
/// while its index `i` in `gArr[i] := 0` is read. `written_arguments` holds the argument
/// expressions of the node's file that its calls bind to output and in-out parameters, by text
/// range (`FileAnalysis::written_arguments`).
#[must_use]
pub fn access_of(node: &SyntaxNode, written_arguments: &FxHashSet<(u32, u32)>) -> Access {
    let mut current = node.clone();
    loop {
        let range = current.text_range();
        if written_arguments.contains(&(u32::from(range.start()), u32::from(range.end()))) {
            return Access::Write;
        }
        let Some(parent) = current.parent() else {
            return Access::Read;
        };
        let first_child = parent.first_child();
        match parent.kind() {
            SyntaxKind::AssignStmt if first_child.as_ref() == Some(&current) => {
                return Access::Write;
            }
            SyntaxKind::ForStmt if current.kind() == SyntaxKind::Name => return Access::Write,
            SyntaxKind::CallExpr if first_child.as_ref() == Some(&current) => {
                return Access::Call;
            }
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr | SyntaxKind::DerefExpr
                if first_child.as_ref() == Some(&current) =>
            {
                current = parent;
            }
            _ => return Access::Read,
        }
    }
}

#[cfg(test)]
#[path = "access/contract_tests.rs"]
mod contract_tests;
