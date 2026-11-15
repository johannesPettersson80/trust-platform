//! The declarations of Rust POUs (specification 48 RU-23, RU-34): a Rust POU holds no statements,
//! actions, properties or base, and `{attribute 'rust'}` marks only a function, function block or
//! program.

use trust_syntax::syntax::SyntaxNode;

use crate::diagnostics::{DiagnosticBuilder, DiagnosticCode};
use crate::rust_pous::rust_pou_errors;

/// Reports each rule of a Rust POU that the declarations of `root` break (E310).
pub(in crate::db) fn check_rust_pous(root: &SyntaxNode, builder: &mut DiagnosticBuilder) {
    for (range, message) in rust_pou_errors(root) {
        builder.error(DiagnosticCode::InvalidRustPou, range, message);
    }
}
