use super::TokenKind;

impl TokenKind {
    /// Returns `true` if this token is a variable block keyword.
    pub fn is_var_keyword(self) -> bool {
        matches!(
            self,
            Self::KwVar
                | Self::KwVarInput
                | Self::KwVarOutput
                | Self::KwVarInOut
                | Self::KwVarTemp
                | Self::KwVarGlobal
                | Self::KwVarExternal
                | Self::KwVarAccess
                | Self::KwVarConfig
                | Self::KwVarStat
        )
    }

    /// Returns `true` if this token can start an expression.
    pub fn can_start_expr(self) -> bool {
        matches!(
            self,
            Self::Hash
                | Self::Ident
                | Self::KwEn
                | Self::KwEno
                | Self::KwGet
                | Self::KwSet
                | Self::KwStep
                | Self::IntLiteral
                | Self::RealLiteral
                | Self::StringLiteral
                | Self::WideStringLiteral
                | Self::TimeLiteral
                | Self::DateLiteral
                | Self::TimeOfDayLiteral
                | Self::DateAndTimeLiteral
                | Self::KwTrue
                | Self::KwFalse
                | Self::KwNull
                | Self::KwNot
                | Self::TypedLiteralPrefix
                | Self::LParen
                | Self::LBracket
                | Self::Minus
                | Self::Plus
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwRef
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::DirectAddress
        )
    }

    /// Returns `true` if this token can start a statement.
    pub fn can_start_statement(self) -> bool {
        matches!(
            self,
            Self::Hash
                | Self::Ident
                | Self::KwStep
                | Self::DirectAddress
                | Self::KwThis
                | Self::KwSuper
                | Self::KwNew
                | Self::KwNewDunder
                | Self::KwDeleteDunder
                | Self::KwRef
                | Self::KwAdr
                | Self::KwSizeOf
                | Self::KwIf
                | Self::KwCase
                | Self::KwFor
                | Self::KwWhile
                | Self::KwRepeat
                | Self::KwReturn
                | Self::KwExit
                | Self::KwContinue
                | Self::KwJmp
                | Self::Semicolon // Empty statement
        )
    }

    /// Returns `true` if this token is a comparison operator.
    pub fn is_comparison_op(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Neq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq
        )
    }

    /// Returns `true` if this token is an additive operator.
    pub fn is_additive_op(self) -> bool {
        matches!(self, Self::Plus | Self::Minus)
    }

    /// Returns `true` if this token is a multiplicative operator.
    pub fn is_multiplicative_op(self) -> bool {
        matches!(self, Self::Star | Self::Slash | Self::KwMod)
    }

    /// Returns the binding power for Pratt parsing (left, right).
    /// Returns None if not an infix operator.
    pub fn infix_binding_power(self) -> Option<(u8, u8)> {
        Some(match self {
            Self::KwOr => (1, 2),
            Self::KwXor => (3, 4),
            Self::KwAnd | Self::Ampersand => (5, 6),
            Self::Eq | Self::Neq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq => (7, 8),
            Self::Plus | Self::Minus => (9, 10),
            Self::Star | Self::Slash | Self::KwMod => (11, 12),
            Self::Power => (13, 14),
            _ => return None,
        })
    }

    /// Returns the binding power for prefix operators.
    pub fn prefix_binding_power(self) -> Option<u8> {
        Some(match self {
            Self::KwNot | Self::Plus | Self::Minus => 15,
            _ => return None,
        })
    }
}
