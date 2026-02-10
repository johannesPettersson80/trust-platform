//! Expression parsing using Pratt parsing.
//!
//! Operator precedence (low to high):
//! - OR (1-2)
//! - XOR (3-4)
//! - AND/& (5-6)
//! - =, <>, <, <=, >, >= (7-8)
//! - +, - (9-10)
//! - *, /, MOD (11-12)
//! - ** (14-13, right associative)
//! - NOT, unary +/- (15)

use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use super::super::CompletedMarker;
use super::super::Parser;

// Guard against pathological recursive inputs discovered by fuzzing.
const MAX_EXPRESSION_DEPTH: usize = 1024;

impl Parser<'_, '_> {
    /// Parse an expression using Pratt parsing.
    pub(crate) fn parse_expression(&mut self) -> CompletedMarker {
        self.parse_expr_bp(0)
    }

    /// Parse expression with minimum binding power.
    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8) -> CompletedMarker {
        if self.expr_depth >= MAX_EXPRESSION_DEPTH {
            let marker = self.start();
            self.error("expression nesting exceeds parser limit");
            if !self.at_end() {
                self.bump();
            }
            return marker.complete(self, SyntaxKind::Error);
        }

        self.expr_depth += 1;

        let mut lhs = if let Some(bp) = self.current().prefix_binding_power() {
            let marker = self.start();
            self.bump();
            self.parse_expr_bp(bp);
            marker.complete(self, SyntaxKind::UnaryExpr)
        } else {
            self.parse_primary_expr()
        };

        loop {
            if let Some(next) = self.parse_postfix_expr(lhs) {
                lhs = next;
                continue;
            }

            let op = self.current();
            if let Some((l_bp, r_bp)) = op.infix_binding_power() {
                if l_bp < min_bp {
                    break;
                }

                let marker = lhs.precede(self);
                self.bump(); // operator
                self.parse_expr_bp(r_bp);
                lhs = marker.complete(self, SyntaxKind::BinaryExpr);
                continue;
            }

            break;
        }

        self.expr_depth -= 1;
        lhs
    }

    /// Parse postfix expressions (field access, calls, indexing, dereference).
    pub(crate) fn parse_postfix_expr(&mut self, lhs: CompletedMarker) -> Option<CompletedMarker> {
        match self.current() {
            TokenKind::Dot => {
                let marker = lhs.precede(self);
                self.bump();
                if self.at(TokenKind::Ident) {
                    self.parse_name();
                } else if self.at(TokenKind::IntLiteral) || self.at(TokenKind::DirectAddress) {
                    self.start_node(SyntaxKind::Literal);
                    self.bump();
                    self.finish_node();
                } else {
                    self.error("expected field name");
                }
                Some(marker.complete(self, SyntaxKind::FieldExpr))
            }
            TokenKind::LParen => {
                let marker = lhs.precede(self);
                self.parse_arg_list();
                Some(marker.complete(self, SyntaxKind::CallExpr))
            }
            TokenKind::LBracket => {
                let marker = lhs.precede(self);
                self.bump();
                self.parse_expression();
                while self.at(TokenKind::Comma) {
                    self.bump();
                    self.parse_expression();
                }
                if self.at(TokenKind::RBracket) {
                    self.bump();
                } else {
                    self.error("expected ]");
                }
                Some(marker.complete(self, SyntaxKind::IndexExpr))
            }
            TokenKind::Caret => {
                let marker = lhs.precede(self);
                self.bump();
                Some(marker.complete(self, SyntaxKind::DerefExpr))
            }
            _ => None,
        }
    }

    /// Parse primary expressions (literals, identifiers, etc.).
    pub(crate) fn parse_primary_expr(&mut self) -> CompletedMarker {
        match self.current() {
            TokenKind::IntLiteral
            | TokenKind::RealLiteral
            | TokenKind::StringLiteral
            | TokenKind::WideStringLiteral
            | TokenKind::TimeLiteral
            | TokenKind::DateLiteral
            | TokenKind::TimeOfDayLiteral
            | TokenKind::DateAndTimeLiteral => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::Literal)
            }
            TokenKind::KwTrue | TokenKind::KwFalse | TokenKind::KwNull => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::Literal)
            }
            TokenKind::TypedLiteralPrefix => {
                let marker = self.start();
                self.bump();
                let has_sign = matches!(self.current(), TokenKind::Plus | TokenKind::Minus);
                if has_sign {
                    self.bump();
                    if matches!(
                        self.current(),
                        TokenKind::IntLiteral | TokenKind::RealLiteral
                    ) {
                        if self.current() == TokenKind::IntLiteral
                            && self.source.current_text().contains('#')
                        {
                            self.error("based numeric literals cannot use leading sign");
                        }
                        self.bump();
                    } else {
                        self.error("expected numeric literal after sign");
                        if matches!(
                            self.current(),
                            TokenKind::StringLiteral
                                | TokenKind::WideStringLiteral
                                | TokenKind::TimeLiteral
                                | TokenKind::DateLiteral
                                | TokenKind::TimeOfDayLiteral
                                | TokenKind::DateAndTimeLiteral
                                | TokenKind::KwTrue
                                | TokenKind::KwFalse
                                | TokenKind::Ident
                                | TokenKind::KwEn
                                | TokenKind::KwEno
                        ) {
                            self.bump();
                        }
                    }
                } else if matches!(
                    self.current(),
                    TokenKind::IntLiteral
                        | TokenKind::RealLiteral
                        | TokenKind::StringLiteral
                        | TokenKind::WideStringLiteral
                        | TokenKind::TimeLiteral
                        | TokenKind::DateLiteral
                        | TokenKind::TimeOfDayLiteral
                        | TokenKind::DateAndTimeLiteral
                        | TokenKind::KwTrue
                        | TokenKind::KwFalse
                        | TokenKind::Ident
                        | TokenKind::KwEn
                        | TokenKind::KwEno
                ) {
                    self.bump();
                } else {
                    self.error("expected typed literal value");
                }
                marker.complete(self, SyntaxKind::Literal)
            }
            TokenKind::Ident
            | TokenKind::KwEn
            | TokenKind::KwEno
            | TokenKind::KwRef
            | TokenKind::KwNew
            | TokenKind::KwNewDunder
            | TokenKind::KwDeleteDunder => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::NameRef)
            }
            TokenKind::DirectAddress => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::NameRef)
            }
            TokenKind::LParen => {
                let marker = self.start();
                self.bump();
                self.parse_expression();
                if self.at(TokenKind::RParen) {
                    self.bump();
                } else {
                    self.error("expected )");
                }
                marker.complete(self, SyntaxKind::ParenExpr)
            }
            TokenKind::KwThis => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::ThisExpr)
            }
            TokenKind::KwSuper => {
                let marker = self.start();
                self.bump();
                marker.complete(self, SyntaxKind::SuperExpr)
            }
            TokenKind::KwAdr => {
                let marker = self.start();
                self.bump();
                if self.at(TokenKind::LParen) {
                    self.bump();
                    self.parse_expression();
                    if self.at(TokenKind::RParen) {
                        self.bump();
                    } else {
                        self.error("expected )");
                    }
                } else {
                    self.error("expected (");
                }
                marker.complete(self, SyntaxKind::AddrExpr)
            }
            TokenKind::KwSizeOf => {
                let marker = self.start();
                self.bump();
                if self.at(TokenKind::LParen) {
                    self.bump();
                    if self.current().is_type_keyword()
                        || matches!(
                            self.current(),
                            TokenKind::Ident
                                | TokenKind::KwArray
                                | TokenKind::KwPointer
                                | TokenKind::KwRefTo
                        )
                    {
                        self.parse_type_ref();
                    } else if self.current().can_start_expr() {
                        self.parse_expression();
                    } else {
                        self.error("expected type or expression");
                    }
                    if self.at(TokenKind::RParen) {
                        self.bump();
                    } else {
                        self.error("expected )");
                    }
                } else {
                    self.error("expected (");
                }
                marker.complete(self, SyntaxKind::SizeOfExpr)
            }
            _ => {
                let marker = self.start();
                self.error("expected expression");
                if !self.at_end() {
                    self.bump();
                }
                marker.complete(self, SyntaxKind::Error)
            }
        }
    }

    /// Parse argument list for function calls.
    pub(crate) fn parse_arg_list(&mut self) {
        self.start_node(SyntaxKind::ArgList);
        self.bump(); // (

        while !self.at(TokenKind::RParen) && !self.at_end() {
            self.start_node(SyntaxKind::Arg);

            // Check for named argument
            if (self.at(TokenKind::Ident) || self.at(TokenKind::KwEn) || self.at(TokenKind::KwEno))
                && matches!(
                    self.peek_kind_n(1),
                    TokenKind::Assign | TokenKind::Arrow | TokenKind::RefAssign
                )
            {
                self.start_node(SyntaxKind::Name);
                self.bump();
                self.finish_node();
                self.bump(); // :=, =>, ?=
            }

            self.parse_expression();
            self.finish_node();

            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }

        if self.at(TokenKind::RParen) {
            self.bump();
        }

        self.finish_node();
    }
}
