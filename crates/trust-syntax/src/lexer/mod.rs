//! Lexer for IEC 61131-3 Structured Text.
//!
//! This module provides a lexer that tokenizes ST source code into a stream
//! of tokens with their positions in the source text.

mod tokens;

pub use tokens::TokenKind;

use logos::Logos;
use text_size::{TextRange, TextSize};

/// A token produced by the lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// The kind of token.
    pub kind: TokenKind,
    /// The byte range of the token in the source text.
    pub range: TextRange,
}

impl Token {
    /// Creates a new token.
    #[must_use]
    pub fn new(kind: TokenKind, range: TextRange) -> Self {
        Self { kind, range }
    }

    /// Returns the length of the token in bytes.
    #[must_use]
    pub fn len(&self) -> TextSize {
        self.range.len()
    }

    /// Returns true if the token has zero length.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }
}

/// Lexer for Structured Text source code.
///
/// The lexer is an iterator over tokens. It handles all error recovery
/// internally - any unrecognized characters are returned as `TokenKind::Error`.
pub struct Lexer<'src> {
    inner: logos::Lexer<'src, TokenKind>,
    source: &'src str,
}

impl<'src> Lexer<'src> {
    /// Creates a new lexer for the given source text.
    #[must_use]
    pub fn new(source: &'src str) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            source,
        }
    }

    /// Returns the source text being lexed.
    #[must_use]
    pub fn source(&self) -> &'src str {
        self.source
    }

    /// Returns the text of the current token.
    #[must_use]
    pub fn slice(&self) -> &'src str {
        self.inner.slice()
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let kind = self.inner.next()?;
        let span = self.inner.span();

        let kind = kind.unwrap_or(TokenKind::Error);
        let range = TextRange::new(
            TextSize::from(span.start as u32),
            TextSize::from(span.end as u32),
        );

        Some(Token::new(kind, range))
    }
}

/// Lex the entire source and return all tokens.
///
/// This is a convenience function for testing and simple use cases.
/// For the parser, use the `Lexer` iterator directly.
#[must_use]
pub fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).collect()
}

/// Lex source and return tokens paired with their text.
///
/// Useful for debugging and testing.
#[must_use]
pub fn lex_with_text(source: &str) -> Vec<(Token, &str)> {
    Lexer::new(source)
        .map(|token| {
            let text = &source[usize::from(token.range.start())..usize::from(token.range.end())];
            (token, text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_basic() {
        let source = "x := 42;";
        let tokens = lex(source);

        // x, whitespace, :=, whitespace, 42, ;
        let non_trivia: Vec<_> = tokens.iter().filter(|t| !t.kind.is_trivia()).collect();
        assert_eq!(non_trivia.len(), 4);
        assert_eq!(non_trivia[0].kind, TokenKind::Ident);
        assert_eq!(non_trivia[1].kind, TokenKind::Assign);
        assert_eq!(non_trivia[2].kind, TokenKind::IntLiteral);
        assert_eq!(non_trivia[3].kind, TokenKind::Semicolon);
    }

    #[test]
    fn test_lexer_preserves_positions() {
        let source = "abc := 123";
        let tokens = lex(source);

        // "abc" is at position 0..3
        assert_eq!(tokens[0].range, TextRange::new(0.into(), 3.into()));
        // " " is at position 3..4
        assert_eq!(tokens[1].range, TextRange::new(3.into(), 4.into()));
        // ":=" is at position 4..6
        assert_eq!(tokens[2].range, TextRange::new(4.into(), 6.into()));
    }

    #[test]
    fn test_lex_with_text() {
        let source = "x := 42";
        let tokens = lex_with_text(source);

        let non_trivia: Vec<_> = tokens.iter().filter(|(t, _)| !t.kind.is_trivia()).collect();
        assert_eq!(non_trivia[0].1, "x");
        assert_eq!(non_trivia[1].1, ":=");
        assert_eq!(non_trivia[2].1, "42");
    }

    #[test]
    fn test_full_function_block() {
        let source = r#"
FUNCTION_BLOCK FB_Motor
VAR_INPUT
    enable : BOOL;
    speed : REAL;
END_VAR
VAR_OUTPUT
    running : BOOL;
END_VAR
VAR
    _internalState : INT;
END_VAR

IF enable THEN
    running := TRUE;
END_IF
END_FUNCTION_BLOCK
"#;

        let tokens = lex(source);
        let non_trivia: Vec<_> = tokens.iter().filter(|t| !t.kind.is_trivia()).collect();

        // Check key tokens are present
        assert!(non_trivia
            .iter()
            .any(|t| t.kind == TokenKind::KwFunctionBlock));
        assert!(non_trivia.iter().any(|t| t.kind == TokenKind::KwVarInput));
        assert!(non_trivia.iter().any(|t| t.kind == TokenKind::KwIf));
        assert!(non_trivia
            .iter()
            .any(|t| t.kind == TokenKind::KwEndFunctionBlock));
    }
}
