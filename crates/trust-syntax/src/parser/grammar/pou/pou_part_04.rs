use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use crate::parser::Parser;

impl Parser<'_, '_> {
    pub(crate) fn parse_config_init(&mut self) {
        self.start_node(SyntaxKind::ConfigInit);
        self.parse_access_path();

        if self.at(TokenKind::KwAt) {
            self.bump();
            if self.at(TokenKind::DirectAddress) {
                self.bump();
            } else {
                self.error("expected direct address after AT");
            }
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            if self.current().is_type_keyword() {
                self.parse_type_ref();
            } else if self.at(TokenKind::Ident) {
                self.start_node(SyntaxKind::TypeRef);
                if self.peek_kind_n(1) == TokenKind::Dot {
                    self.parse_qualified_name();
                } else {
                    self.parse_name();
                }
                self.finish_node();
            } else {
                self.error("expected type in VAR_CONFIG entry");
            }
        } else {
            self.error("expected ':' in VAR_CONFIG entry");
        }

        if self.at(TokenKind::Assign) {
            self.bump();
            self.parse_var_initializer();
        }

        self.finish_node();
    }

    /// Parse a CLASS declaration.
    pub(crate) fn parse_class(&mut self) {
        self.start_node(SyntaxKind::Class);
        self.bump(); // CLASS

        if self.at(TokenKind::KwFinal) || self.at(TokenKind::KwAbstract) {
            self.bump();
        }

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected class name");
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

        // Parse methods and properties
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
            } else if self.current().is_trivia() {
                self.bump();
            } else if self.at(TokenKind::KwEndClass) || self.at_end() {
                break;
            } else {
                self.error("expected METHOD, PROPERTY, or END_CLASS");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndClass) {
            self.bump();
        } else {
            self.error("expected END_CLASS");
        }

        self.finish_node();
    }

    /// Parse an INTERFACE declaration.
    pub(crate) fn parse_interface(&mut self) {
        self.start_node(SyntaxKind::Interface);
        self.bump(); // INTERFACE

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected interface name");
        }

        // Parse EXTENDS clause
        if self.at(TokenKind::KwExtends) {
            self.parse_extends_clause();
        }

        // Parse method and property signatures
        while !self.at(TokenKind::KwEndInterface) && !self.at_end() {
            if self.at(TokenKind::KwMethod) {
                self.parse_method_signature();
            } else if self.at(TokenKind::KwProperty) {
                self.parse_property_signature();
            } else if matches!(
                self.current(),
                TokenKind::KwPublic
                    | TokenKind::KwPrivate
                    | TokenKind::KwProtected
                    | TokenKind::KwInternal
            ) {
                if self.peek_kind_n(1) == TokenKind::KwProperty {
                    self.parse_property_signature();
                } else {
                    self.parse_method_signature();
                }
            } else {
                self.error("expected METHOD or PROPERTY in interface");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndInterface) {
            self.bump();
        } else {
            self.error("expected END_INTERFACE");
        }

        self.finish_node();
    }

    /// Parse a NAMESPACE declaration.
    pub(crate) fn parse_namespace(&mut self) {
        self.start_node(SyntaxKind::Namespace);
        self.bump(); // NAMESPACE

        if self.at(TokenKind::KwInternal) {
            self.bump();
        }

        if self.at(TokenKind::Ident) {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
        } else {
            self.error("expected namespace name");
        }

        self.parse_using_directives();

        // Parse namespace contents
        while !self.at(TokenKind::KwEndNamespace) && !self.at_end() {
            if self.at(TokenKind::KwProgram) || self.at(TokenKind::KwTestProgram) {
                self.parse_program();
            } else if self.at(TokenKind::KwFunction) {
                self.parse_function();
            } else if self.at(TokenKind::KwVarGlobal) {
                self.parse_var_block();
            } else if self.at(TokenKind::KwFunctionBlock) || self.at(TokenKind::KwTestFunctionBlock)
            {
                self.parse_function_block();
            } else if self.at(TokenKind::KwClass) {
                self.parse_class();
            } else if self.at(TokenKind::KwInterface) {
                self.parse_interface();
            } else if self.at(TokenKind::KwType) {
                self.parse_type_decl();
            } else if self.at(TokenKind::KwNamespace) {
                self.parse_namespace();
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected declaration in namespace");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndNamespace) {
            self.bump();
        } else {
            self.error("expected END_NAMESPACE");
        }

        self.finish_node();
    }

    // Parse a METHOD declaration.
}
