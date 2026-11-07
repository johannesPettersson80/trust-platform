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

/// Specification 10 §4.8: parentheses after a named type hold the instance's `FB_init` arguments,
/// an `ArgList` of the declaration, also before an initial value; parentheses that hold `lo..hi`
/// stay the type's subrange. Catches the arguments parsed as a broken subrange, and a subrange
/// taken for arguments.
#[test]
fn fb_init_arguments_follow_the_type_and_subranges_stay_subranges() {
    let parsed = parse(
        "PROGRAM Main\nVAR\n    com1 : serialdevice(iCOMnum := 1);\n    com2 : serialdevice(2, TRUE) := (Speed := 9600);\n    level : Percent(0..100);\nEND_VAR\nEND_PROGRAM",
    );
    assert!(parsed.ok(), "{:?}", parsed.errors());
    let declarations: Vec<_> = parsed
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .collect();
    let arguments: Vec<usize> = declarations
        .iter()
        .map(|declaration| {
            declaration
                .children()
                .filter(|child| child.kind() == SyntaxKind::ArgList)
                .map(|list| {
                    list.children()
                        .filter(|arg| arg.kind() == SyntaxKind::Arg)
                        .count()
                })
                .sum()
        })
        .collect();
    assert_eq!(arguments, vec![1, 2, 0]);
    assert!(declarations[2]
        .descendants()
        .any(|node| node.kind() == SyntaxKind::Subrange));
}
