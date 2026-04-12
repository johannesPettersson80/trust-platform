use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use crate::parser::Parser;

impl Parser<'_, '_> {
    pub(crate) fn parse_program(&mut self) {
        let is_test_program = self.at(TokenKind::KwTestProgram);
        let expected_end = if is_test_program {
            TokenKind::KwEndTestProgram
        } else {
            TokenKind::KwEndProgram
        };
        let alternate_end = if is_test_program {
            TokenKind::KwEndProgram
        } else {
            TokenKind::KwEndTestProgram
        };

        self.start_node(SyntaxKind::Program);
        self.bump(); // PROGRAM / TEST_PROGRAM

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected program name");
        }

        self.parse_using_directives();

        // Parse var blocks
        while self.current().is_var_keyword() {
            self.parse_var_block();
        }

        // Parse statements and actions in a statement list
        self.start_node(SyntaxKind::StmtList);
        while !self.at(expected_end)
            && !self.at(alternate_end)
            && !self.at_end()
            && !self.at_stmt_list_end()
        {
            if self.at(TokenKind::KwAction) {
                self.parse_action();
            } else {
                self.parse_statement();
            }
        }
        self.finish_node();

        if self.at(expected_end) {
            self.bump();
        } else {
            if is_test_program {
                self.error("expected END_TEST_PROGRAM");
            } else {
                self.error("expected END_PROGRAM");
            }
            if self.at(alternate_end) {
                self.bump();
            }
        }

        self.finish_node();
    }

    /// Parse a FUNCTION declaration.
    pub(crate) fn parse_function(&mut self) {
        self.start_node(SyntaxKind::Function);
        self.bump(); // FUNCTION

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected function name");
        }

        // Parse return type
        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_type_ref();
        }

        self.parse_using_directives();

        // Parse var blocks
        while self.current().is_var_keyword() {
            self.parse_var_block();
        }

        // Parse statements
        self.start_node(SyntaxKind::StmtList);
        while !self.at(TokenKind::KwEndFunction) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }
        self.finish_node();

        if self.at(TokenKind::KwEndFunction) {
            self.bump();
        } else {
            self.error("expected END_FUNCTION");
        }

        self.finish_node();
    }

    /// Parse a FUNCTION_BLOCK declaration.
    pub(crate) fn parse_function_block(&mut self) {
        let is_test_function_block = self.at(TokenKind::KwTestFunctionBlock);
        let expected_end = if is_test_function_block {
            TokenKind::KwEndTestFunctionBlock
        } else {
            TokenKind::KwEndFunctionBlock
        };
        let alternate_end = if is_test_function_block {
            TokenKind::KwEndFunctionBlock
        } else {
            TokenKind::KwEndTestFunctionBlock
        };

        self.start_node(SyntaxKind::FunctionBlock);
        self.bump(); // FUNCTION_BLOCK / TEST_FUNCTION_BLOCK

        if self.at(TokenKind::KwFinal) || self.at(TokenKind::KwAbstract) {
            self.bump();
        }

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected function block name");
        }

        self.parse_using_directives();

        // Parse EXTENDS clause
        if self.at(TokenKind::KwExtends) {
            self.parse_extends_clause();
        }

        // Parse IMPLEMENTS clause
        if self.at(TokenKind::KwImplements) {
            self.parse_implements_clause();
        }

        // Parse var blocks
        while self.current().is_var_keyword() {
            self.parse_var_block();
        }

        // Parse methods, properties, actions, and statements
        loop {
            if self.at(TokenKind::KwMethod) {
                self.parse_method();
            } else if self.at(TokenKind::KwProperty) {
                self.parse_property();
            } else if matches!(
                self.current(),
                TokenKind::KwPublic
                    | TokenKind::KwPrivate
                    | TokenKind::KwProtected
                    | TokenKind::KwInternal
            ) {
                if self.peek_kind_n(1) == TokenKind::KwProperty {
                    self.parse_property();
                } else {
                    self.parse_method();
                }
            } else if self.at(TokenKind::KwAction) {
                self.parse_action();
            } else if self.at(expected_end) || self.at(alternate_end) || self.at_end() {
                break;
            } else if self.current().can_start_statement() {
                self.start_node(SyntaxKind::StmtList);
                while self.current().can_start_statement()
                    && !self.at(expected_end)
                    && !self.at(alternate_end)
                    && !self.at_end()
                    && !self.at_stmt_list_end()
                {
                    self.parse_statement();
                }
                self.finish_node();
            } else {
                break;
            }
        }

        if self.at(expected_end) {
            self.bump();
        } else {
            if is_test_function_block {
                self.error("expected END_TEST_FUNCTION_BLOCK");
            } else {
                self.error("expected END_FUNCTION_BLOCK");
            }
            if self.at(alternate_end) {
                self.bump();
            }
        }

        self.finish_node();
    }

    // Parse a CONFIGURATION declaration.
}
