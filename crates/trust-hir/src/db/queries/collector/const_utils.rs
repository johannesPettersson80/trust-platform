use super::diagnostics::is_pou_kind;
use super::*;

pub(super) fn scope_chain_for_node(node: &SyntaxNode) -> Vec<Option<SmolStr>> {
    let mut scopes = Vec::new();
    for ancestor in node.ancestors() {
        if !is_pou_kind(ancestor.kind()) {
            continue;
        }
        if let Some((name, _)) = name_from_node(&ancestor) {
            scopes.push(Some(name));
        }
    }
    scopes.push(None);
    scopes
}

fn normalize_const_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}

pub(super) fn const_key(scope: &Option<SmolStr>, name: &str) -> (Option<SmolStr>, SmolStr) {
    let scope_key = scope
        .as_ref()
        .map(|scope_name| normalize_const_name(scope_name.as_str()));
    (scope_key, normalize_const_name(name))
}

pub(super) fn parse_int_literal_from_node(node: &SyntaxNode) -> Option<i64> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|token| token.kind() == SyntaxKind::IntLiteral)
        .and_then(|token| parse_int_literal(token.text()))
}

pub(super) fn parse_int_literal(text: &str) -> Option<i64> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str.parse().ok()?;
        i64::from_str_radix(digits, base).ok()
    } else {
        cleaned.parse::<i64>().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ConstEvalError {
    NotConstant,
    UndefinedName(SmolStr),
    DivideByZero,
    IntegerOverflow,
    NegativeExponent,
    CyclicDependency(SmolStr),
}

#[derive(Clone, Copy)]
pub(super) enum IntUnaryOp {
    Plus,
    Minus,
}

pub(super) fn unary_op_from_node(node: &SyntaxNode) -> Option<IntUnaryOp> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Some(IntUnaryOp::Plus),
            SyntaxKind::Minus => return Some(IntUnaryOp::Minus),
            _ => {}
        }
    }
    None
}

#[derive(Clone, Copy)]
pub(super) enum IntBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,
}

pub(super) fn binary_op_from_node(node: &SyntaxNode) -> Option<IntBinaryOp> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Some(IntBinaryOp::Add),
            SyntaxKind::Minus => return Some(IntBinaryOp::Sub),
            SyntaxKind::Star => return Some(IntBinaryOp::Mul),
            SyntaxKind::Slash => return Some(IntBinaryOp::Div),
            SyntaxKind::KwMod => return Some(IntBinaryOp::Mod),
            SyntaxKind::Power => return Some(IntBinaryOp::Power),
            _ => {}
        }
    }
    None
}
