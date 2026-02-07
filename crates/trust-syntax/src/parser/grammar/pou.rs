//! Program Organization Unit (POU) parsing.
//!
//! Handles:
//! - PROGRAM / END_PROGRAM
//! - TEST_PROGRAM / END_TEST_PROGRAM (extension)
//! - FUNCTION / END_FUNCTION
//! - FUNCTION_BLOCK / END_FUNCTION_BLOCK
//! - TEST_FUNCTION_BLOCK / END_TEST_FUNCTION_BLOCK (extension)
//! - CLASS / END_CLASS
//! - CONFIGURATION / END_CONFIGURATION
//! - RESOURCE / END_RESOURCE
//! - TASK / PROGRAM (configuration)
//! - METHOD / END_METHOD
//! - PROPERTY / END_PROPERTY (with GET/SET)
//! - INTERFACE / END_INTERFACE
//! - NAMESPACE / END_NAMESPACE
//! - ACTION / END_ACTION

use crate::lexer::TokenKind;
use crate::syntax::SyntaxKind;

use super::super::Parser;

impl Parser<'_, '_> {
    /// Parse a PROGRAM declaration.
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
                self.parse_statement();
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

    /// Parse a CONFIGURATION declaration.
    pub(crate) fn parse_configuration(&mut self) {
        self.start_node(SyntaxKind::Configuration);
        self.bump(); // CONFIGURATION

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected configuration name");
        }

        while !self.at(TokenKind::KwEndConfiguration) && !self.at_end() {
            if self.at(TokenKind::KwVarAccess) {
                self.parse_var_access_block();
            } else if self.at(TokenKind::KwVarConfig) {
                self.parse_var_config_block();
            } else if self.current().is_var_keyword() {
                self.parse_var_block();
            } else if self.at(TokenKind::KwResource) {
                self.parse_resource();
            } else if self.at(TokenKind::KwTask) {
                self.parse_task_config();
            } else if self.at(TokenKind::KwProgram) {
                self.parse_program_config();
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected RESOURCE, TASK, PROGRAM, or VAR block");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndConfiguration) {
            self.bump();
        } else {
            self.error("expected END_CONFIGURATION");
        }

        self.finish_node();
    }

    /// Parse a RESOURCE declaration.
    pub(crate) fn parse_resource(&mut self) {
        self.start_node(SyntaxKind::Resource);
        self.bump(); // RESOURCE

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected resource name");
        }

        if self.at(TokenKind::KwOn) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_qualified_name();
            } else {
                self.error("expected resource type name after ON");
            }
        }

        while !self.at(TokenKind::KwEndResource) && !self.at_end() {
            if self.at(TokenKind::KwVarAccess) {
                self.parse_var_access_block();
            } else if self.current().is_var_keyword() {
                self.parse_var_block();
            } else if self.at(TokenKind::KwTask) {
                self.parse_task_config();
            } else if self.at(TokenKind::KwProgram) {
                self.parse_program_config();
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected TASK, PROGRAM, or VAR block in RESOURCE");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndResource) {
            self.bump();
        } else {
            self.error("expected END_RESOURCE");
        }

        self.finish_node();
    }

    /// Parse a TASK configuration.
    pub(crate) fn parse_task_config(&mut self) {
        self.start_node(SyntaxKind::TaskConfig);
        self.bump(); // TASK

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected task name");
        }

        if self.at(TokenKind::LParen) {
            self.start_node(SyntaxKind::TaskInit);
            self.bump();
            while !self.at(TokenKind::RParen) && !self.at_end() {
                if self.at(TokenKind::Ident) {
                    self.parse_name();
                    if self.at(TokenKind::Assign) {
                        self.bump();
                        self.parse_expression();
                    }
                } else if self.at(TokenKind::Comma) {
                    self.bump();
                    continue;
                } else if self.current().is_trivia() {
                    self.bump();
                } else {
                    self.error("expected task init element");
                    self.bump();
                }
            }
            if self.at(TokenKind::RParen) {
                self.bump();
            } else {
                self.error("expected ')'");
            }
            self.finish_node();
        }

        if self.at(TokenKind::Semicolon) {
            self.bump();
        }

        self.finish_node();
    }

    /// Parse a PROGRAM configuration.
    pub(crate) fn parse_program_config(&mut self) {
        self.start_node(SyntaxKind::ProgramConfig);
        self.bump(); // PROGRAM

        if self.at(TokenKind::KwRetain) || self.at(TokenKind::KwNonRetain) {
            self.bump();
        }

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected program name");
        }

        if self.at(TokenKind::KwWith) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_name();
            } else {
                self.error("expected task name after WITH");
            }
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_qualified_name();
            } else if self.current().is_type_keyword() {
                self.parse_type_ref();
            } else {
                self.error("expected program type");
            }
        } else {
            self.error("expected ':' after program name");
        }

        if self.at(TokenKind::LParen) {
            self.parse_program_config_list();
        }

        if self.at(TokenKind::Semicolon) {
            self.bump();
        }

        self.finish_node();
    }

    fn parse_program_config_list(&mut self) {
        self.start_node(SyntaxKind::ProgramConfigList);
        self.bump(); // (

        while !self.at(TokenKind::RParen) && !self.at_end() {
            self.start_node(SyntaxKind::ProgramConfigElem);

            if self.at(TokenKind::Ident) || self.at(TokenKind::DirectAddress) {
                self.parse_access_path();
                if self.at(TokenKind::KwWith) {
                    self.bump();
                    if self.at(TokenKind::Ident) {
                        self.parse_name();
                    } else {
                        self.error("expected task name after WITH");
                    }
                } else if self.at(TokenKind::Assign) || self.at(TokenKind::Arrow) {
                    self.bump();
                    self.parse_expression();
                }
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected program configuration element");
                self.bump();
            }

            self.finish_node();

            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }

        if self.at(TokenKind::RParen) {
            self.bump();
        } else {
            self.error("expected ')'");
        }

        self.finish_node();
    }

    fn parse_var_access_block(&mut self) {
        self.start_node(SyntaxKind::VarAccessBlock);
        self.bump(); // VAR_ACCESS

        while !self.at(TokenKind::KwEndVar) && !self.at_end() {
            if self.at(TokenKind::Ident) {
                self.parse_access_decl();
                if self.at(TokenKind::Semicolon) {
                    self.bump();
                } else {
                    self.error("expected ';' after VAR_ACCESS entry");
                }
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected access declaration");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndVar) {
            self.bump();
        } else {
            self.error("expected END_VAR");
        }

        self.finish_node();
    }

    fn parse_access_decl(&mut self) {
        self.start_node(SyntaxKind::AccessDecl);

        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected access name");
        }

        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_access_path();
        } else {
            self.error("expected ':' after access name");
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
                self.error("expected type in access declaration");
            }
        } else {
            self.error("expected ':' before access type");
        }

        if self.at(TokenKind::KwReadWrite) || self.at(TokenKind::KwReadOnly) {
            self.bump();
        } else if self.at(TokenKind::Ident) {
            let text = self.source.current_text().to_ascii_uppercase();
            if text == "READ_WRITE" || text == "READ_ONLY" {
                self.bump();
            }
        }

        self.finish_node();
    }

    fn parse_access_path(&mut self) {
        self.start_node(SyntaxKind::AccessPath);

        if self.at(TokenKind::DirectAddress) {
            self.bump();
        } else if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected access path");
            self.finish_node();
            return;
        }

        loop {
            if self.at(TokenKind::LBracket) {
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
                continue;
            }

            if self.at(TokenKind::Dot) {
                self.bump();
                if self.at(TokenKind::DirectAddress) || self.at(TokenKind::IntLiteral) {
                    self.bump();
                } else if self.at(TokenKind::Ident) {
                    self.parse_name();
                } else {
                    self.error("expected name, integer, or direct address after '.'");
                }
                continue;
            }

            break;
        }

        self.finish_node();
    }

    fn parse_var_config_block(&mut self) {
        self.start_node(SyntaxKind::VarConfigBlock);
        self.bump(); // VAR_CONFIG

        while !self.at(TokenKind::KwEndVar) && !self.at_end() {
            if self.at(TokenKind::Ident) || self.at(TokenKind::DirectAddress) {
                self.parse_config_init();
                if self.at(TokenKind::Semicolon) {
                    self.bump();
                } else {
                    self.error("expected ';' after VAR_CONFIG entry");
                }
            } else if self.current().is_trivia() {
                self.bump();
            } else {
                self.error("expected VAR_CONFIG entry");
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndVar) {
            self.bump();
        } else {
            self.error("expected END_VAR");
        }

        self.finish_node();
    }

    fn parse_config_init(&mut self) {
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
            self.parse_expression();
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

    /// Parse a METHOD declaration.
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

        if self.at(TokenKind::Ident) {
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

        if self.at(TokenKind::Ident) {
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

    /// Parse a property signature (for interfaces).
    pub(crate) fn parse_property_signature(&mut self) {
        self.start_node(SyntaxKind::Property);

        if matches!(
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

    fn parse_using_directives(&mut self) {
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
