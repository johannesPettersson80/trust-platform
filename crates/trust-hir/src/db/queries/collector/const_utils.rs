use super::diagnostics::is_pou_kind;
use super::*;

pub(super) fn scope_chain_for_node(node: &SyntaxNode) -> Vec<Option<SmolStr>> {
    let mut namespace = Vec::new();
    let mut pou_stack = Vec::new();
    let mut ancestors: Vec<_> = node.ancestors().collect();
    ancestors.reverse();

    for ancestor in ancestors {
        if ancestor.kind() == SyntaxKind::Namespace {
            if let Some((parts, _)) = qualified_name_parts(&ancestor) {
                namespace.extend(parts.into_iter().map(|(name, _)| name));
            }
        } else if is_pou_kind(ancestor.kind()) {
            pou_stack.extend(pou_scope_parts(&ancestor));
        }
    }

    const_scope_chain_from_parts(&namespace, &pou_stack)
}

pub(super) fn const_scope_identity(
    namespace: &[SmolStr],
    pou_stack: &[SmolStr],
) -> Option<SmolStr> {
    let mut parts = Vec::with_capacity(namespace.len() + pou_stack.len());
    parts.extend(namespace.iter().cloned());
    parts.extend(pou_stack.iter().cloned());
    (!parts.is_empty()).then(|| qualified_name_string(&parts))
}

pub(super) fn const_scope_chain_from_parts(
    namespace: &[SmolStr],
    pou_stack: &[SmolStr],
) -> Vec<Option<SmolStr>> {
    let mut parts = Vec::with_capacity(namespace.len() + pou_stack.len());
    parts.extend(namespace.iter().cloned());
    parts.extend(pou_stack.iter().cloned());

    let mut scopes = Vec::new();
    for len in (1..=parts.len()).rev() {
        scopes.push(Some(qualified_name_string(&parts[..len])));
    }
    scopes.push(None);
    scopes
}

pub(super) fn pou_scope_parts(node: &SyntaxNode) -> Vec<SmolStr> {
    if let Some((parts, _)) = qualified_name_parts(node) {
        return parts.into_iter().map(|(name, _)| name).collect();
    }
    name_from_node(node)
        .map(|(name, _)| vec![name])
        .unwrap_or_default()
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
    AmbiguousName(SmolStr),
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
