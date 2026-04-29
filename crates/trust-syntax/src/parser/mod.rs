//! Parser for IEC 61131-3 Structured Text.
//!
//! This module provides a hand-written recursive descent parser that builds
//! a lossless concrete syntax tree (CST) using the `rowan` library.
//!
//! # Design
//!
//! The parser is designed for IDE use:
//!
//! - **Error-tolerant**: Continues parsing after errors
//! - **Lossless**: Preserves all source text including whitespace and comments
//! - **Incremental-ready**: Architecture supports future incremental reparsing
//!
//! # Architecture
//!
//! The parser uses a three-phase approach:
//!
//! 1. **Lexing**: Tokenize source text (see `lexer` module)
//! 2. **Parsing**: Build a flat stream of events (start node, add token, finish node)
//! 3. **Tree Building**: Convert events into a `rowan` green tree

#![allow(clippy::module_inception)]

pub mod event;
pub mod grammar;
mod parser;
mod sink;
mod source;

pub use parser::parse;
pub(crate) use parser::{BoundedTopLevelScan, CompletedMarker, Parser};

use crate::syntax::SyntaxNode;

/// Result of parsing source text.
#[derive(Debug)]
pub struct Parse {
    /// The root syntax node.
    green_node: rowan::GreenNode,
    /// Parsing errors.
    errors: Vec<ParseError>,
}

impl Parse {
    /// Returns the root syntax node.
    #[must_use]
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green_node.clone())
    }

    /// Returns the parsing errors.
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Returns `true` if parsing produced no errors.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// A parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The error message.
    pub message: String,
    /// The byte range where the error occurred.
    pub range: text_size::TextRange,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}..{}",
            self.message,
            u32::from(self.range.start()),
            u32::from(self.range.end())
        )
    }
}

impl std::error::Error for ParseError {}
