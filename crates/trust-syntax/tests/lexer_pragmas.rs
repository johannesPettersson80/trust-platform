use trust_syntax::lexer::{lex_with_text, TokenKind};

fn non_trivia_kinds(source: &str) -> Vec<TokenKind> {
    lex_with_text(source)
        .into_iter()
        .filter(|(token, _)| !token.kind.is_trivia())
        .map(|(token, _)| token.kind)
        .collect()
}

#[test]
fn iec_6_2() {
    let source = "x {pragma stuff} := 1; '{not pragma}'";
    let tokens = lex_with_text(source);
    let pragma_count = tokens
        .iter()
        .filter(|(token, _)| token.kind == TokenKind::Pragma)
        .count();

    assert_eq!(pragma_count, 1);
    let string_texts: Vec<_> = tokens
        .iter()
        .filter(|(token, _)| token.kind == TokenKind::StringLiteral)
        .map(|(_, text)| *text)
        .collect();
    assert_eq!(string_texts, vec!["'{not pragma}'"]);

    let non_trivia = non_trivia_kinds(source);
    assert_eq!(
        non_trivia,
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::IntLiteral,
            TokenKind::Semicolon,
            TokenKind::StringLiteral
        ]
    );
}
