use super::*;

pub(super) fn parse_int_literal_from_node(node: &SyntaxNode) -> Option<i64> {
    for token in node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        match token.kind() {
            SyntaxKind::KwTrue => return Some(1),
            SyntaxKind::KwFalse => return Some(0),
            _ => {}
        }
    }

    int_literal_info(node).map(|info| info.value)
}

#[derive(Clone, Copy)]
pub(super) struct IntLiteralInfo {
    pub(super) value: i64,
    pub(super) is_based: bool,
}

pub(super) fn int_literal_info(node: &SyntaxNode) -> Option<IntLiteralInfo> {
    let mut info = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|token| token.kind() == SyntaxKind::IntLiteral)
        .and_then(|token| parse_int_literal(token.text()))?;
    if node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| token.kind() == SyntaxKind::Minus)
    {
        info.value = info.value.checked_neg()?;
    }
    Some(info)
}

pub(super) fn smallest_int_type_for_literal(value: i64, prefer_unsigned: bool) -> TypeId {
    if value < 0 {
        if value >= i8::MIN as i64 {
            return TypeId::SINT;
        }
        if value >= i16::MIN as i64 {
            return TypeId::INT;
        }
        if value >= i32::MIN as i64 {
            return TypeId::DINT;
        }
        return TypeId::LINT;
    }

    let unsigned_value = value as u64;
    if prefer_unsigned {
        if unsigned_value <= u8::MAX as u64 {
            return TypeId::USINT;
        }
        if unsigned_value <= u16::MAX as u64 {
            return TypeId::UINT;
        }
        if unsigned_value <= u32::MAX as u64 {
            return TypeId::UDINT;
        }
        return TypeId::ULINT;
    }

    if value <= i8::MAX as i64 {
        return TypeId::SINT;
    }
    if value <= i16::MAX as i64 {
        return TypeId::INT;
    }
    if value <= i32::MAX as i64 {
        return TypeId::DINT;
    }
    TypeId::LINT
}

fn parse_int_literal(text: &str) -> Option<IntLiteralInfo> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str.parse().ok()?;
        let value = i64::from_str_radix(digits, base).ok()?;
        return Some(IntLiteralInfo {
            value,
            is_based: true,
        });
    }
    let value = cleaned.parse::<i64>().ok()?;
    Some(IntLiteralInfo {
        value,
        is_based: false,
    })
}

#[derive(Clone, Copy)]
pub(super) enum IntUnaryOp {
    Plus,
    Minus,
}

pub(super) fn int_unary_op_from_node(node: &SyntaxNode) -> Option<IntUnaryOp> {
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

pub(super) fn is_untyped_int_literal_expr(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::Literal => {
            let mut saw_int = false;
            for element in node.descendants_with_tokens() {
                let token = match element.into_token() {
                    Some(token) => token,
                    None => continue,
                };
                match token.kind() {
                    SyntaxKind::TypedLiteralPrefix => return false,
                    SyntaxKind::IntLiteral => saw_int = true,
                    SyntaxKind::RealLiteral
                    | SyntaxKind::StringLiteral
                    | SyntaxKind::WideStringLiteral
                    | SyntaxKind::KwTrue
                    | SyntaxKind::KwFalse
                    | SyntaxKind::KwNull
                    | SyntaxKind::TimeLiteral
                    | SyntaxKind::DateLiteral
                    | SyntaxKind::TimeOfDayLiteral
                    | SyntaxKind::DateAndTimeLiteral => return false,
                    _ => {}
                }
            }
            saw_int
        }
        SyntaxKind::ParenExpr => node
            .children()
            .next()
            .is_some_and(|child| is_untyped_int_literal_expr(&child)),
        SyntaxKind::UnaryExpr => {
            if int_unary_op_from_node(node).is_none() {
                return false;
            }
            node.children()
                .next()
                .is_some_and(|child| is_untyped_int_literal_expr(&child))
        }
        SyntaxKind::BinaryExpr => {
            if int_binary_op_from_node(node).is_none() {
                return false;
            }
            let children: Vec<_> = node.children().collect();
            if children.len() < 2 {
                return false;
            }
            is_untyped_int_literal_expr(&children[0])
                && is_untyped_int_literal_expr(&children[children.len() - 1])
        }
        _ => false,
    }
}

fn real_binary_op_from_node(node: &SyntaxNode) -> bool {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Power => return true,
            _ => {}
        }
    }
    false
}

pub(super) fn is_untyped_real_literal_expr(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::Literal => {
            let mut saw_real = false;
            for element in node.descendants_with_tokens() {
                let token = match element.into_token() {
                    Some(token) => token,
                    None => continue,
                };
                match token.kind() {
                    SyntaxKind::TypedLiteralPrefix => return false,
                    SyntaxKind::RealLiteral => saw_real = true,
                    SyntaxKind::IntLiteral
                    | SyntaxKind::StringLiteral
                    | SyntaxKind::WideStringLiteral
                    | SyntaxKind::KwTrue
                    | SyntaxKind::KwFalse
                    | SyntaxKind::KwNull
                    | SyntaxKind::TimeLiteral
                    | SyntaxKind::DateLiteral
                    | SyntaxKind::TimeOfDayLiteral
                    | SyntaxKind::DateAndTimeLiteral => return false,
                    _ => {}
                }
            }
            saw_real
        }
        SyntaxKind::ParenExpr => node
            .children()
            .next()
            .is_some_and(|child| is_untyped_real_literal_expr(&child)),
        SyntaxKind::UnaryExpr => {
            if int_unary_op_from_node(node).is_none() {
                return false;
            }
            node.children()
                .next()
                .is_some_and(|child| is_untyped_real_literal_expr(&child))
        }
        SyntaxKind::BinaryExpr => {
            if !real_binary_op_from_node(node) {
                return false;
            }
            let children: Vec<_> = node.children().collect();
            if children.len() < 2 {
                return false;
            }
            is_untyped_real_literal_expr(&children[0])
                && is_untyped_real_literal_expr(&children[children.len() - 1])
        }
        _ => false,
    }
}

pub(super) fn is_zero_numeric_literal_expr(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::Literal => literal_is_zero(node),
        SyntaxKind::ParenExpr => node
            .children()
            .next()
            .is_some_and(|child| is_zero_numeric_literal_expr(&child)),
        SyntaxKind::UnaryExpr => {
            if int_unary_op_from_node(node).is_none() {
                return false;
            }
            node.children()
                .next()
                .is_some_and(|child| is_zero_numeric_literal_expr(&child))
        }
        _ => false,
    }
}

fn literal_is_zero(node: &SyntaxNode) -> bool {
    if int_literal_info(node).is_some_and(|info| info.value == 0) {
        return true;
    }

    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|token| token.kind() == SyntaxKind::RealLiteral)
        .and_then(|token| {
            let cleaned: String = token.text().chars().filter(|c| *c != '_').collect();
            cleaned.parse::<f64>().ok()
        })
        .is_some_and(|value| value == 0.0)
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

pub(super) fn int_binary_op_from_node(node: &SyntaxNode) -> Option<IntBinaryOp> {
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

#[derive(Clone, Copy)]
pub(crate) struct StringLiteralInfo {
    pub(crate) is_wide: bool,
    pub(crate) len: u32,
}

pub(crate) fn string_literal_info(node: &SyntaxNode) -> Option<StringLiteralInfo> {
    if node.kind() != SyntaxKind::Literal {
        return None;
    }
    for element in node.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        let (is_wide, text) = match token.kind() {
            SyntaxKind::StringLiteral => (false, token.text()),
            SyntaxKind::WideStringLiteral => (true, token.text()),
            _ => continue,
        };
        let len = string_literal_len(text, is_wide)?;
        return Some(StringLiteralInfo { is_wide, len });
    }
    None
}

fn string_literal_len(text: &str, is_wide: bool) -> Option<u32> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let quote = bytes[0];
    if bytes[bytes.len() - 1] != quote {
        return None;
    }
    let mut i = 1usize;
    let mut count: u32 = 0;
    let end = bytes.len() - 1;
    while i < end {
        if bytes[i] == b'$' {
            if i + 1 >= end {
                return None;
            }
            let next = bytes[i + 1];
            if matches!(
                next,
                b'$' | b'\''
                    | b'"'
                    | b'L'
                    | b'l'
                    | b'N'
                    | b'n'
                    | b'P'
                    | b'p'
                    | b'R'
                    | b'r'
                    | b'T'
                    | b't'
            ) {
                count += 1;
                i += 2;
                continue;
            }
            let digits = if is_wide { 4 } else { 2 };
            if i + 1 + digits <= end
                && bytes[i + 1..i + 1 + digits]
                    .iter()
                    .all(|b| b.is_ascii_hexdigit())
            {
                count += 1;
                i += 1 + digits;
                continue;
            }
        }
        let scalar = text[i..end].chars().next()?;
        count += 1;
        i += scalar.len_utf8();
    }
    Some(count)
}

fn has_literal_prefix(text: &str, prefixes: &[&str]) -> bool {
    let upper = text.to_ascii_uppercase();
    prefixes.iter().any(|prefix| upper.starts_with(prefix))
}

pub(super) fn is_long_time_literal(text: &str) -> bool {
    has_literal_prefix(text, &["LT#", "LTIME#"])
}

pub(super) fn is_long_date_literal(text: &str) -> bool {
    has_literal_prefix(text, &["LDATE#", "LD#"])
}

pub(super) fn is_long_tod_literal(text: &str) -> bool {
    has_literal_prefix(text, &["LTOD#", "LTIME_OF_DAY#"])
}

pub(super) fn is_long_dt_literal(text: &str) -> bool {
    has_literal_prefix(text, &["LDT#", "LDATE_AND_TIME#"])
}

/// The exact value of an integer literal. The magnitude is read as u64, so the whole unsigned
/// 64-bit range stays exact. Covers an IntLiteral with or without a typed prefix, a sign inside a
/// typed literal, and a unary sign or parentheses around the literal. None otherwise.
pub(crate) fn exact_int_literal_value(node: &SyntaxNode) -> Option<i128> {
    match node.kind() {
        SyntaxKind::Literal => exact_literal_token_value(node),
        SyntaxKind::ParenExpr => exact_int_literal_value(&node.children().next()?),
        SyntaxKind::UnaryExpr => {
            let op = int_unary_op_from_node(node)?;
            let value = exact_int_literal_value(&node.children().next()?)?;
            Some(match op {
                IntUnaryOp::Plus => value,
                IntUnaryOp::Minus => -value,
            })
        }
        _ => None,
    }
}

fn exact_literal_token_value(node: &SyntaxNode) -> Option<i128> {
    let mut digits = None;
    let mut negative = false;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::IntLiteral => digits = Some(token.text().to_string()),
            SyntaxKind::Minus => negative = true,
            _ => {}
        }
    }
    let magnitude = i128::from(parse_exact_magnitude(&digits?)?);
    Some(if negative { -magnitude } else { magnitude })
}

/// The unsigned magnitude of an IntLiteral's text, read as u64. Based literals use bases 2 through
/// 36, and `_` separators are dropped.
fn parse_exact_magnitude(text: &str) -> Option<u64> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str.parse().ok()?;
        if !(2..=36).contains(&base) {
            return None;
        }
        return u64::from_str_radix(digits, base).ok();
    }
    cleaned.parse::<u64>().ok()
}

/// Whether `value` lies in the integer type `ty`.
pub(crate) fn integer_type_range_fits(ty: &Type, value: i128) -> bool {
    match ty {
        Type::SInt => i8::try_from(value).is_ok(),
        Type::Int => i16::try_from(value).is_ok(),
        Type::DInt => i32::try_from(value).is_ok(),
        Type::LInt => i64::try_from(value).is_ok(),
        Type::USInt => u8::try_from(value).is_ok(),
        Type::UInt => u16::try_from(value).is_ok(),
        Type::UDInt => u32::try_from(value).is_ok(),
        Type::ULInt => u64::try_from(value).is_ok(),
        _ => false,
    }
}
