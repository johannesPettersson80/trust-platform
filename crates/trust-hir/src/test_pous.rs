//! Which declarations are tests (specification 22, "Test POU discovery"): a `TEST_PROGRAM`, a
//! `TEST_FUNCTION_BLOCK`, and an ordinary `PROGRAM` whose declaration carries
//! `{attribute 'test'}`, the form of a test that a CODESYS project can hold. Test discovery, the
//! build that leaves test programs unbound, unused-POU analysis and the language server's run
//! entry all ask this one rule.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::attributes::declares_attribute;

/// The kind of a test declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    /// A test program: `TEST_PROGRAM`, or a `PROGRAM` with `{attribute 'test'}`.
    Program,
    /// `TEST_FUNCTION_BLOCK`.
    FunctionBlock,
}

impl TestKind {
    /// The name of the kind wherever a test is reported: `TEST_PROGRAM` or `TEST_FUNCTION_BLOCK`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Program => "TEST_PROGRAM",
            Self::FunctionBlock => "TEST_FUNCTION_BLOCK",
        }
    }
}

/// The test kind of the POU declaration `node`, or `None` when it declares no test. The attribute
/// counts in the pragma before the `PROGRAM` keyword, as [`declares_attribute`] reads it; before a
/// function block or any other declaration, and inside a body, it declares no test.
#[must_use]
pub fn test_kind(node: &SyntaxNode) -> Option<TestKind> {
    let keyword = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())?;
    match (node.kind(), keyword.kind()) {
        (SyntaxKind::Program, SyntaxKind::KwTestProgram) => Some(TestKind::Program),
        (SyntaxKind::Program, SyntaxKind::KwProgram) if declares_attribute(node, "test") => {
            Some(TestKind::Program)
        }
        (SyntaxKind::FunctionBlock, SyntaxKind::KwTestFunctionBlock) => {
            Some(TestKind::FunctionBlock)
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "test_pous/contract_tests.rs"]
mod contract_tests;
