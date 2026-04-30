use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use crate::parser::Parser;

impl Parser<'_, '_> {
    pub(crate) fn parse_method(&mut self) {
        self.start_node(SyntaxKind::Method);

        // Parse access modifier
        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        if self.at(TokenKind::KwMethod) {
            self.bump();
        } else {
            self.error("expected METHOD");
        }

        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        if self.at(TokenKind::KwFinal) || self.at(TokenKind::KwAbstract) {
            self.bump();
        }

        if self.at(TokenKind::KwOverride) {
            self.bump();
        }

        if self.at_name_token() {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
        } else {
            self.error("expected method name");
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
        while !self.at(TokenKind::KwEndMethod) && !self.at_end() && !self.at_stmt_list_end() {
            self.parse_statement();
        }
        self.finish_node();

        if self.at(TokenKind::KwEndMethod) {
            self.bump();
        }

        self.finish_node();
    }

    /// Parse a method signature (for interfaces).
    pub(crate) fn parse_method_signature(&mut self) {
        self.start_node(SyntaxKind::Method);

        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        if self.at(TokenKind::KwMethod) {
            self.bump();
        } else {
            self.error("expected METHOD");
        }

        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        if self.at(TokenKind::KwFinal) || self.at(TokenKind::KwAbstract) {
            self.bump();
        }

        if self.at(TokenKind::KwOverride) {
            self.bump();
        }

        if self.at_name_token() {
            self.parse_name();
        } else {
            self.error("expected method name");
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_type_ref();
        }

        while self.current().is_var_keyword() {
            self.parse_var_block();
        }

        if self.at(TokenKind::KwEndMethod) {
            self.bump();
        } else {
            self.error("expected END_METHOD");
        }

        self.finish_node();
    }

    /// Parse a PROPERTY declaration.
    pub(crate) fn parse_property(&mut self) {
        self.start_node(SyntaxKind::Property);

        // Parse access modifier
        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        self.bump(); // PROPERTY

        if self.at(TokenKind::Ident) {
            self.parse_name();
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_type_ref();
        }

        // Parse GET accessor
        if self.at(TokenKind::KwGet) {
            self.start_node(SyntaxKind::PropertyGet);
            self.bump();
            self.start_node(SyntaxKind::StmtList);
            while !self.at(TokenKind::KwEndGet)
                && !self.at(TokenKind::KwSet)
                && !self.at(TokenKind::KwEndProperty)
                && !self.at_end()
                && !self.at_stmt_list_end()
            {
                self.parse_statement();
            }
            self.finish_node(); // StmtList
            if self.at(TokenKind::KwEndGet) {
                self.bump();
            }
            self.finish_node(); // PropertyGet
        }

        // Parse SET accessor
        if self.at(TokenKind::KwSet) {
            self.start_node(SyntaxKind::PropertySet);
            self.bump();
            self.start_node(SyntaxKind::StmtList);
            while !self.at(TokenKind::KwEndSet)
                && !self.at(TokenKind::KwEndProperty)
                && !self.at_end()
                && !self.at_stmt_list_end()
            {
                self.parse_statement();
            }
            self.finish_node(); // StmtList
            if self.at(TokenKind::KwEndSet) {
                self.bump();
            }
            self.finish_node(); // PropertySet
        }

        if self.at(TokenKind::KwEndProperty) {
            self.bump();
        }

        self.finish_node();
    }

    // Parse a property signature (for interfaces).
}
