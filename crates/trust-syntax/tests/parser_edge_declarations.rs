mod common;
use common::*;

fn edge_tokens(source: &str) -> Vec<SyntaxKind> {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "edge declaration must parse: {:?}",
        parsed.errors()
    );
    parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .flat_map(|node| {
            node.descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .map(|token| token.kind())
                .filter(|kind| matches!(kind, SyntaxKind::KwREdge | SyntaxKind::KwFEdge))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn edge_declaration_parser_preserves_function_block_rising_and_falling_suffixes() {
    let tokens = edge_tokens(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
END_FUNCTION_BLOCK
"#,
    );

    assert_eq!(tokens, [SyntaxKind::KwREdge, SyntaxKind::KwFEdge]);
}

#[test]
fn edge_declaration_parser_preserves_program_rising_and_falling_suffixes() {
    let tokens = edge_tokens(
        r#"
PROGRAM EdgeProgram
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(tokens, [SyntaxKind::KwREdge, SyntaxKind::KwFEdge]);
}

#[test]
fn edge_declaration_parser_keeps_one_suffix_on_a_multi_name_declaration() {
    let source = r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    First, Second, Third : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "multi-name edge declaration must parse: {:?}",
        parsed.errors()
    );
    let edge_declaration = parsed
        .syntax()
        .descendants()
        .find(|node| {
            node.kind() == SyntaxKind::VarDecl
                && node
                    .descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .any(|token| token.kind() == SyntaxKind::KwREdge)
        })
        .expect("edge declaration");
    let names = edge_declaration
        .children()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .count();
    assert_eq!(names, 3);
}
