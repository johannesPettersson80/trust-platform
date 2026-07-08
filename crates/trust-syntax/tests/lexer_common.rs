use trust_syntax::lexer::{lex_with_text, TokenKind};

fn non_trivia_kinds(source: &str) -> Vec<TokenKind> {
    lex_with_text(source)
        .into_iter()
        .filter(|(token, _)| !token.kind.is_trivia())
        .map(|(token, _)| token.kind)
        .collect()
}

fn non_trivia_pairs(source: &str) -> Vec<(TokenKind, String)> {
    lex_with_text(source)
        .into_iter()
        .filter(|(token, _)| !token.kind.is_trivia())
        .map(|(token, text)| (token.kind, text.to_string()))
        .collect()
}

#[test]
fn iec_6_1() {
    let source = "ProGrAm my_var1 // line\n(* outer (* inner *) outer *)\n/* alt */\nEND_PROGRAM";
    let tokens = lex_with_text(source);
    let kinds: Vec<_> = tokens.iter().map(|(token, _)| token.kind).collect();

    assert!(kinds.contains(&TokenKind::Whitespace));
    assert!(kinds.contains(&TokenKind::LineComment));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == TokenKind::BlockComment)
            .count(),
        2
    );

    let non_trivia = non_trivia_kinds(source);
    assert_eq!(
        non_trivia,
        vec![
            TokenKind::KwProgram,
            TokenKind::Ident,
            TokenKind::KwEndProgram
        ]
    );

    // DEV-013: identifiers are ASCII-only.
    let unicode_tokens = lex_with_text("å");
    assert!(unicode_tokens
        .iter()
        .any(|(token, _)| token.kind == TokenKind::Error));
}

#[test]
fn numeric_dot_and_range_tokens_do_not_need_int_literal_dot_rewrite() {
    let tokens = non_trivia_pairs("1..5 1.0 1.");

    assert_eq!(
        tokens,
        vec![
            (TokenKind::IntLiteral, "1".to_string()),
            (TokenKind::DotDot, "..".to_string()),
            (TokenKind::IntLiteral, "5".to_string()),
            (TokenKind::RealLiteral, "1.0".to_string()),
            (TokenKind::IntLiteral, "1".to_string()),
            (TokenKind::Dot, ".".to_string()),
        ]
    );
}
