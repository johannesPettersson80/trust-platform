//! Test POU discovery (specification 22, "Test POU discovery"). The first two tests moved here from
//! `trust-dev` with the function they test; the first now expects the dotted namespace in the name,
//! as specification 22 says (it pinned `CaseOne`, the namespace dropped).

use super::*;

fn discover(sources: &[(&str, &str)]) -> Vec<DiscoveredTest> {
    discover_tests(sources.iter().map(|(path, text)| (Path::new(*path), *text)))
}

#[test]
fn discovery_finds_test_pous_with_namespace_qualification() {
    let discovered = discover(&[
        (
            "b.st",
            r#"
TEST_PROGRAM Plain
END_TEST_PROGRAM
"#,
        ),
        (
            "a.st",
            r#"
NAMESPACE NS.Core
TEST_FUNCTION_BLOCK CaseOne
END_TEST_FUNCTION_BLOCK
END_NAMESPACE
"#,
        ),
    ]);
    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].name, "NS.Core.CaseOne");
    assert_eq!(discovered[0].kind, TestKind::FunctionBlock);
    assert_eq!(
        discovered[0].source_line.as_deref(),
        Some("TEST_FUNCTION_BLOCK CaseOne")
    );
    assert_eq!(discovered[1].name, "Plain");
    assert_eq!(discovered[1].kind, TestKind::Program);
    assert_eq!(
        discovered[1].source_line.as_deref(),
        Some("TEST_PROGRAM Plain")
    );
}

#[test]
fn discovery_ignores_comments_after_test_name() {
    let discovered = discover(&[(
        "comments.st",
        r#"
TEST_PROGRAM InlineComment (* inline comment *)
END_TEST_PROGRAM

TEST_PROGRAM NextLineComment
(* line comment right after declaration *)
END_TEST_PROGRAM
"#,
    )]);
    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].name, "InlineComment");
    assert_eq!(discovered[1].name, "NextLineComment");
}

#[test]
fn a_test_declaration_reports_the_one_based_line_of_its_keyword() {
    // Line 1 is empty, line 2 a comment, line 3 holds `TEST_PROGRAM Third` (indented), lines 5-6 a
    // plain PROGRAM, line 7 the keyword `TEST_FUNCTION_BLOCK` and line 8 its name. Catches:
    // zero-based lines, the name's line instead of the keyword's, or a plain PROGRAM taken as a test.
    let discovered = discover(&[(
        "lines.st",
        "\n// tests\n  TEST_PROGRAM Third\nEND_TEST_PROGRAM\nPROGRAM Plain\nEND_PROGRAM\nTEST_FUNCTION_BLOCK\n  Seventh\nEND_TEST_FUNCTION_BLOCK\n",
    )]);
    let found: Vec<_> = discovered
        .iter()
        .map(|test| (test.kind.label(), test.name.as_str(), test.line))
        .collect();
    assert_eq!(
        found,
        [
            ("TEST_PROGRAM", "Third", 3),
            ("TEST_FUNCTION_BLOCK", "Seventh", 7)
        ]
    );
}

#[test]
fn nested_and_dotted_namespaces_qualify_a_test_from_the_outermost() {
    // `NAMESPACE Plant` around `NAMESPACE Units.Mixer` around the test: `Plant.Units.Mixer.Fills`.
    // Catches: a dropped dotted part, or the parts in inner-to-outer order.
    let discovered = discover(&[(
        "nested.st",
        "NAMESPACE Plant\nNAMESPACE Units.Mixer\nTEST_PROGRAM Fills\nEND_TEST_PROGRAM\nEND_NAMESPACE\nEND_NAMESPACE\n",
    )]);
    assert_eq!(
        discovered
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>(),
        ["Plant.Units.Mixer.Fills"]
    );
}
