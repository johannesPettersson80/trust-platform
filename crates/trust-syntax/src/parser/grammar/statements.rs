//! Statement parsing for IEC 61131-3 Structured Text.
//!
//! Supported statements:
//! - Assignment: `x := expr;`
//! - Expression statement: `func();`
//! - IF/ELSIF/ELSE/END_IF
//! - CASE/OF/END_CASE
//! - FOR/TO/BY/DO/END_FOR
//! - WHILE/DO/END_WHILE
//! - REPEAT/UNTIL/END_REPEAT
//! - RETURN, EXIT, CONTINUE
//! - Empty statement: `;`

use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use super::super::Parser;

impl Parser<'_, '_> {
    /// Parse a single statement.
    pub(crate) fn parse_statement(&mut self) {
        if self.at(TokenKind::KwIf) {
            self.parse_if_stmt();
        } else if self.at(TokenKind::KwCase) {
            self.parse_case_stmt();
        } else if self.at(TokenKind::KwFor) {
            self.parse_for_stmt();
        } else if self.at(TokenKind::KwWhile) {
            self.parse_while_stmt();
        } else if self.at(TokenKind::KwRepeat) {
            self.parse_repeat_stmt();
        } else if self.at(TokenKind::KwReturn) {
            self.parse_return_stmt();
        } else if self.at(TokenKind::KwExit) {
            self.start_node(SyntaxKind::ExitStmt);
            self.bump();
            self.expect_semicolon();
            self.finish_node();
        } else if self.at(TokenKind::KwContinue) {
            self.start_node(SyntaxKind::ContinueStmt);
            self.bump();
            self.expect_semicolon();
            self.finish_node();
        } else if self.at(TokenKind::KwJmp) {
            self.start_node(SyntaxKind::JmpStmt);
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_name();
            } else {
                self.error("expected label after JMP");
            }
            self.expect_semicolon();
            self.finish_node();
        } else if self.at(TokenKind::Ident) && self.peek_kind_n(1) == TokenKind::Colon {
            self.parse_label_stmt();
        } else if self.at(TokenKind::Semicolon) {
            self.start_node(SyntaxKind::EmptyStmt);
            self.bump();
            self.finish_node();
        } else if self.current().can_start_expr() {
            self.parse_assign_or_call_stmt();
        } else {
            self.error("expected statement");
            self.recover_statement();
        }
    }

    /// Parse IF statement.
    pub(crate) fn parse_if_stmt(&mut self) {
        self.start_node(SyntaxKind::IfStmt);
        self.bump(); // IF

        self.parse_expression(); // condition

        if self.at(TokenKind::KwThen) {
            self.bump();
        } else {
            self.error("expected THEN");
        }

        // Parse then statements
        while !self.at(TokenKind::KwElsif)
            && !self.at(TokenKind::KwElse)
            && !self.at(TokenKind::KwEndIf)
            && !self.at_end()
            && !self.at_stmt_list_end()
        {
            self.parse_statement();
        }

        // Parse elsif branches
        while self.at(TokenKind::KwElsif) {
            self.start_node(SyntaxKind::ElsifBranch);
            self.bump();
            self.parse_expression();
            if self.at(TokenKind::KwThen) {
                self.bump();
            } else {
                self.error("expected THEN");
            }
            while !self.at(TokenKind::KwElsif)
                && !self.at(TokenKind::KwElse)
                && !self.at(TokenKind::KwEndIf)
                && !self.at_end()
                && !self.at_stmt_list_end()
            {
                self.parse_statement();
            }
            self.finish_node();
        }

        // Parse else branch
        if self.at(TokenKind::KwElse) {
            self.start_node(SyntaxKind::ElseBranch);
            self.bump();
            while !self.at(TokenKind::KwEndIf) && !self.at_end() && !self.at_stmt_list_end() {
                self.parse_statement();
            }
            self.finish_node();
        }

        if self.at(TokenKind::KwEndIf) {
            self.bump();
        } else {
            self.error("expected END_IF");
        }

        self.finish_node();
    }

    fn parse_label_stmt(&mut self) {
        self.start_node(SyntaxKind::LabelStmt);
        self.parse_name();
        if self.at(TokenKind::Colon) {
            self.bump();
        } else {
            self.error("expected ':' after label");
        }
        if self.current().can_start_statement() {
            self.parse_statement();
        } else if self.at(TokenKind::Semicolon) {
            self.start_node(SyntaxKind::EmptyStmt);
            self.bump();
            self.finish_node();
        } else {
            self.error("expected statement after label");
        }
        self.finish_node();
    }

    /// Parse CASE statement.
    pub(crate) fn parse_case_stmt(&mut self) {
        self.start_node(SyntaxKind::CaseStmt);
        self.bump(); // CASE

        self.parse_expression();

        if self.at(TokenKind::KwOf) {
            self.bump();
        } else {
            self.error("expected OF");
        }

        // Parse case branches
        while !self.at(TokenKind::KwElse)
            && !self.at(TokenKind::KwEndCase)
            && !self.at_end()
            && !self.at_stmt_list_end()
        {
            self.start_node(SyntaxKind::CaseBranch);

            // Parse labels
            self.parse_case_label();
            while self.at(TokenKind::Comma) {
                self.bump();
                self.parse_case_label();
            }

            if self.at(TokenKind::Colon) {
                self.bump();
            } else {
                self.error("expected ':' after CASE label");
            }

            // Parse statements
            while !self.at(TokenKind::KwElse)
                && !self.at(TokenKind::KwEndCase)
                && !self.at_end()
                && !self.at_stmt_list_end()
            {
                if self.current().can_start_expr() && self.source.has_case_label_ahead() {
                    break;
                }
                if !self.current().can_start_statement() {
                    break;
                }
                self.parse_statement();
            }

            self.finish_node();
        }

        // Parse else branch
        if self.at(TokenKind::KwElse) {
            self.start_node(SyntaxKind::ElseBranch);
            self.bump();
            while !self.at(TokenKind::KwEndCase) && !self.at_end() && !self.at_stmt_list_end() {
                self.parse_statement();
            }
            self.finish_node();
        }

        if self.at(TokenKind::KwEndCase) {
            self.bump();
        } else {
            self.error("expected END_CASE");
        }

        self.finish_node();
    }

    fn parse_case_label(&mut self) {
        self.start_node(SyntaxKind::CaseLabel);
        self.parse_subrange();
        self.finish_node();
    }

    /// Parse FOR statement.
    pub(crate) fn parse_for_stmt(&mut self) {
        self.start_node(SyntaxKind::ForStmt);
        self.bump(); // FOR

        if self.at(TokenKind::Hash) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_name();
            } else {
                self.error("expected identifier after '#'");
            }
        } else if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected FOR control variable");
        }

        if self.at(TokenKind::Assign) {
            self.bump();
            self.parse_expression(); // start
        } else {
            self.error("expected ':=' after FOR control variable");
        }

        if self.at(TokenKind::KwTo) {
            self.bump();
            self.parse_expression(); // end
        } else {
            self.error("expected TO");
        }

        if self.at(TokenKind::KwBy) {
            self.bump();
            self.parse_expression(); // step
        }

        if self.at(TokenKind::KwDo) {
            self.bump();
        } else {
            self.error("expected DO");
        }

        while !self.at(TokenKind::KwEndFor) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }

        if self.at(TokenKind::KwEndFor) {
            self.bump();
        } else {
            self.error("expected END_FOR");
        }

        self.finish_node();
    }

    /// Parse WHILE statement.
    pub(crate) fn parse_while_stmt(&mut self) {
        self.start_node(SyntaxKind::WhileStmt);
        self.bump(); // WHILE

        self.parse_expression();

        if self.at(TokenKind::KwDo) {
            self.bump();
        } else {
            self.error("expected DO");
        }

        while !self.at(TokenKind::KwEndWhile) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }

        if self.at(TokenKind::KwEndWhile) {
            self.bump();
        } else {
            self.error("expected END_WHILE");
        }

        self.finish_node();
    }

    /// Parse REPEAT statement.
    pub(crate) fn parse_repeat_stmt(&mut self) {
        self.start_node(SyntaxKind::RepeatStmt);
        self.bump(); // REPEAT

        while !self.at(TokenKind::KwUntil) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }

        if self.at(TokenKind::KwUntil) {
            self.bump();
            self.parse_expression();
        } else {
            self.error("expected UNTIL");
        }

        if self.at(TokenKind::KwEndRepeat) {
            self.bump();
        } else {
            self.error("expected END_REPEAT");
        }

        self.finish_node();
    }

    /// Parse RETURN statement.
    pub(crate) fn parse_return_stmt(&mut self) {
        self.start_node(SyntaxKind::ReturnStmt);
        self.bump(); // RETURN

        if self.current().can_start_expr() {
            self.parse_expression();
        }

        self.expect_semicolon();

        self.finish_node();
    }

    /// Parse assignment or call statement.
    pub(crate) fn parse_assign_or_call_stmt(&mut self) {
        let is_assign = self.source.has_assign_ahead();
        if is_assign {
            self.start_node(SyntaxKind::AssignStmt);
        } else {
            self.start_node(SyntaxKind::ExprStmt);
        }

        self.parse_expression();

        if is_assign {
            if self.at(TokenKind::Assign) || self.at(TokenKind::RefAssign) {
                self.bump();
                self.parse_expression();
            } else {
                self.error("expected := or ?=");
            }
        }

        self.expect_semicolon();

        self.finish_node();
    }
}
