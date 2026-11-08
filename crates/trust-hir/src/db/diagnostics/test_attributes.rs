//! The test attributes of a file's declarations (specification 22, "Multi-cycle tests"): `test_cycle` and `test_max_cycles` must
//! read, and only a test program carries them.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::diagnostics::{DiagnosticBuilder, DiagnosticCode};
use crate::test_pous::test_attribute_errors;

/// Reports each test attribute of the POUs of `root` that does not read or belongs to no test program (E309).
pub(in crate::db) fn check_test_attributes(root: &SyntaxNode, builder: &mut DiagnosticBuilder) {
    for node in root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::Program | SyntaxKind::FunctionBlock | SyntaxKind::Function
        )
    }) {
        for (range, message) in test_attribute_errors(&node) {
            builder.error(DiagnosticCode::InvalidTestAttribute, range, message);
        }
    }
}
