//! Syntax tree types for IEC 61131-3 Structured Text.
//!
//! This module provides the `rowan`-based syntax tree implementation,
//! including the `SyntaxKind` enum that covers both tokens and composite nodes.

use crate::lexer::TokenKind;
use crate::token_kinds::for_each_token_kind;

macro_rules! define_syntax_kind {
    ($($token:ident),* $(,)?) => {
        /// All syntax node and token kinds in IEC 61131-3 Structured Text.
        ///
        /// This enum includes both token kinds (from the lexer) and composite
        /// node kinds (produced by the parser).
        // Variants mirror lexer/token names; documenting each would be noisy.
        #[allow(missing_docs)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind {
            // =========================================================================
            // TOKEN KINDS (mirrors TokenKind)
            // =========================================================================
            $($token,)*

            // COMPOSITE NODE KINDS (produced by parser)
            // =========================================================================
            /// Root node of a source file
            SourceFile,

            /// A program declaration: `PROGRAM name ... END_PROGRAM`
            Program,

            /// A function declaration: `FUNCTION name : type ... END_FUNCTION`
            Function,

            /// A function block declaration: `FUNCTION_BLOCK name ... END_FUNCTION_BLOCK`
            FunctionBlock,

            /// A class declaration: `CLASS name ... END_CLASS`
            Class,

            /// A method declaration: `METHOD name ... END_METHOD`
            Method,

            /// A property declaration: `PROPERTY name : type ... END_PROPERTY`
            Property,

            /// A property getter: `GET ... END_GET`
            PropertyGet,

            /// A property setter: `SET ... END_SET`
            PropertySet,

            /// An interface declaration: `INTERFACE name ... END_INTERFACE`
            Interface,

            /// A namespace declaration: `NAMESPACE name ... END_NAMESPACE`
            Namespace,

            /// A USING directive: `USING Namespace.Name;`
            UsingDirective,

            /// A configuration declaration: `CONFIGURATION name ... END_CONFIGURATION`
            Configuration,

            /// A resource declaration: `RESOURCE name ... END_RESOURCE`
            Resource,

            /// An action declaration: `ACTION name ... END_ACTION`
            Action,

            /// A task configuration: `TASK name (...)`
            TaskConfig,

            /// A task initialization: `(INTERVAL := ..., PRIORITY := ...)`
            TaskInit,

            /// A program configuration: `PROGRAM name : Type (...)`
            ProgramConfig,

            /// A program configuration list: `(elem, elem, ...)`
            ProgramConfigList,

            /// A program configuration element
            ProgramConfigElem,

            /// VAR_ACCESS block
            VarAccessBlock,

            /// Access declaration inside VAR_ACCESS
            AccessDecl,

            /// Access path inside VAR_ACCESS
            AccessPath,

            /// VAR_CONFIG block
            VarConfigBlock,

            /// Config initialization entry inside VAR_CONFIG
            ConfigInit,

            /// A type declaration: `TYPE name : ... END_TYPE`
            TypeDecl,

            /// A struct definition: `STRUCT ... END_STRUCT`
            StructDef,

            /// A union definition: `UNION ... END_UNION`
            UnionDef,

            /// An enum definition: `(val1, val2, ...)`
            EnumDef,

            /// An enum value
            EnumValue,

            /// An array type: `ARRAY[...] OF type`
            ArrayType,

            /// A pointer type: `POINTER TO type`
            PointerType,

            /// A reference type: `REF_TO type` or `REFERENCE TO type`
            ReferenceType,

            /// A string type with optional length: `STRING[80]`
            StringType,

            /// A subrange: `1..10`
            Subrange,

            /// Variable block: `VAR ... END_VAR`, `VAR_INPUT ... END_VAR`, etc.
            VarBlock,

            /// Variable declaration: `name : type := initializer;`
            VarDecl,

            /// Variable list: `a, b, c`
            VarList,

            /// Extends clause: `EXTENDS BaseClass`
            ExtendsClause,

            /// Implements clause: `IMPLEMENTS I_Interface, I_Other`
            ImplementsClause,

            /// A name (identifier)
            Name,

            /// A qualified name: `Namespace.Type`
            QualifiedName,

            /// A type reference
            TypeRef,

            /// Parameter list in declaration
            ParamList,

            /// Single parameter
            Param,

            /// Argument list in call
            ArgList,

            /// Single argument (may be named: `param := value`)
            Arg,

            /// Statement list
            StmtList,

            /// Assignment statement: `x := expr;`
            AssignStmt,

            /// If statement: `IF ... THEN ... END_IF`
            IfStmt,

            /// Elsif branch
            ElsifBranch,

            /// Else branch
            ElseBranch,

            /// Case statement: `CASE expr OF ... END_CASE`
            CaseStmt,

            /// Case branch: `1, 2, 3: statements`
            CaseBranch,

            /// Case label
            CaseLabel,

            /// For statement: `FOR i := 1 TO 10 BY 1 DO ... END_FOR`
            ForStmt,

            /// While statement: `WHILE cond DO ... END_WHILE`
            WhileStmt,

            /// Repeat statement: `REPEAT ... UNTIL cond END_REPEAT`
            RepeatStmt,

            /// Return statement: `RETURN;` or `RETURN expr;`
            ReturnStmt,

            /// Exit statement: `EXIT;`
            ExitStmt,

            /// Continue statement: `CONTINUE;`
            ContinueStmt,

            /// Jump statement: `JMP label;`
            JmpStmt,

            /// Label statement: `Label: statement`
            LabelStmt,

            /// Empty statement: `;`
            EmptyStmt,

            /// Expression statement (call without assignment)
            ExprStmt,

            // Expressions
            /// Binary expression: `a + b`
            BinaryExpr,

            /// Unary expression: `-x`, `NOT x`
            UnaryExpr,

            /// Parenthesized expression: `(expr)`
            ParenExpr,

            /// Function/method call: `func(args)`
            CallExpr,

            /// Index expression: `arr[i]`
            IndexExpr,

            /// Field access: `struct.field`
            FieldExpr,

            /// Dereference: `ptr^`
            DerefExpr,

            /// Address-of: `ADR(var)`
            AddrExpr,

            /// Sizeof: `SIZEOF(type | storage)`
            SizeOfExpr,

            /// Name reference (variable, constant, etc.)
            NameRef,

            /// Literal value
            Literal,

            /// This reference: `THIS`
            ThisExpr,

            /// Super reference: `SUPER`
            SuperExpr,

            /// Initializer list: `(a := 1, b := 2)`
            InitializerList,

            /// Array initializer: `[1, 2, 3]`
            ArrayInitializer,

            /// Condition expression (for IF, WHILE, etc.)
            Condition,
        }
    };
}

for_each_token_kind!(define_syntax_kind);

/// Syntax node kinds that declare a POU-like semantic owner.
pub const POU_DECLARATION_KINDS: &[SyntaxKind] = &[
    SyntaxKind::Program,
    SyntaxKind::Function,
    SyntaxKind::FunctionBlock,
    SyntaxKind::Class,
    SyntaxKind::Method,
    SyntaxKind::Property,
    SyntaxKind::Interface,
];

impl SyntaxKind {
    /// Returns `true` if this is a trivia kind.
    #[must_use]
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::Pragma
        )
    }

    /// Returns `true` if this is a token kind (not a composite node).
    #[must_use]
    pub fn is_token(self) -> bool {
        (self as u16) <= (Self::Eof as u16)
    }

    /// Returns `true` if this is a composite node kind.
    #[must_use]
    pub fn is_node(self) -> bool {
        !self.is_token()
    }

    /// Returns `true` for generic expression syntax nodes.
    ///
    /// Aggregate-only initializer nodes are intentionally excluded here; callers
    /// that operate in initializer position should use
    /// [`SyntaxKind::is_initializer_expression_node`].
    #[must_use]
    pub fn is_expression_node(self) -> bool {
        if self.is_trivia() {
            return false;
        }
        matches!(
            self,
            Self::Literal
                | Self::NameRef
                | Self::BinaryExpr
                | Self::UnaryExpr
                | Self::CallExpr
                | Self::IndexExpr
                | Self::FieldExpr
                | Self::DerefExpr
                | Self::AddrExpr
                | Self::ParenExpr
                | Self::ThisExpr
                | Self::SuperExpr
                | Self::SizeOfExpr
        )
    }

    /// Returns `true` for aggregate initializer nodes.
    #[must_use]
    pub fn is_aggregate_initializer_node(self) -> bool {
        if self.is_trivia() {
            return false;
        }
        matches!(self, Self::InitializerList | Self::ArrayInitializer)
    }

    /// Returns `true` for syntax that can appear as an initializer RHS.
    #[must_use]
    pub fn is_initializer_expression_node(self) -> bool {
        self.is_expression_node() || self.is_aggregate_initializer_node()
    }

    /// Returns `true` for syntax nodes that declare a POU-like semantic owner.
    #[must_use]
    pub fn is_pou_declaration(self) -> bool {
        POU_DECLARATION_KINDS.contains(&self)
    }

    /// Returns `true` for statement syntax nodes.
    #[must_use]
    pub fn is_statement_node(self) -> bool {
        if self.is_trivia() {
            return false;
        }
        matches!(
            self,
            Self::AssignStmt
                | Self::IfStmt
                | Self::ForStmt
                | Self::WhileStmt
                | Self::RepeatStmt
                | Self::CaseStmt
                | Self::ReturnStmt
                | Self::ExprStmt
                | Self::ExitStmt
                | Self::ContinueStmt
                | Self::JmpStmt
                | Self::LabelStmt
                | Self::EmptyStmt
        )
    }
}

macro_rules! map_token_kinds {
    ($($name:ident),* $(,)?) => {
        impl From<TokenKind> for SyntaxKind {
            fn from(kind: TokenKind) -> Self {
                match kind {
                    $(TokenKind::$name => SyntaxKind::$name,)*
                }
            }
        }
    };
}

for_each_token_kind!(map_token_kinds);

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// The language type for Structured Text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StLanguage {}

macro_rules! define_syntax_kinds {
    ($($token:ident),* $(,)?) => {
        const SYNTAX_KINDS: &[SyntaxKind] = &[
            $(SyntaxKind::$token,)*
            SyntaxKind::SourceFile,
            SyntaxKind::Program,
            SyntaxKind::Function,
            SyntaxKind::FunctionBlock,
            SyntaxKind::Class,
            SyntaxKind::Method,
            SyntaxKind::Property,
            SyntaxKind::PropertyGet,
            SyntaxKind::PropertySet,
            SyntaxKind::Interface,
            SyntaxKind::Namespace,
            SyntaxKind::UsingDirective,
            SyntaxKind::Configuration,
            SyntaxKind::Resource,
            SyntaxKind::Action,
            SyntaxKind::TaskConfig,
            SyntaxKind::TaskInit,
            SyntaxKind::ProgramConfig,
            SyntaxKind::ProgramConfigList,
            SyntaxKind::ProgramConfigElem,
            SyntaxKind::VarAccessBlock,
            SyntaxKind::AccessDecl,
            SyntaxKind::AccessPath,
            SyntaxKind::VarConfigBlock,
            SyntaxKind::ConfigInit,
            SyntaxKind::TypeDecl,
            SyntaxKind::StructDef,
            SyntaxKind::UnionDef,
            SyntaxKind::EnumDef,
            SyntaxKind::EnumValue,
            SyntaxKind::ArrayType,
            SyntaxKind::PointerType,
            SyntaxKind::ReferenceType,
            SyntaxKind::StringType,
            SyntaxKind::Subrange,
            SyntaxKind::VarBlock,
            SyntaxKind::VarDecl,
            SyntaxKind::VarList,
            SyntaxKind::ExtendsClause,
            SyntaxKind::ImplementsClause,
            SyntaxKind::Name,
            SyntaxKind::QualifiedName,
            SyntaxKind::TypeRef,
            SyntaxKind::ParamList,
            SyntaxKind::Param,
            SyntaxKind::ArgList,
            SyntaxKind::Arg,
            SyntaxKind::StmtList,
            SyntaxKind::AssignStmt,
            SyntaxKind::IfStmt,
            SyntaxKind::ElsifBranch,
            SyntaxKind::ElseBranch,
            SyntaxKind::CaseStmt,
            SyntaxKind::CaseBranch,
            SyntaxKind::CaseLabel,
            SyntaxKind::ForStmt,
            SyntaxKind::WhileStmt,
            SyntaxKind::RepeatStmt,
            SyntaxKind::ReturnStmt,
            SyntaxKind::ExitStmt,
            SyntaxKind::ContinueStmt,
            SyntaxKind::JmpStmt,
            SyntaxKind::LabelStmt,
            SyntaxKind::EmptyStmt,
            SyntaxKind::ExprStmt,
            SyntaxKind::BinaryExpr,
            SyntaxKind::UnaryExpr,
            SyntaxKind::ParenExpr,
            SyntaxKind::CallExpr,
            SyntaxKind::IndexExpr,
            SyntaxKind::FieldExpr,
            SyntaxKind::DerefExpr,
            SyntaxKind::AddrExpr,
            SyntaxKind::SizeOfExpr,
            SyntaxKind::NameRef,
            SyntaxKind::Literal,
            SyntaxKind::ThisExpr,
            SyntaxKind::SuperExpr,
            SyntaxKind::InitializerList,
            SyntaxKind::ArrayInitializer,
            SyntaxKind::Condition,
        ];
    };
}

for_each_token_kind!(define_syntax_kinds);

impl rowan::Language for StLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SYNTAX_KINDS
            .get(raw.0 as usize)
            .copied()
            .unwrap_or(SyntaxKind::Error)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// A syntax node in the ST syntax tree.
pub type SyntaxNode = rowan::SyntaxNode<StLanguage>;

/// A syntax token in the ST syntax tree.
pub type SyntaxToken = rowan::SyntaxToken<StLanguage>;

/// A syntax element (either node or token) in the ST syntax tree.
pub type SyntaxElement = rowan::SyntaxElement<StLanguage>;

/// A builder for syntax trees.
pub type SyntaxTreeBuilder = rowan::GreenNodeBuilder<'static>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_kind_to_syntax_kind() {
        assert_eq!(
            SyntaxKind::from(TokenKind::KwFunctionBlock),
            SyntaxKind::KwFunctionBlock
        );
        assert_eq!(SyntaxKind::from(TokenKind::Ident), SyntaxKind::Ident);
        assert_eq!(SyntaxKind::from(TokenKind::Assign), SyntaxKind::Assign);
    }

    #[test]
    fn test_is_trivia() {
        assert!(SyntaxKind::Whitespace.is_trivia());
        assert!(SyntaxKind::LineComment.is_trivia());
        assert!(SyntaxKind::BlockComment.is_trivia());
        assert!(SyntaxKind::Pragma.is_trivia());
        assert!(!SyntaxKind::Ident.is_trivia());
    }

    #[test]
    fn test_is_token_vs_node() {
        assert!(SyntaxKind::Ident.is_token());
        assert!(SyntaxKind::KwIf.is_token());
        assert!(!SyntaxKind::IfStmt.is_token());
        assert!(!SyntaxKind::FunctionBlock.is_token());

        assert!(!SyntaxKind::Ident.is_node());
        assert!(SyntaxKind::IfStmt.is_node());
    }

    #[test]
    fn test_initializer_classifier_sets() {
        assert!(SyntaxKind::Literal.is_expression_node());
        assert!(SyntaxKind::NameRef.is_expression_node());
        assert!(SyntaxKind::CallExpr.is_expression_node());
        assert!(!SyntaxKind::InitializerList.is_expression_node());
        assert!(!SyntaxKind::ArrayInitializer.is_expression_node());

        assert!(SyntaxKind::InitializerList.is_aggregate_initializer_node());
        assert!(SyntaxKind::ArrayInitializer.is_aggregate_initializer_node());
        assert!(SyntaxKind::Literal.is_initializer_expression_node());
        assert!(SyntaxKind::InitializerList.is_initializer_expression_node());
        assert!(SyntaxKind::ArrayInitializer.is_initializer_expression_node());

        assert!(!SyntaxKind::Pragma.is_expression_node());
        assert!(!SyntaxKind::Pragma.is_statement_node());
        assert!(!SyntaxKind::Pragma.is_initializer_expression_node());
    }

    #[test]
    fn test_pou_declaration_classifier_set() {
        let expected = &[
            SyntaxKind::Program,
            SyntaxKind::Function,
            SyntaxKind::FunctionBlock,
            SyntaxKind::Class,
            SyntaxKind::Method,
            SyntaxKind::Property,
            SyntaxKind::Interface,
        ];

        assert_eq!(POU_DECLARATION_KINDS, expected);
        for kind in expected {
            assert!(kind.is_pou_declaration(), "{kind:?} should be a POU");
        }
        for kind in [
            SyntaxKind::PropertyGet,
            SyntaxKind::PropertySet,
            SyntaxKind::ProgramConfig,
            SyntaxKind::Namespace,
            SyntaxKind::Configuration,
            SyntaxKind::Resource,
        ] {
            assert!(
                !kind.is_pou_declaration(),
                "{kind:?} is not a POU declaration"
            );
        }
    }
}
