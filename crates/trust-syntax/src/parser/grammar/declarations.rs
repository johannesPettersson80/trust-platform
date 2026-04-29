//! Declaration parsing for IEC 61131-3 Structured Text.
//!
//! Handles:
//! - Variable blocks (VAR, VAR_INPUT, VAR_OUTPUT, etc.)
//! - Variable declarations with optional AT binding
//! - Type declarations (TYPE...END_TYPE)
//! - Struct, Union, Enum definitions
//! - Array, Pointer, Reference types

use crate::lexer::TokenKind;
use crate::parser::BoundedTopLevelScan;
use crate::syntax::SyntaxKind;

use super::super::Parser;

const POSITIONAL_INITIALIZER_DIAGNOSTIC: &str =
    "positional struct initializers are not supported; use named field initializers";
const DECLARATION_RECOVERY_LIMIT: usize = 512;
const DECLARATION_RECOVERY_BOUNDARIES: &[TokenKind] = &[
    TokenKind::Semicolon,
    TokenKind::KwEndVar,
    TokenKind::KwEndType,
    TokenKind::KwEndStruct,
    TokenKind::KwEndUnion,
    TokenKind::KwEndProgram,
    TokenKind::KwEndFunction,
    TokenKind::KwEndFunctionBlock,
    TokenKind::KwEndClass,
    TokenKind::KwEndConfiguration,
];

impl Parser<'_, '_> {
    /// Parse a TYPE declaration block.
    pub(crate) fn parse_type_decl(&mut self) {
        self.start_node(SyntaxKind::TypeDecl);
        self.bump(); // TYPE

        while !self.at(TokenKind::KwEndType) && !self.at_end() {
            if self.at(TokenKind::Ident) {
                self.parse_name();
            } else {
                self.error("expected type name");
                if !self.at_end() {
                    self.bump();
                }
                continue;
            }

            if self.at(TokenKind::Colon) {
                self.bump();
                self.parse_type_def();
                if self.at(TokenKind::Assign) {
                    self.bump();
                    self.parse_var_initializer();
                }
            } else {
                self.error("expected ':' after type name");
            }

            if self.at(TokenKind::Semicolon) {
                self.bump();
            }
        }

        if self.at(TokenKind::KwEndType) {
            self.bump();
        } else {
            self.error("expected END_TYPE");
        }

        self.finish_node();
    }

    /// Parse a type definition (struct, union, enum, array, or alias).
    pub(crate) fn parse_type_def(&mut self) {
        if self.at(TokenKind::KwStruct) {
            self.parse_struct_def();
        } else if self.at(TokenKind::KwUnion) {
            self.parse_union_def();
        } else if self.at(TokenKind::LParen) {
            self.parse_enum_def();
        } else if self.at_typed_enum_def() {
            self.parse_enum_def_with_base_type();
        } else if self.at(TokenKind::KwArray) {
            self.parse_array_type();
        } else {
            self.parse_type_ref();
        }
    }

    /// Parse a STRUCT definition.
    pub(crate) fn parse_struct_def(&mut self) {
        self.start_node(SyntaxKind::StructDef);
        self.bump(); // STRUCT

        while !self.at(TokenKind::KwEndStruct) && !self.at_end() {
            self.parse_var_decl();
        }

        if self.at(TokenKind::KwEndStruct) {
            self.bump();
        }

        self.finish_node();
    }

    /// Parse a UNION definition.
    pub(crate) fn parse_union_def(&mut self) {
        self.start_node(SyntaxKind::UnionDef);
        self.bump(); // UNION

        while !self.at(TokenKind::KwEndUnion) && !self.at_end() {
            self.parse_var_decl();
        }

        if self.at(TokenKind::KwEndUnion) {
            self.bump();
        }

        self.finish_node();
    }

    /// Parse an enumeration definition.
    pub(crate) fn parse_enum_def(&mut self) {
        self.start_node(SyntaxKind::EnumDef);
        self.bump(); // (

        while !self.at(TokenKind::RParen) && !self.at_end() {
            self.start_node(SyntaxKind::EnumValue);
            if self.at(TokenKind::Ident) {
                self.parse_name();
            }

            // Optional value assignment
            if self.at(TokenKind::Assign) {
                self.bump();
                self.parse_expression();
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
        }

        // Optional base type
        if self.current().is_type_keyword() || self.at(TokenKind::Ident) {
            self.parse_type_ref();
        }

        self.finish_node();
    }

    fn parse_enum_def_with_base_type(&mut self) {
        self.start_node(SyntaxKind::EnumDef);

        self.start_node(SyntaxKind::TypeRef);
        if self.current().is_type_keyword() {
            self.bump();
        } else if self.at(TokenKind::Ident) {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
        } else {
            self.error("expected base type for enum");
        }
        self.finish_node();

        if self.at(TokenKind::LParen) {
            self.bump(); // (
        } else {
            self.error("expected '(' after enum base type");
        }

        while !self.at(TokenKind::RParen) && !self.at_end() {
            self.start_node(SyntaxKind::EnumValue);
            if self.at(TokenKind::Ident) {
                self.parse_name();
            }

            // Optional value assignment
            if self.at(TokenKind::Assign) {
                self.bump();
                self.parse_expression();
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
        }

        self.finish_node();
    }

    fn at_typed_enum_def(&self) -> bool {
        if !(self.current().is_type_keyword() || self.at(TokenKind::Ident)) {
            return false;
        }

        let mut offset = 1;
        while self.peek_kind_n(offset) == TokenKind::Dot
            && self.peek_kind_n(offset + 1) == TokenKind::Ident
        {
            offset += 2;
        }

        if self.peek_kind_n(offset) != TokenKind::LParen {
            return false;
        }

        matches!(
            self.scan_top_level_ahead(
                offset + 1,
                &[TokenKind::Comma, TokenKind::Assign],
                &[TokenKind::RParen],
                &[
                    TokenKind::DotDot,
                    TokenKind::Semicolon,
                    TokenKind::KwEndType
                ],
                DECLARATION_RECOVERY_LIMIT,
            ),
            BoundedTopLevelScan::Found(TokenKind::Comma | TokenKind::Assign)
                | BoundedTopLevelScan::Closed(TokenKind::RParen)
        )
    }

    /// Parse an ARRAY type.
    pub(crate) fn parse_array_type(&mut self) {
        self.start_node(SyntaxKind::ArrayType);
        self.bump(); // ARRAY

        if self.at(TokenKind::LBracket) {
            self.bump();

            // Parse dimensions
            self.parse_subrange();

            while self.at(TokenKind::Comma) {
                self.bump();
                self.parse_subrange();
            }

            if self.at(TokenKind::RBracket) {
                self.bump();
            }
        }

        if self.at(TokenKind::KwOf) {
            self.bump();
            self.parse_type_ref();
        }

        self.finish_node();
    }

    /// Parse a subrange (e.g., 0..10).
    pub(crate) fn parse_subrange(&mut self) {
        self.start_node(SyntaxKind::Subrange);
        if self.at(TokenKind::Star) {
            self.start_node(SyntaxKind::Literal);
            self.bump();
            self.finish_node();
        } else {
            self.parse_expression();
        }

        if self.at(TokenKind::DotDot) {
            self.bump();
            if self.at(TokenKind::Star) {
                self.start_node(SyntaxKind::Literal);
                self.bump();
                self.finish_node();
            } else {
                self.parse_expression();
            }
        }

        self.finish_node();
    }

    /// Parse a VAR block.
    pub(crate) fn parse_var_block(&mut self) {
        self.start_node(SyntaxKind::VarBlock);
        self.bump(); // VAR, VAR_INPUT, etc.

        // Parse optional modifiers
        while matches!(
            self.current(),
            TokenKind::KwConstant
                | TokenKind::KwRetain
                | TokenKind::KwNonRetain
                | TokenKind::KwPersistent
        ) {
            self.bump();
        }

        // Parse optional access specifier (PUBLIC/PRIVATE/PROTECTED/INTERNAL)
        if matches!(
            self.current(),
            TokenKind::KwPublic
                | TokenKind::KwPrivate
                | TokenKind::KwProtected
                | TokenKind::KwInternal
        ) {
            self.bump();
        }

        while !self.at(TokenKind::KwEndVar) && !self.at_end() {
            self.parse_var_decl();
        }

        if self.at(TokenKind::KwEndVar) {
            self.bump();
        } else {
            self.error("expected END_VAR");
        }

        self.finish_node();
    }

    /// Parse a variable declaration.
    pub(crate) fn parse_var_decl(&mut self) {
        self.start_node(SyntaxKind::VarDecl);

        // Parse variable names
        if self.at(TokenKind::Ident) || self.at(TokenKind::KwEn) || self.at(TokenKind::KwEno) {
            self.parse_name();

            while self.at(TokenKind::Comma) {
                self.bump();
                if self.at(TokenKind::Ident)
                    || self.at(TokenKind::KwEn)
                    || self.at(TokenKind::KwEno)
                {
                    self.parse_name();
                }
            }
        } else {
            self.error("expected variable name");
            if !self.at_end() {
                self.bump();
            }
        }

        // Parse AT address binding (e.g., VAR x AT %IB0 : BOOL)
        if self.at(TokenKind::KwAt) {
            self.bump(); // AT
            if self.at(TokenKind::DirectAddress) {
                self.bump();
            } else {
                self.error("expected direct address after AT");
            }
        }

        // Parse type
        if self.at(TokenKind::Colon) {
            self.bump();
            self.parse_type_ref();
        }

        // Parse initializer
        if self.at(TokenKind::Assign) {
            self.bump();
            self.parse_var_initializer();
        }

        if self.at(TokenKind::Semicolon) {
            self.bump();
        }

        self.finish_node();
    }

    /// Parse an initializer in a declaration/default context.
    pub(crate) fn parse_var_initializer(&mut self) {
        if self.at_aggregate_initializer_start() || self.at_empty_initializer_start() {
            self.parse_initializer_list();
        } else if self.at_positional_initializer_start() {
            self.parse_positional_initializer_list();
        } else {
            self.parse_expression();
        }
    }

    fn at_aggregate_initializer_start(&self) -> bool {
        self.at(TokenKind::LParen)
            && matches!(
                self.peek_kind_n(1),
                TokenKind::Ident | TokenKind::KwEn | TokenKind::KwEno
            )
            && self.peek_kind_n(2) == TokenKind::Assign
    }

    fn at_empty_initializer_start(&self) -> bool {
        self.at(TokenKind::LParen) && self.peek_kind_n(1) == TokenKind::RParen
    }

    fn at_positional_initializer_start(&self) -> bool {
        self.at(TokenKind::LParen)
            && !self.at_aggregate_initializer_start()
            && matches!(
                self.scan_top_level_ahead(
                    1,
                    &[TokenKind::Comma],
                    &[TokenKind::RParen],
                    DECLARATION_RECOVERY_BOUNDARIES,
                    DECLARATION_RECOVERY_LIMIT,
                ),
                BoundedTopLevelScan::Found(TokenKind::Comma)
            )
    }

    fn parse_positional_initializer_list(&mut self) {
        let marker = self.start();
        self.bump(); // (
        self.error(POSITIONAL_INITIALIZER_DIAGNOSTIC);
        self.recover_top_level_until(
            &[TokenKind::RParen],
            DECLARATION_RECOVERY_BOUNDARIES,
            DECLARATION_RECOVERY_LIMIT,
            true,
        );

        marker.complete(self, SyntaxKind::InitializerList);
    }

    fn parse_initializer_list(&mut self) {
        let marker = self.start();
        self.bump(); // (

        if self.at(TokenKind::RParen) {
            self.error("empty aggregate initializers are not supported");
        } else {
            self.parse_initializer_element();
            while self.at(TokenKind::Comma) {
                self.bump();
                if self.at(TokenKind::RParen) {
                    self.error("expected initializer field");
                    break;
                }
                self.parse_initializer_element();
            }
        }

        if self.at(TokenKind::RParen) {
            self.bump();
        } else {
            self.error("expected )");
            self.recover_top_level_until(
                &[TokenKind::RParen],
                DECLARATION_RECOVERY_BOUNDARIES,
                DECLARATION_RECOVERY_LIMIT,
                true,
            );
        }

        marker.complete(self, SyntaxKind::InitializerList);
    }

    fn parse_initializer_element(&mut self) {
        if matches!(
            self.current(),
            TokenKind::Ident | TokenKind::KwEn | TokenKind::KwEno
        ) {
            self.parse_name();
            if self.at(TokenKind::Assign) {
                self.bump();
            } else {
                self.error("expected := in aggregate initializer");
            }
            if self.at(TokenKind::Comma)
                || self.at(TokenKind::RParen)
                || self.at(TokenKind::Semicolon)
                || self.at(TokenKind::KwEndVar)
                || self.at_end()
            {
                self.error("expected aggregate initializer value");
                return;
            }
            self.parse_var_initializer();
        } else {
            self.error(POSITIONAL_INITIALIZER_DIAGNOSTIC);
            self.recover_top_level_until(
                &[TokenKind::RParen],
                DECLARATION_RECOVERY_BOUNDARIES,
                DECLARATION_RECOVERY_LIMIT,
                false,
            );
        }
    }

    /// Parse a type reference.
    pub(crate) fn parse_type_ref(&mut self) {
        self.start_node(SyntaxKind::TypeRef);

        if self.at(TokenKind::KwArray) {
            self.parse_array_type();
        } else if self.at(TokenKind::KwPointer) {
            self.start_node(SyntaxKind::PointerType);
            self.bump();
            if self.at(TokenKind::KwTo) {
                self.bump();
            }
            self.parse_type_ref();
            self.finish_node();
        } else if self.at(TokenKind::KwRefTo) {
            self.start_node(SyntaxKind::ReferenceType);
            self.bump();
            self.parse_type_ref();
            self.finish_node();
        } else if self.at(TokenKind::KwString) || self.at(TokenKind::KwWString) {
            self.start_node(SyntaxKind::StringType);
            self.bump();
            if self.at(TokenKind::LBracket) {
                self.bump();
                self.parse_expression();
                if self.at(TokenKind::RBracket) {
                    self.bump();
                } else {
                    self.error("expected ]");
                }
            } else if self.at(TokenKind::LParen) {
                self.bump();
                self.parse_expression();
                if self.at(TokenKind::RParen) {
                    self.bump();
                } else {
                    self.error("expected )");
                }
            }
            self.finish_node();
        } else if self.current().is_type_keyword() {
            self.bump();
            self.parse_type_subrange();
        } else if self.at(TokenKind::Ident) {
            if self.peek_kind_n(1) == TokenKind::Dot {
                self.parse_qualified_name();
            } else {
                self.parse_name();
            }
            self.parse_type_subrange();
        } else {
            self.error("expected type");
        }

        self.finish_node();
    }

    fn parse_type_subrange(&mut self) {
        if !self.at(TokenKind::LParen) {
            return;
        }

        self.bump();
        self.parse_subrange();
        if self.at(TokenKind::RParen) {
            self.bump();
        } else {
            self.error("expected )");
        }
    }

    /// Parse a name (identifier).
    pub(crate) fn parse_name(&mut self) {
        self.start_node(SyntaxKind::Name);
        if self.at(TokenKind::Ident) || self.at(TokenKind::KwEn) || self.at(TokenKind::KwEno) {
            self.bump();
        }
        self.finish_node();
    }

    /// Parse a qualified name (e.g., Namespace.Type).
    pub(crate) fn parse_qualified_name(&mut self) {
        self.start_node(SyntaxKind::QualifiedName);
        if self.at(TokenKind::Ident) {
            self.parse_name();
        } else {
            self.error("expected name");
        }

        while self.at(TokenKind::Dot) {
            self.bump();
            if self.at(TokenKind::Ident) {
                self.parse_name();
            } else {
                self.error("expected name after '.'");
                break;
            }
        }

        self.finish_node();
    }
}
