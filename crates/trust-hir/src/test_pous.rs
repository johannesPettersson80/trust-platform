//! Which declarations are tests (specification 22, "Test POU discovery"): a `TEST_PROGRAM`, a
//! `TEST_FUNCTION_BLOCK`, and an ordinary `PROGRAM` whose declaration carries
//! `{attribute 'test'}`, the form of a test that a CODESYS project can hold. Test discovery, the
//! build that leaves test programs unbound, unused-POU analysis and the language server's run
//! entry all ask this one rule.

use text_size::TextRange;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::attributes::{attribute_entry, declares_attribute};

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

/// The cycle a multi-cycle test runs at when it names none: 10 ms (specification 22, "Multi-cycle tests").
pub const DEFAULT_TEST_CYCLE_NS: u64 = 10_000_000;
/// The cycles a multi-cycle test may run when it names no budget.
pub const DEFAULT_TEST_MAX_CYCLES: u32 = 10_000;
/// The longest cycle a test may name: one hour.
const MAX_TEST_CYCLE_NS: u64 = 3_600_000_000_000;
/// The largest budget a test may name.
const MAX_TEST_MAX_CYCLES: u32 = 10_000_000;

/// The cycles of a multi-cycle test (specification 22, "Multi-cycle tests"): the cycle, by which its time advances before each
/// later cycle, the budget, the most cycles it may run, and whether the project's simulation runs in lockstep with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestCycles {
    /// The cycle, in nanoseconds.
    pub cycle_ns: u64,
    /// The most cycles the test runs.
    pub max_cycles: u32,
    /// `{attribute 'test_simulation'}`: the project's `simulation.toml` runs in lockstep with the test, its plants publishing
    /// before each cycle and advancing after it.
    pub simulation: bool,
}

/// The cycles of the test program `node`, or `None` when it runs its body once: a test program is multi-cycle when its body calls
/// `TEST_FINISHED()` or its declaration carries `{attribute 'test_cycle'}`, `{attribute 'test_max_cycles'}` or `{attribute
/// 'test_simulation'}`. A value that does not read leaves the default in its place; [`test_attribute_errors`] reports it, and the
/// compile refuses it.
#[must_use]
pub fn test_cycles(node: &SyntaxNode) -> Option<TestCycles> {
    if test_kind(node) != Some(TestKind::Program) {
        return None;
    }
    let cycle = attribute_entry(node, "test_cycle");
    let budget = attribute_entry(node, "test_max_cycles");
    let simulation = attribute_entry(node, "test_simulation").is_some();
    if cycle.is_none() && budget.is_none() && !simulation && !calls_test_finished(node) {
        return None;
    }
    Some(TestCycles {
        cycle_ns: cycle
            .and_then(|entry| cycle_ns(&entry.value))
            .unwrap_or(DEFAULT_TEST_CYCLE_NS),
        max_cycles: budget
            .and_then(|entry| max_cycles(&entry.value))
            .unwrap_or(DEFAULT_TEST_MAX_CYCLES),
        simulation,
    })
}

/// A test attribute's rule: its key, whether a value of it reads, and the words that say what the value must be.
type AttributeRule = (&'static str, fn(&str) -> bool, &'static str);

/// The refusals of the test attributes of the POU declaration `node`, each with the pragma's range: a `test_cycle` that is no
/// duration above zero and at most one hour, a `test_max_cycles` that is no whole number from 1 to 10,000,000, a
/// `test_simulation` with a value, and any of them on a declaration that is no test program.
#[must_use]
pub fn test_attribute_errors(node: &SyntaxNode) -> Vec<(TextRange, String)> {
    let program = test_kind(node) == Some(TestKind::Program);
    let rules: [AttributeRule; 3] = [
        (
            "test_cycle",
            |value| cycle_ns(value).is_some(),
            "a duration above zero and at most T#1h, such as T#10ms",
        ),
        (
            "test_max_cycles",
            |value| max_cycles(value).is_some(),
            "a whole number of cycles from 1 to 10000000",
        ),
        (
            "test_simulation",
            |value| value.trim().is_empty(),
            "given without a value",
        ),
    ];
    let mut errors = Vec::new();
    for (key, reads, rule) in rules {
        let Some(entry) = attribute_entry(node, key) else {
            continue;
        };
        if !program {
            errors.push((
                entry.range,
                format!("{{attribute '{key}'}} belongs to a test program"),
            ));
        } else if !reads(&entry.value) {
            errors.push((
                entry.range,
                format!("{{attribute '{key}'}} is '{}', not {rule}", entry.value),
            ));
        }
    }
    errors
}

/// The cycle a `test_cycle` value names, in nanoseconds: a duration literal above zero and at most one hour.
fn cycle_ns(value: &str) -> Option<u64> {
    let nanos = crate::duration_text::duration_nanos(value)?;
    u64::try_from(nanos)
        .ok()
        .filter(|nanos| (1..=MAX_TEST_CYCLE_NS).contains(nanos))
}

/// The budget a `test_max_cycles` value names: a whole number from 1 to 10,000,000.
fn max_cycles(value: &str) -> Option<u32> {
    value
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|cycles| (1..=MAX_TEST_MAX_CYCLES).contains(cycles))
}

/// Whether the body of `node` calls `TEST_FINISHED()`, compared without case.
fn calls_test_finished(node: &SyntaxNode) -> bool {
    node.descendants()
        .filter(|child| child.kind() == SyntaxKind::CallExpr)
        .any(|call| {
            call.first_child().is_some_and(|callee| {
                callee
                    .text()
                    .to_string()
                    .trim()
                    .eq_ignore_ascii_case("TEST_FINISHED")
            })
        })
}

#[cfg(test)]
#[path = "test_pous/contract_tests.rs"]
mod contract_tests;
