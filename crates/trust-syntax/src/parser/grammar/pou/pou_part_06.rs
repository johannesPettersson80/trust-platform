use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use crate::parser::Parser;

impl Parser<'_, '_> {
    pub(crate) fn parse_property_signature(&mut self) {
        self.start_node(SyntaxKind::Property);

        while matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        if self.at(TokenKind::KwProperty) {
            self.bump();
        } else {
            self.error("expected PROPERTY");
        }

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected property name");
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_type_ref();
        }

        if self.at(TokenKind::KwGet) {
            self.start_node(SyntaxKind::PropertyGet);
            self.bump();
            if self.at(TokenKind::KwEndGet) {
                self.bump();
            } else {
                self.error("expected END_GET");
            }
            self.finish_node();
        }

        if self.at(TokenKind::KwSet) {
            self.start_node(SyntaxKind::PropertySet);
            self.bump();
            if self.at(TokenKind::KwEndSet) {
                self.bump();
            } else {
                self.error("expected END_SET");
            }
            self.finish_node();
        }

        if self.at(TokenKind::KwEndProperty) {
            self.bump();
        } else {
            self.error("expected END_PROPERTY");
        }

        self.finish_node();
    }

    /// Parse an ACTION declaration.
    pub(crate) fn parse_action(&mut self) {
        self.start_node(SyntaxKind::Action);
        self.bump(); // ACTION

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected action name");
        }

        if self.at(TokenKind::Colon) {
            self.bump();
        } else {
            self.error("expected ':' after action name");
        }

        // Parse statements
        self.start_node(SyntaxKind::StmtList);
        while !self.at(TokenKind::KwEndAction) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }
        self.finish_node();

        if self.at(TokenKind::KwEndAction) {
            self.bump();
        } else {
            self.error("expected END_ACTION");
        }

        self.finish_node();
    }

    pub(crate) fn parse_using_directives(&mut self) {
        while self.at(TokenKind::KwUsing) {
            self.parse_using_directive();
        }
    }

    pub(crate) fn parse_using_directive(&mut self) {
        self.start_node(SyntaxKind::UsingDirective);
        self.bump(); // USING

        if self.at(TokenKind::Ident) {
            self.parse_qualified_name();
        } else {
            self.error("expected namespace name after USING");
        }

        while self.at(TokenKind::Comma) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_qualified_name();
            } else {
                self.error("expected namespace name after ','");
                break;
            }
        }

        self.expect_semicolon();
        self.finish_node();
    }

    /// Parse EXTENDS clause.
    pub(crate) fn parse_extends_clause(&mut self) {
        self.start_node(SyntaxKind::ExtendsClause);
        self.bump(); // EXTENDS
        if self.at(TokenKind::Ident) {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
        }
        self.finish_node();
    }

    /// Parse IMPLEMENTS clause.
    pub(crate) fn parse_implements_clause(&mut self) {
        self.start_node(SyntaxKind::ImplementsClause);
        self.bump(); // IMPLEMENTS

        if self.at(TokenKind::Ident) {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
        }

        while self.at(TokenKind::Comma) {
            self.bump();
            if self.at(TokenKind::Ident) {
                if self.peek_kind_n(1) == TokenKind::Dot {
                    self.parse_qualified_name();
                } else {
                    self.parse_name();
                }
            }
        }

        self.finish_node();
    }
}
