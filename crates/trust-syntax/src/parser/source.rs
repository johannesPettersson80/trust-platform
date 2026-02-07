//! Token source for the parser.
//!
//! This module provides the `Source` struct that wraps a token stream
//! and provides lookahead and consumption operations.

use crate::lexer::{Token, TokenKind};

/// A token source that provides tokens to the parser.
pub struct Source<'t, 'src> {
    tokens: &'t [Token],
    // Reserved for error reporting and debug helpers.
    #[allow(dead_code)]
    source: &'src str,
    cursor: usize,
}

impl<'t, 'src> Source<'t, 'src> {
    /// Creates a new source from tokens and source text.
    pub fn new(tokens: &'t [Token], source: &'src str) -> Self {
        Self {
            tokens,
            source,
            cursor: 0,
        }
    }

    /// Returns the current token kind, or `Eof` if at end.
    pub fn current(&self) -> TokenKind {
        self.peek_kind_n(0)
    }

    /// Returns the current token, or `None` if at end.
    pub fn current_token(&self) -> Option<&Token> {
        self.peek_token_n(0)
    }

    /// Peeks at the nth token ahead (0 = current), skipping trivia.
    pub fn peek_kind(&self) -> TokenKind {
        self.peek_kind_n(0)
    }

    /// Peeks at the nth non-trivia token ahead.
    pub fn peek_kind_n(&self, n: usize) -> TokenKind {
        let mut cursor = self.cursor;
        let mut non_trivia_seen = 0;

        while let Some(token) = self.tokens.get(cursor) {
            if !token.kind.is_trivia() {
                if non_trivia_seen == n {
                    return token.kind;
                }
                non_trivia_seen += 1;
            }
            cursor += 1;
        }

        TokenKind::Eof
    }

    /// Peeks at the nth non-trivia token ahead and returns the token.
    pub fn peek_token_n(&self, n: usize) -> Option<&Token> {
        let mut cursor = self.cursor;
        let mut non_trivia_seen = 0;

        while let Some(token) = self.tokens.get(cursor) {
            if !token.kind.is_trivia() {
                if non_trivia_seen == n {
                    return Some(token);
                }
                non_trivia_seen += 1;
            }
            cursor += 1;
        }

        None
    }

    /// Advances past the current token.
    pub fn bump(&mut self) {
        while let Some(token) = self.tokens.get(self.cursor) {
            if !token.kind.is_trivia() {
                self.cursor += 1;
                break;
            }
            self.cursor += 1;
        }
    }

    /// Returns `true` if at end of input.
    pub fn at_end(&self) -> bool {
        self.peek_kind_n(0) == TokenKind::Eof
    }

    /// Returns the text of the current token.
    // Currently unused; kept for diagnostics and tests.
    #[allow(dead_code)]
    pub fn current_text(&self) -> &'src str {
        self.current_token()
            .map(|t| &self.source[usize::from(t.range.start())..usize::from(t.range.end())])
            .unwrap_or("")
    }

    /// Returns the source text.
    // Currently unused; kept for diagnostics and tests.
    #[allow(dead_code)]
    pub fn source(&self) -> &'src str {
        self.source
    }

    /// Returns true if there is a top-level assignment operator before statement end.
    pub fn has_assign_ahead(&self) -> bool {
        let mut cursor = self.cursor;
        let mut paren_depth = 0u32;
        let mut bracket_depth = 0u32;

        while let Some(token) = self.tokens.get(cursor) {
            cursor += 1;

            if token.kind.is_trivia() {
                continue;
            }

            match token.kind {
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::Assign | TokenKind::RefAssign
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    return true;
                }
                TokenKind::Semicolon
                | TokenKind::KwThen
                | TokenKind::KwDo
                | TokenKind::KwOf
                | TokenKind::KwElse
                | TokenKind::KwElsif
                | TokenKind::KwEndIf
                | TokenKind::KwEndCase
                | TokenKind::KwEndFor
                | TokenKind::KwEndWhile
                | TokenKind::KwEndRepeat
                | TokenKind::KwEndFunctionBlock
                | TokenKind::KwEndTestFunctionBlock
                | TokenKind::KwEndFunction
                | TokenKind::KwEndProgram
                | TokenKind::KwEndTestProgram
                | TokenKind::KwEndMethod
                | TokenKind::KwEndProperty
                | TokenKind::KwEndInterface
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    return false;
                }
                _ => {}
            }
        }

        false
    }

    /// Returns true if there is a top-level colon before statement end.
    pub fn has_case_label_ahead(&self) -> bool {
        let mut cursor = self.cursor;
        let mut paren_depth = 0u32;
        let mut bracket_depth = 0u32;

        while let Some(token) = self.tokens.get(cursor) {
            cursor += 1;

            if token.kind.is_trivia() {
                continue;
            }

            match token.kind {
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
                TokenKind::Colon if paren_depth == 0 && bracket_depth == 0 => return true,
                TokenKind::Semicolon
                | TokenKind::KwThen
                | TokenKind::KwDo
                | TokenKind::KwOf
                | TokenKind::KwElse
                | TokenKind::KwElsif
                | TokenKind::KwEndIf
                | TokenKind::KwEndCase
                | TokenKind::KwEndFor
                | TokenKind::KwEndWhile
                | TokenKind::KwEndRepeat
                | TokenKind::KwEndFunctionBlock
                | TokenKind::KwEndTestFunctionBlock
                | TokenKind::KwEndFunction
                | TokenKind::KwEndProgram
                | TokenKind::KwEndTestProgram
                | TokenKind::KwEndMethod
                | TokenKind::KwEndProperty
                | TokenKind::KwEndInterface
                    if paren_depth == 0 && bracket_depth == 0 =>
                {
                    return false;
                }
                _ => {}
            }
        }

        false
    }
}
