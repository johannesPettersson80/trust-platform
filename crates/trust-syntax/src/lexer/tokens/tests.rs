use super::*;

fn lex(input: &str) -> Vec<(TokenKind, &str)> {
    crate::lexer::lex_with_text(input)
        .into_iter()
        .map(|(token, text)| (token.kind, text))
        .collect()
}

#[path = "tests_part_01.rs"]
mod tests_part_01;
#[path = "tests_part_02.rs"]
mod tests_part_02;
