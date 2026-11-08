//! Specification 22, "Test POU discovery": which declarations are tests.

use super::*;
use trust_syntax::parser::parse;

/// The test kind of each program and function block of `source`, by name, in source order.
fn kinds(source: &str) -> Vec<(String, Option<TestKind>)> {
    parse(source)
        .syntax()
        .descendants()
        .filter(|node| matches!(node.kind(), SyntaxKind::Program | SyntaxKind::FunctionBlock))
        .map(|node| {
            let name = node
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .map(|name| name.text().to_string().trim().to_owned())
                .unwrap_or_default();
            (name, test_kind(&node))
        })
        .collect()
}

/// Catches the attribute ignored (a CODESYS project's test is never found), the attribute leaking
/// onto the next declaration (a production program becomes a test and is left unbound), and the
/// keyword forms lost on the way.
#[test]
fn a_program_with_the_test_attribute_is_a_test_program() {
    assert_eq!(
        kinds(
            r#"
{attribute 'test'}
PROGRAM PRG_TestTank
END_PROGRAM
PROGRAM PRG_Tank
END_PROGRAM
TEST_PROGRAM Native
END_TEST_PROGRAM
TEST_FUNCTION_BLOCK NativeBlock
END_TEST_FUNCTION_BLOCK
"#
        ),
        vec![
            ("PRG_TestTank".to_owned(), Some(TestKind::Program)),
            ("PRG_Tank".to_owned(), None),
            ("Native".to_owned(), Some(TestKind::Program)),
            ("NativeBlock".to_owned(), Some(TestKind::FunctionBlock)),
        ]
    );
}

/// Catches a pragma missed when a comment separates it from the keyword or when the program sits
/// in a namespace, where the parser parks the pragma in the namespace head's trivia.
#[test]
fn the_attribute_counts_across_comments_and_inside_a_namespace() {
    assert_eq!(
        kinds(
            r#"
NAMESPACE Plant
{attribute 'test'}
(* the level loop reaches its setpoint *)
PROGRAM PRG_TestLevel
END_PROGRAM
END_NAMESPACE
"#
        ),
        vec![("PRG_TestLevel".to_owned(), Some(TestKind::Program))]
    );
}

/// Catches a function block made a test by the attribute (the runner would invoke it as a
/// program), a pragma inside a body marking its program, and a key matched by its prefix.
#[test]
fn the_attribute_declares_a_test_only_before_a_program() {
    assert_eq!(
        kinds(
            r#"
{attribute 'test'}
FUNCTION_BLOCK FB_Check
END_FUNCTION_BLOCK
PROGRAM PRG_Body
VAR
    {attribute 'test'}
    level : INT;
END_VAR
END_PROGRAM
{attribute 'testing'}
PROGRAM PRG_OtherAttribute
END_PROGRAM
"#
        ),
        vec![
            ("FB_Check".to_owned(), None),
            ("PRG_Body".to_owned(), None),
            ("PRG_OtherAttribute".to_owned(), None),
        ]
    );
}

/// The first POU of `source`.
fn first_pou(source: &str) -> SyntaxNode {
    parse(source)
        .syntax()
        .descendants()
        .find(|node| matches!(node.kind(), SyntaxKind::Program | SyntaxKind::FunctionBlock))
        .expect("a POU")
}

/// Specification 22, "Multi-cycle tests": a test program is multi-cycle when its body calls `TEST_FINISHED()` or it names a cycle
/// or a budget; the defaults are 10 ms and 10,000 cycles. Catches a test that calls `TEST_FINISHED()` run once, an attribute's value
/// ignored, a test that names nothing made multi-cycle, and a test function block taken for one.
#[test]
fn a_test_program_is_multi_cycle_when_it_calls_test_finished_or_names_its_cycles() {
    let cycles = |source: &str| test_cycles(&first_pou(source));
    assert_eq!(
        cycles("TEST_PROGRAM T\nIF TRUE THEN test_finished(); END_IF\nEND_TEST_PROGRAM"),
        Some(TestCycles {
            cycle_ns: 10_000_000,
            max_cycles: 10_000
        })
    );
    assert_eq!(
        cycles("{attribute 'test_cycle' := 'T#100ms'}\nTEST_PROGRAM T\nEND_TEST_PROGRAM"),
        Some(TestCycles {
            cycle_ns: 100_000_000,
            max_cycles: 10_000
        })
    );
    assert_eq!(
        cycles("{attribute 'test'}\n{attribute 'test_max_cycles' := '50'}\nPROGRAM P\nEND_PROGRAM"),
        Some(TestCycles {
            cycle_ns: 10_000_000,
            max_cycles: 50
        })
    );
    assert_eq!(
        cycles("TEST_PROGRAM T\nASSERT_TRUE(TRUE);\nEND_TEST_PROGRAM"),
        None
    );
    assert_eq!(
        cycles("TEST_FUNCTION_BLOCK B\nTEST_FINISHED();\nEND_TEST_FUNCTION_BLOCK"),
        None
    );
}

/// Specification 22: a cycle must be a duration above zero and at most one hour, a budget a whole number from 1 to 10,000,000, and
/// only a test program takes them; each refusal is the compile's (E309). Catches a zero cycle (the time would never advance), a zero
/// budget, a cycle longer than an hour, and the attributes accepted on a production program.
#[test]
fn a_test_attribute_that_does_not_read_or_belongs_to_no_test_program_is_refused() {
    let errors = |source: &str| -> Vec<String> {
        test_attribute_errors(&first_pou(source))
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    };
    assert_eq!(
        errors("{attribute 'test_cycle' := 'T#0ms'}\nTEST_PROGRAM T\nEND_TEST_PROGRAM"),
        ["{attribute 'test_cycle'} is 'T#0ms', not a duration above zero and at most T#1h, such as T#10ms"]
    );
    assert_eq!(
        errors("{attribute 'test_cycle' := 'T#2h'}\nTEST_PROGRAM T\nEND_TEST_PROGRAM").len(),
        1
    );
    assert_eq!(
        errors("{attribute 'test_max_cycles' := '0'}\nTEST_PROGRAM T\nEND_TEST_PROGRAM"),
        ["{attribute 'test_max_cycles'} is '0', not a whole number of cycles from 1 to 10000000"]
    );
    assert_eq!(
        errors("{attribute 'test_max_cycles' := '50'}\nPROGRAM P\nEND_PROGRAM"),
        ["{attribute 'test_max_cycles'} belongs to a test program"]
    );
    assert_eq!(
        errors("{attribute 'test_cycle' := 'T#1h'}\n{attribute 'test_max_cycles' := '10000000'}\nTEST_PROGRAM T\nEND_TEST_PROGRAM"),
        Vec::<String>::new(),
        "the largest values read"
    );
}
