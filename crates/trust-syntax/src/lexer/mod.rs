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
        let mut kind = kind.unwrap_or(TokenKind::Error);
        let mut end = span.end;

        if let Some(candidate_len) =
            malformed_candidate_len(kind, &self.source[span.start..], span.end - span.start)
        {
            let consumed = span.end - span.start;
            if candidate_len > consumed {
                self.inner.bump(candidate_len - consumed);
            }
            kind = TokenKind::Error;
            end = span.start + candidate_len;
        }
        let range = TextRange::new(
            TextSize::from(span.start as u32),
            TextSize::from(end as u32),
        );

        Some(Token::new(kind, range))
    }
}

fn malformed_candidate_len(kind: TokenKind, source: &str, consumed: usize) -> Option<usize> {
    if matches!(kind, TokenKind::Ident | TokenKind::Error) {
        let candidate_len = identifier_candidate_len(source);
        if candidate_len > 0 && !valid_ascii_identifier(&source[..candidate_len]) {
            return Some(candidate_len);
        }
    }

    if source.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        if let Some(candidate_len) = based_integer_candidate_len(source) {
            if !valid_based_integer(&source[..candidate_len]) {
                return Some(candidate_len);
            }
        } else {
            let candidate_len = decimal_candidate_len(source);
            if candidate_len > consumed
                || matches!(kind, TokenKind::IntLiteral | TokenKind::RealLiteral)
            {
                let candidate = &source[..candidate_len];
                if !valid_decimal_candidate(candidate) {
                    return Some(candidate_len);
                }
            }
        }
    }

    if matches!(
        kind,
        TokenKind::TypedLiteralPrefix
            | TokenKind::TimeLiteral
            | TokenKind::DateLiteral
            | TokenKind::TimeOfDayLiteral
            | TokenKind::DateAndTimeLiteral
    ) {
        let prefix_len = temporal_prefix_len(source);
        if prefix_len > 0 {
            let candidate_len = temporal_candidate_len(source);
            if candidate_len > prefix_len
                && kind == TokenKind::TypedLiteralPrefix
                && candidate_len > consumed
            {
                return Some(candidate_len);
            }
            if kind == TokenKind::TimeLiteral
                && duration_fraction_precedes_another_unit(&source[..candidate_len])
            {
                return Some(candidate_len);
            }
        }
    }

    None
}

fn identifier_candidate_len(source: &str) -> usize {
    let mut end = 0usize;
    for (offset, ch) in source.char_indices() {
        if ch == '_' || ch.is_alphanumeric() {
            end = offset + ch.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn valid_ascii_identifier(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && *first != b'_' {
        return false;
    }
    if bytes
        .iter()
        .any(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
    {
        return false;
    }
    if *first == b'_' {
        if bytes.len() == 1 || bytes[1] == b'_' {
            return false;
        }
        if !bytes[1..].iter().any(u8::is_ascii_alphabetic) {
            return false;
        }
    }
    !candidate.ends_with('_') && !candidate.contains("__")
}

fn based_integer_candidate_len(source: &str) -> Option<usize> {
    let prefix_len = ["16#", "2#", "8#"]
        .into_iter()
        .find(|prefix| source.starts_with(prefix))
        .map(str::len)?;
    let bytes = source.as_bytes();
    let mut end = prefix_len;
    if bytes
        .get(end)
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        end += 1;
    }
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    Some(end)
}

fn valid_based_integer(candidate: &str) -> bool {
    let Some((base_text, digits)) = candidate.split_once('#') else {
        return false;
    };
    let base = match base_text {
        "2" => 2,
        "8" => 8,
        "16" => 16,
        _ => return false,
    };
    if !valid_separated_digits(digits) {
        return false;
    }
    digits
        .bytes()
        .filter(|byte| *byte != b'_')
        .all(|byte| match base {
            2 => matches!(byte, b'0' | b'1'),
            8 => matches!(byte, b'0'..=b'7'),
            16 => byte.is_ascii_hexdigit(),
            _ => false,
        })
}

fn decimal_candidate_len(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut end = 0usize;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
    {
        end += 1;
    }
    if bytes.get(end) == Some(&b'.') && bytes.get(end + 1) != Some(&b'.') {
        end += 1;
        while bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            end += 1;
        }
        if bytes
            .get(end)
            .is_some_and(|byte| matches!(byte, b'e' | b'E'))
        {
            end += 1;
            if bytes
                .get(end)
                .is_some_and(|byte| matches!(byte, b'+' | b'-'))
            {
                end += 1;
            }
            while bytes
                .get(end)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
            {
                end += 1;
            }
        }
    }
    end
}

fn valid_decimal_candidate(candidate: &str) -> bool {
    if let Some((whole, fraction_and_exponent)) = candidate.split_once('.') {
        if !valid_separated_digits(whole) {
            return false;
        }
        let (fraction, exponent) = fraction_and_exponent
            .split_once(['e', 'E'])
            .map_or((fraction_and_exponent, None), |(fraction, exponent)| {
                (fraction, Some(exponent))
            });
        if !valid_separated_digits(fraction) {
            return false;
        }
        if let Some(exponent) = exponent {
            let digits = exponent
                .strip_prefix('+')
                .or_else(|| exponent.strip_prefix('-'))
                .unwrap_or(exponent);
            valid_separated_digits(digits)
        } else {
            true
        }
    } else {
        valid_separated_digits(candidate)
    }
}

fn valid_separated_digits(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('_')
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn temporal_prefix_len(source: &str) -> usize {
    const PREFIXES: &[&str] = &[
        "LDATE_AND_TIME#",
        "DATE_AND_TIME#",
        "LTIME_OF_DAY#",
        "TIME_OF_DAY#",
        "LTIME#",
        "LDATE#",
        "LTOD#",
        "TIME#",
        "DATE#",
        "LDT#",
        "TOD#",
        "LT#",
        "LD#",
        "DT#",
        "T#",
        "D#",
    ];
    PREFIXES
        .iter()
        .find(|prefix| {
            source
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        })
        .map_or(0, |prefix| prefix.len())
}

fn temporal_candidate_len(source: &str) -> usize {
    let prefix_len = temporal_prefix_len(source);
    let mut end = prefix_len;
    for byte in source.as_bytes().iter().skip(prefix_len) {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-' | b'+') {
            end += 1;
        } else {
            break;
        }
    }
    end
}

fn duration_fraction_precedes_another_unit(candidate: &str) -> bool {
    candidate
        .find('.')
        .is_some_and(|fraction| candidate[fraction..].contains('_'))
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
