use super::*;

/// Binary operators for type checking.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,
    // Comparison
    Eq,
    Neq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    // Logical
    And,
    Or,
    Xor,
    // Unknown
    Unknown,
}

impl BinaryOp {
    /// Determines the binary operator from syntax tokens in a node.
    pub(super) fn from_node(node: &SyntaxNode) -> Self {
        for element in node.children_with_tokens() {
            let token = match element.into_token() {
                Some(token) => token,
                None => continue,
            };
            match token.kind() {
                SyntaxKind::Plus => return Self::Add,
                SyntaxKind::Minus => return Self::Sub,
                SyntaxKind::Star => return Self::Mul,
                SyntaxKind::Slash => return Self::Div,
                SyntaxKind::KwMod => return Self::Mod,
                SyntaxKind::Power => return Self::Power,
                SyntaxKind::Eq => return Self::Eq,
                SyntaxKind::Neq => return Self::Neq,
                SyntaxKind::Lt => return Self::Lt,
                SyntaxKind::LtEq => return Self::LtEq,
                SyntaxKind::Gt => return Self::Gt,
                SyntaxKind::GtEq => return Self::GtEq,
                SyntaxKind::Ampersand => return Self::And,
                SyntaxKind::KwAnd => return Self::And,
                SyntaxKind::KwOr => return Self::Or,
                SyntaxKind::KwXor => return Self::Xor,
                _ => {}
            }
        }
        Self::Unknown
    }

    /// Returns true if this is a comparison operator.
    pub(super) fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Neq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq
        )
    }

    /// Returns true if this is a logical operator.
    pub(super) fn is_logical(self) -> bool {
        matches!(self, Self::And | Self::Or | Self::Xor)
    }

    /// Returns true if this is an arithmetic operator.
    pub(super) fn is_arithmetic(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod | Self::Power
        )
    }
}

/// Unary operators for type checking.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    Unknown,
}

impl UnaryOp {
    /// Determines the unary operator from syntax tokens in a node.
    pub(super) fn from_node(node: &SyntaxNode) -> Self {
        for element in node.children_with_tokens() {
            let token = match element.into_token() {
                Some(token) => token,
                None => continue,
            };
            match token.kind() {
                SyntaxKind::Minus => return Self::Neg,
                SyntaxKind::KwNot => return Self::Not,
                _ => {}
            }
        }
        Self::Unknown
    }
}
