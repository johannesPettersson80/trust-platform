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
