use smol_str::SmolStr;

use crate::datetime::{
    days_from_civil, days_to_ticks, nanos_to_ticks, DateTimeCalcError, DivisionMode, NANOS_PER_DAY,
};
use crate::eval::expr::{Expr, LValue};
use crate::eval::ops::{BinaryOp, UnaryOp};
use crate::eval::{eval_expr, ArgValue, CallArg, EvalContext};
use crate::memory::VariableStorage;
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};
use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::util::{direct_expr_children, first_expr_child, is_expression_kind, node_text};
use super::super::{
    coerce_value_to_type, lower_type_ref, resolve_type_name, CompileError, LoweringContext,
};

pub(in crate::harness) fn lower_lvalue(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<LValue, CompileError> {
    match node.kind() {
        SyntaxKind::NameRef => Ok(LValue::Name(node_text(node).into())),
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let target = &exprs[0];
            let name = if target.kind() == SyntaxKind::NameRef {
                node_text(target)
            } else {
                return Err(CompileError::new("unsupported index target"));
            };
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            if indices.is_empty() {
                return Err(CompileError::new("missing index expression"));
            }
            Ok(LValue::Index {
                name: name.into(),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let target = &exprs[0];
            let name = if target.kind() == SyntaxKind::NameRef {
                node_text(target)
            } else {
                return Err(CompileError::new("unsupported field target"));
            };
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            Ok(LValue::Field {
                name: name.into(),
                field: node_text(&field).into(),
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(LValue::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        _ => Err(CompileError::new("unsupported assignment target")),
    }
}

pub(in crate::harness) fn lower_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    match node.kind() {
        SyntaxKind::Literal => lower_literal(node, ctx),
        SyntaxKind::NameRef => Ok(Expr::Name(node_text(node).into())),
        SyntaxKind::ThisExpr => Ok(Expr::This),
        SyntaxKind::SuperExpr => Ok(Expr::Super),
        SyntaxKind::UnaryExpr => {
            let op = unary_op_from_node(node)?;
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing unary operand"))?;
            Ok(Expr::Unary {
                op,
                expr: Box::new(lower_expr(&expr, ctx)?),
            })
        }
        SyntaxKind::BinaryExpr => {
            let op = binary_op_from_node(node)?;
            let exprs = direct_expr_children(node);
            if exprs.len() != 2 {
                return Err(CompileError::new("invalid binary expression"));
            }
            Ok(Expr::Binary {
                op,
                left: Box::new(lower_expr(&exprs[0], ctx)?),
                right: Box::new(lower_expr(&exprs[1], ctx)?),
            })
        }
        SyntaxKind::ParenExpr => {
            let expr = first_expr_child(node)
                .ok_or_else(|| CompileError::new("missing parenthesized expression"))?;
            lower_expr(&expr, ctx)
        }
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            Ok(Expr::Index {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            Ok(Expr::Field {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                field: node_text(&field).into(),
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(Expr::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        SyntaxKind::AddrExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing ADR operand"))?;
            let lvalue = lower_lvalue(&expr, ctx)?;
            Ok(Expr::Ref(lvalue))
        }
        SyntaxKind::CallExpr => lower_call_expr(node, ctx),
        SyntaxKind::SizeOfExpr => lower_sizeof_expr(node, ctx),
        SyntaxKind::ArrayInitializer | SyntaxKind::InitializerList => {
            Err(CompileError::new("initializer lists are not supported yet"))
        }
        _ => Err(CompileError::new("unsupported expression")),
    }
}

fn lower_sizeof_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    if let Some(type_ref) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
    {
        let type_id = lower_type_ref(&type_ref, ctx)?;
        return Ok(Expr::SizeOf(crate::eval::expr::SizeOfTarget::Type(type_id)));
    }
    if let Some(expr_node) = node
        .children()
        .find(|child| is_expression_kind(child.kind()))
    {
        let expr = lower_expr(&expr_node, ctx)?;
        return Ok(Expr::SizeOf(crate::eval::expr::SizeOfTarget::Expr(
            Box::new(expr),
        )));
    }
    Err(CompileError::new("SIZEOF expects a type or expression"))
}

fn lower_call_expr(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Expr, CompileError> {
    let target = first_expr_child(node).ok_or_else(|| CompileError::new("missing call target"))?;
    let target = lower_expr(&target, ctx)?;
    let args = lower_call_args(node, ctx)?;
    Ok(Expr::Call {
        target: Box::new(target),
        args,
    })
}

fn lower_call_args(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<CallArg>, CompileError> {
    let arg_list = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList);
    let Some(arg_list) = arg_list else {
        return Ok(Vec::new());
    };
    let mut args = Vec::new();
    for arg in arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
    {
        args.push(lower_call_arg(&arg, ctx)?);
    }
    Ok(args)
}

fn lower_call_arg(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<CallArg, CompileError> {
    let name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| node_text(&name).into());

    let mut has_arrow = false;
    for token in node
        .children_with_tokens()
        .filter_map(|child| child.into_token())
    {
        if token.kind() == SyntaxKind::Arrow {
            has_arrow = true;
        }
    }

    let expr_node =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing call argument"))?;
    let value = if has_arrow {
        ArgValue::Target(lower_lvalue(&expr_node, ctx)?)
    } else {
        match lower_lvalue(&expr_node, ctx) {
            Ok(target) => ArgValue::Target(target),
            Err(_) => ArgValue::Expr(lower_expr(&expr_node, ctx)?),
        }
    };

    Ok(CallArg { name, value })
}

fn lower_literal(node: &SyntaxNode, ctx: &LoweringContext<'_>) -> Result<Expr, CompileError> {
    let mut sign: i64 = 1;
    let mut int_literal: Option<i64> = None;
    let mut bool_literal: Option<bool> = None;
    let mut real_literal: Option<f64> = None;
    let mut string_literal: Option<(String, bool)> = None;
    let mut typed_prefix: Option<String> = None;
    let mut ident_literal: Option<String> = None;
    let mut value_literal: Option<Value> = None;
    let mut saw_sign = false;

    for element in node.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::TypedLiteralPrefix => {
                typed_prefix = Some(token.text().trim_end_matches('#').to_ascii_uppercase());
            }
            SyntaxKind::KwTrue => bool_literal = Some(true),
            SyntaxKind::KwFalse => bool_literal = Some(false),
            SyntaxKind::KwNull => value_literal = Some(Value::Null),
            SyntaxKind::Plus => {
                sign = 1;
                saw_sign = true;
            }
            SyntaxKind::Minus => {
                sign = -1;
                saw_sign = true;
            }
            SyntaxKind::IntLiteral => {
                int_literal = Some(parse_int_literal(token.text())?);
            }
            SyntaxKind::RealLiteral => {
                real_literal = Some(parse_real_literal(token.text())?);
            }
            SyntaxKind::StringLiteral => {
                let parsed = parse_string_literal(token.text(), false)?;
                string_literal = Some((parsed, false));
            }
            SyntaxKind::WideStringLiteral => {
                let parsed = parse_string_literal(token.text(), true)?;
                string_literal = Some((parsed, true));
            }
            SyntaxKind::TimeLiteral => {
                value_literal = Some(parse_time_literal(token.text())?);
            }
            SyntaxKind::DateLiteral => {
                value_literal = Some(parse_date_literal(token.text(), ctx.profile)?);
            }
            SyntaxKind::TimeOfDayLiteral => {
                value_literal = Some(parse_tod_literal(token.text(), ctx.profile)?);
            }
            SyntaxKind::DateAndTimeLiteral => {
                value_literal = Some(parse_dt_literal(token.text(), ctx.profile)?);
            }
            SyntaxKind::Ident => {
                ident_literal = Some(token.text().to_string());
            }
            _ => {}
        }
    }

    let has_typed_prefix = typed_prefix.is_some();
    let mut value = if let Some(value) = value_literal {
        value
    } else if let Some((string, wide)) = string_literal {
        if wide {
            Value::WString(string)
        } else {
            Value::String(SmolStr::new(string))
        }
    } else if let Some(value) = bool_literal {
        Value::Bool(value)
    } else if let Some(value) = real_literal {
        let signed = if saw_sign { value * sign as f64 } else { value };
        Value::LReal(signed)
    } else if let Some(value) = int_literal {
        let value = if saw_sign { value * sign } else { value };
        if has_typed_prefix {
            Value::LInt(value)
        } else {
            let value = i32::try_from(value)
                .map_err(|_| CompileError::new("integer literal out of range"))?;
            Value::DInt(value)
        }
    } else if ident_literal.is_some() {
        Value::Null
    } else {
        return Err(CompileError::new("invalid literal"));
    };

    if let Some(prefix) = typed_prefix {
        let type_id = if let Some(type_id) = TypeId::from_builtin_name(&prefix) {
            type_id
        } else {
            resolve_type_name(&prefix, ctx)?
        };
        if let Some(ident) = ident_literal {
            if let Some(Value::Enum(enum_value)) = enum_literal_value(&ident, type_id, ctx.registry)
            {
                return Ok(Expr::Literal(Value::Enum(enum_value)));
            }
        }
        if value == Value::Null {
            return Err(CompileError::new("invalid typed literal"));
        }
        value = coerce_value_to_type(value, type_id)?;
    }

    Ok(Expr::Literal(value))
}

fn parse_int_literal(text: &str) -> Result<i64, CompileError> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str
            .parse()
            .map_err(|_| CompileError::new("invalid integer literal base"))?;
        return i64::from_str_radix(digits, base)
            .map_err(|_| CompileError::new("invalid integer literal"));
    }
    cleaned
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid integer literal"))
}

fn parse_real_literal(text: &str) -> Result<f64, CompileError> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    cleaned
        .parse::<f64>()
        .map_err(|_| CompileError::new("invalid REAL literal"))
}

fn parse_string_literal(text: &str, is_wide: bool) -> Result<String, CompileError> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 {
        return Err(CompileError::new("invalid string literal"));
    }
    let quote = bytes[0];
    if bytes[bytes.len() - 1] != quote {
        return Err(CompileError::new("invalid string literal"));
    }
    let mut result = String::new();
    let mut i = 1usize;
    let end = bytes.len() - 1;
    while i < end {
        if bytes[i] != b'$' {
            result.push(bytes[i] as char);
            i += 1;
            continue;
        }
        if i + 1 >= end {
            return Err(CompileError::new("invalid escape sequence"));
        }
        let next = bytes[i + 1];
        match next {
            b'$' => {
                result.push('$');
                i += 2;
            }
            b'\'' => {
                result.push('\'');
                i += 2;
            }
            b'"' => {
                result.push('"');
                i += 2;
            }
            b'L' | b'l' | b'N' | b'n' => {
                result.push('\n');
                i += 2;
            }
            b'P' | b'p' => {
                result.push('\u{000C}');
                i += 2;
            }
            b'R' | b'r' => {
                result.push('\r');
                i += 2;
            }
            b'T' | b't' => {
                result.push('\t');
                i += 2;
            }
            _ => {
                let digits = if is_wide { 4 } else { 2 };
                if i + 1 + digits > end {
                    return Err(CompileError::new("invalid escape sequence"));
                }
                let hex = &text[i + 1..i + 1 + digits];
                let code = u32::from_str_radix(hex, 16)
                    .map_err(|_| CompileError::new("invalid hex escape"))?;
                let ch = std::char::from_u32(code)
                    .ok_or_else(|| CompileError::new("invalid character code"))?;
                result.push(ch);
                i += 1 + digits;
            }
        }
    }
    Ok(result)
}

fn parse_time_literal(text: &str) -> Result<Value, CompileError> {
    let is_long = is_long_time_literal(text);
    let nanos = parse_duration_nanos(text)?;
    let duration = Duration::from_nanos(nanos);
    Ok(if is_long {
        Value::LTime(duration)
    } else {
        Value::Time(duration)
    })
}

fn parse_date_literal(text: &str, profile: DateTimeProfile) -> Result<Value, CompileError> {
    let is_long = is_long_date_literal(text);
    let (year, month, day) = parse_date_parts(text)?;
    let days = days_from_civil_checked(year, month, day)?;
    if is_long {
        let nanos = days
            .checked_mul(NANOS_PER_DAY)
            .ok_or_else(|| CompileError::new("date out of range"))?;
        return Ok(Value::LDate(LDateValue::new(nanos)));
    }
    let ticks = days_to_ticks_checked(days, profile)?;
    Ok(Value::Date(DateValue::new(ticks)))
}

fn parse_tod_literal(text: &str, profile: DateTimeProfile) -> Result<Value, CompileError> {
    let is_long = is_long_tod_literal(text);
    let nanos = parse_time_of_day_nanos(text)?;
    if is_long {
        return Ok(Value::LTod(LTimeOfDayValue::new(nanos)));
    }
    let ticks = nanos_to_ticks_checked(nanos, profile)?;
    Ok(Value::Tod(TimeOfDayValue::new(ticks)))
}

fn parse_dt_literal(text: &str, profile: DateTimeProfile) -> Result<Value, CompileError> {
    let is_long = is_long_dt_literal(text);
    let (date_part, tod_part) = parse_dt_parts(text)?;
    let (year, month, day) = parse_date_parts(date_part)?;
    let days = days_from_civil_checked(year, month, day)?;
    let nanos_tod = parse_time_of_day_nanos(tod_part)?;
    if is_long {
        let date_nanos = days
            .checked_mul(NANOS_PER_DAY)
            .ok_or_else(|| CompileError::new("date out of range"))?;
        let nanos = date_nanos
            .checked_add(nanos_tod)
            .ok_or_else(|| CompileError::new("date/time out of range"))?;
        return Ok(Value::Ldt(LDateTimeValue::new(nanos)));
    }
    let date_ticks = days_to_ticks_checked(days, profile)?;
    let tod_ticks = nanos_to_ticks_checked(nanos_tod, profile)?;
    let ticks = date_ticks
        .checked_add(tod_ticks)
        .ok_or_else(|| CompileError::new("date/time out of range"))?;
    Ok(Value::Dt(DateTimeValue::new(ticks)))
}

fn days_from_civil_checked(year: i64, month: i64, day: i64) -> Result<i64, CompileError> {
    match days_from_civil(year, month, day) {
        Ok(days) => Ok(days),
        Err(DateTimeCalcError::InvalidDate) => Err(CompileError::new("invalid date")),
        Err(_) => Err(CompileError::new("invalid date")),
    }
}

fn days_to_ticks_checked(days: i64, profile: DateTimeProfile) -> Result<i64, CompileError> {
    match days_to_ticks(days, profile) {
        Ok(ticks) => Ok(ticks),
        Err(DateTimeCalcError::InvalidResolution) => {
            Err(CompileError::new("invalid time resolution"))
        }
        Err(DateTimeCalcError::Overflow) => Err(CompileError::new("date out of range")),
        Err(DateTimeCalcError::InvalidDate) => Err(CompileError::new("invalid date")),
    }
}

fn nanos_to_ticks_checked(nanos: i64, profile: DateTimeProfile) -> Result<i64, CompileError> {
    match nanos_to_ticks(nanos, profile, DivisionMode::Trunc) {
        Ok(ticks) => Ok(ticks),
        Err(DateTimeCalcError::InvalidResolution) => {
            Err(CompileError::new("invalid time resolution"))
        }
        Err(_) => Err(CompileError::new("invalid time resolution")),
    }
}

fn parse_duration_nanos(text: &str) -> Result<i64, CompileError> {
    let upper = text.to_ascii_uppercase();
    let (_, raw) = upper
        .split_once('#')
        .ok_or_else(|| CompileError::new("invalid TIME literal"))?;
    let mut rest = raw.trim();
    let mut sign: f64 = 1.0;
    if let Some(stripped) = rest.strip_prefix('-') {
        sign = -1.0;
        rest = stripped;
    } else if let Some(stripped) = rest.strip_prefix('+') {
        rest = stripped;
    }

    let bytes = rest.as_bytes();
    let mut idx = 0usize;
    let mut total: f64 = 0.0;
    while idx < bytes.len() {
        let start = idx;
        while idx < bytes.len()
            && (bytes[idx].is_ascii_digit() || bytes[idx] == b'_' || bytes[idx] == b'.')
        {
            idx += 1;
        }
        if start == idx {
            return Err(CompileError::new("invalid TIME literal"));
        }
        let num_str: String = rest[start..idx].chars().filter(|c| *c != '_').collect();
        let value = num_str
            .parse::<f64>()
            .map_err(|_| CompileError::new("invalid TIME literal"))?;
        let unit_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_alphabetic() {
            idx += 1;
        }
        let unit = &rest[unit_start..idx];
        let nanos_per = match unit {
            "D" => 86_400_000_000_000.0,
            "H" => 3_600_000_000_000.0,
            "M" => 60_000_000_000.0,
            "S" => 1_000_000_000.0,
            "MS" => 1_000_000.0,
            "US" => 1_000.0,
            "NS" => 1.0,
            _ => return Err(CompileError::new("invalid TIME literal unit")),
        };
        total += value * nanos_per;
        while idx < bytes.len() && bytes[idx] == b'_' {
            idx += 1;
        }
    }
    let nanos = (total * sign).round();
    let nanos =
        i64::try_from(nanos as i128).map_err(|_| CompileError::new("TIME literal out of range"))?;
    Ok(nanos)
}

fn parse_date_parts(text: &str) -> Result<(i64, i64, i64), CompileError> {
    let rest = match text.split_once('#') {
        Some((_, rest)) => rest,
        None => text,
    };
    let mut parts = rest.split('-');
    let year = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid DATE literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid DATE literal"))?;
    let month = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid DATE literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid DATE literal"))?;
    let day = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid DATE literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid DATE literal"))?;
    Ok((year, month, day))
}

fn parse_time_of_day_nanos(text: &str) -> Result<i64, CompileError> {
    let rest = match text.split_once('#') {
        Some((_, rest)) => rest,
        None => text,
    };
    let mut parts = rest.split(':');
    let hours = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid TOD literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid TOD literal"))?;
    let minutes = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid TOD literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid TOD literal"))?;
    let seconds_part = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid TOD literal"))?;
    let (seconds, nanos) = parse_seconds_fraction(seconds_part)?;
    let total = hours
        .checked_mul(3_600)
        .and_then(|v| v.checked_add(minutes.checked_mul(60)?))
        .and_then(|v| v.checked_add(seconds))
        .ok_or_else(|| CompileError::new("invalid TOD literal"))?;
    let total_nanos = total
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(nanos))
        .ok_or_else(|| CompileError::new("invalid TOD literal"))?;
    Ok(total_nanos)
}

fn parse_dt_parts(text: &str) -> Result<(&str, &str), CompileError> {
    let (_, rest) = text
        .split_once('#')
        .ok_or_else(|| CompileError::new("invalid DT literal"))?;
    let (date_part, time_part) = rest
        .rsplit_once('-')
        .ok_or_else(|| CompileError::new("invalid DT literal"))?;
    Ok((date_part, time_part))
}

fn parse_seconds_fraction(text: &str) -> Result<(i64, i64), CompileError> {
    let mut parts = text.split('.');
    let secs = parts
        .next()
        .ok_or_else(|| CompileError::new("invalid time literal"))?
        .parse::<i64>()
        .map_err(|_| CompileError::new("invalid time literal"))?;
    let nanos = if let Some(frac) = parts.next() {
        let digits: String = frac.chars().filter(|c| *c != '_').collect();
        if digits.is_empty() {
            0
        } else {
            let mut padded = digits;
            if padded.len() > 9 {
                padded.truncate(9);
            }
            while padded.len() < 9 {
                padded.push('0');
            }
            padded
                .parse::<i64>()
                .map_err(|_| CompileError::new("invalid time fraction"))?
        }
    } else {
        0
    };
    Ok((secs, nanos))
}

fn is_long_time_literal(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.starts_with("LT#") || upper.starts_with("LTIME#")
}

fn is_long_date_literal(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.starts_with("LDATE#") || upper.starts_with("LD#")
}

fn is_long_tod_literal(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.starts_with("LTOD#") || upper.starts_with("LTIME_OF_DAY#")
}

fn is_long_dt_literal(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.starts_with("LDT#") || upper.starts_with("LDATE_AND_TIME#")
}

fn enum_literal_value(name: &str, type_id: TypeId, registry: &TypeRegistry) -> Option<Value> {
    let ty = registry.get(type_id)?;
    if let trust_hir::Type::Enum {
        name: enum_name,
        values,
        ..
    } = ty
    {
        let (variant_name, numeric_value) = values
            .iter()
            .find(|(variant, _)| variant.eq_ignore_ascii_case(name))?;
        return Some(Value::Enum(EnumValue {
            type_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            numeric_value: *numeric_value,
        }));
    }
    None
}

fn binary_op_from_node(node: &SyntaxNode) -> Result<BinaryOp, CompileError> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Ok(BinaryOp::Add),
            SyntaxKind::Minus => return Ok(BinaryOp::Sub),
            SyntaxKind::Star => return Ok(BinaryOp::Mul),
            SyntaxKind::Slash => return Ok(BinaryOp::Div),
            SyntaxKind::Power => return Ok(BinaryOp::Pow),
            SyntaxKind::KwMod => return Ok(BinaryOp::Mod),
            SyntaxKind::KwAnd | SyntaxKind::Ampersand => return Ok(BinaryOp::And),
            SyntaxKind::KwOr => return Ok(BinaryOp::Or),
            SyntaxKind::KwXor => return Ok(BinaryOp::Xor),
            SyntaxKind::Eq => return Ok(BinaryOp::Eq),
            SyntaxKind::Neq => return Ok(BinaryOp::Ne),
            SyntaxKind::Lt => return Ok(BinaryOp::Lt),
            SyntaxKind::LtEq => return Ok(BinaryOp::Le),
            SyntaxKind::Gt => return Ok(BinaryOp::Gt),
            SyntaxKind::GtEq => return Ok(BinaryOp::Ge),
            _ => {}
        }
    }
    Err(CompileError::new("unsupported binary operator"))
}

fn unary_op_from_node(node: &SyntaxNode) -> Result<UnaryOp, CompileError> {
    for element in node.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Plus => return Ok(UnaryOp::Pos),
            SyntaxKind::Minus => return Ok(UnaryOp::Neg),
            SyntaxKind::KwNot => return Ok(UnaryOp::Not),
            _ => {}
        }
    }
    Err(CompileError::new("unsupported unary operator"))
}

pub(in crate::harness) fn parse_subrange(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(i64, i64), CompileError> {
    let exprs = direct_expr_children(node);
    if exprs.is_empty() {
        return Err(CompileError::new("missing subrange bounds"));
    }
    if exprs.len() == 1 {
        if is_wildcard_expr(&exprs[0]) {
            return Ok((0, i64::MAX));
        }
        let value = const_int_from_node(&exprs[0], ctx)?;
        return Ok((value, value));
    }
    if exprs.len() == 2 {
        if is_wildcard_expr(&exprs[0]) || is_wildcard_expr(&exprs[1]) {
            return Ok((0, i64::MAX));
        }
        let lower = const_int_from_node(&exprs[0], ctx)?;
        let upper = const_int_from_node(&exprs[1], ctx)?;
        return Ok((lower, upper));
    }
    Err(CompileError::new("invalid subrange bounds"))
}

fn is_wildcard_expr(node: &SyntaxNode) -> bool {
    node.text().to_string().trim() == "*"
}

pub(in crate::harness) fn const_int_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<i64, CompileError> {
    let expr = lower_expr(node, ctx)?;
    let mut storage = VariableStorage::default();
    let mut eval_ctx = EvalContext {
        storage: &mut storage,
        registry: ctx.registry,
        profile: ctx.profile,
        now: Duration::ZERO,
        debug: None,
        call_depth: 0,
        functions: None,
        stdlib: None,
        function_blocks: None,
        classes: None,
        using: Some(&ctx.using),
        access: None,
        current_instance: None,
        return_name: None,
        loop_depth: 0,
        pause_requested: false,
        execution_deadline: None,
    };
    let value =
        eval_expr(&mut eval_ctx, &expr).map_err(|err| CompileError::new(err.to_string()))?;
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Byte(v) => Ok(v as i64),
        Value::Word(v) => Ok(v as i64),
        Value::DWord(v) => Ok(v as i64),
        Value::LWord(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Enum(enum_value) => Ok(enum_value.numeric_value),
        _ => Err(CompileError::new("expected integer constant")),
    }
}

pub(in crate::harness) fn const_duration_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Duration, CompileError> {
    let expr = lower_expr(node, ctx)?;
    let mut storage = VariableStorage::default();
    let mut eval_ctx = EvalContext {
        storage: &mut storage,
        registry: ctx.registry,
        profile: ctx.profile,
        now: Duration::ZERO,
        debug: None,
        call_depth: 0,
        functions: None,
        stdlib: None,
        function_blocks: None,
        classes: None,
        using: Some(&ctx.using),
        access: None,
        current_instance: None,
        return_name: None,
        loop_depth: 0,
        pause_requested: false,
        execution_deadline: None,
    };
    let value =
        eval_expr(&mut eval_ctx, &expr).map_err(|err| CompileError::new(err.to_string()))?;
    match value {
        Value::Time(duration) | Value::LTime(duration) => Ok(duration),
        _ => Err(CompileError::new("expected TIME/INTERVAL constant")),
    }
}
