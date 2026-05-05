fn lower_literal_with_context(
    node: &SyntaxNode,
    ctx: &LoweringContext<'_>,
    expected_type: Option<TypeId>,
) -> Result<Expr, CompileError> {
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
    } else if let Some(type_id) = expected_type {
        if value != Value::Null {
            value = coerce_value_to_type(value, type_id)?;
        }
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
            let ch = text[i..end]
                .chars()
                .next()
                .ok_or_else(|| CompileError::new("invalid string literal"))?;
            result.push(ch);
            i += ch.len_utf8();
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
                let hex = std::str::from_utf8(&bytes[i + 1..i + 1 + digits])
                    .map_err(|_| CompileError::new("invalid escape sequence"))?;
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

pub(in crate::harness) fn enum_literal_value(
    name: &str,
    type_id: TypeId,
    registry: &TypeRegistry,
) -> Option<Value> {
    EnumValue::new(registry, type_id, name)
        .ok()
        .map(|value| Value::Enum(Box::new(value)))
}

fn first_ident_token(node: &SyntaxNode) -> Option<trust_syntax::syntax::SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident | SyntaxKind::KwEn | SyntaxKind::KwEno
            )
        })
}

fn name_from_node(node: &SyntaxNode) -> Option<(SmolStr, text_size::TextRange)> {
    let token = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .and_then(|name_node| first_ident_token(&name_node))
        .or_else(|| first_ident_token(node))?;
    Some((SmolStr::new(token.text()), token.text_range()))
}

fn find_symbol_by_name_range(
    symbols: &SymbolTable,
    name: &str,
    range: text_size::TextRange,
) -> Option<SymbolId> {
    symbols
        .iter()
        .find(|symbol| symbol.range == range && symbol.name.eq_ignore_ascii_case(name))
        .map(|symbol| symbol.id)
}

fn find_scope_for_symbol(symbols: &SymbolTable, symbol_id: SymbolId) -> Option<ScopeId> {
    for index in 0..symbols.scope_count() {
        let scope_id = ScopeId(index as u32);
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if scope.owner == Some(symbol_id) {
            return Some(scope_id);
        }
    }
    None
}

#[derive(Clone, Copy)]
struct ExpressionScopeContext {
    scope_id: ScopeId,
    current_pou_symbol: Option<SymbolId>,
    this_type: Option<TypeId>,
}

fn receiver_type_for_pou(
    symbols: &SymbolTable,
    pou_symbol_id: Option<SymbolId>,
    pou_node: &SyntaxNode,
) -> Option<TypeId> {
    match pou_node.kind() {
        SyntaxKind::FunctionBlock | SyntaxKind::Class | SyntaxKind::Interface => pou_symbol_id
            .and_then(|id| symbols.get(id))
            .map(|symbol| symbol.type_id),
        SyntaxKind::Method | SyntaxKind::Property => pou_symbol_id
            .and_then(|id| symbols.get(id))
            .and_then(|symbol| symbol.parent)
            .and_then(|parent| symbols.get(parent))
            .map(|symbol| symbol.type_id),
        _ => None,
    }
}

fn expression_scope_context(symbols: &SymbolTable, node: &SyntaxNode) -> ExpressionScopeContext {
    let Some(pou_node) = node
        .ancestors()
        .find(|ancestor| ancestor.kind().is_pou_declaration())
    else {
        return ExpressionScopeContext {
            scope_id: ScopeId::GLOBAL,
            current_pou_symbol: None,
            this_type: None,
        };
    };

    let pou_symbol_id = name_from_node(&pou_node)
        .and_then(|(name, range)| find_symbol_by_name_range(symbols, name.as_str(), range));
    let scope_id = pou_symbol_id
        .and_then(|id| find_scope_for_symbol(symbols, id))
        .unwrap_or(ScopeId::GLOBAL);
    let this_type = receiver_type_for_pou(symbols, pou_symbol_id, &pou_node);

    ExpressionScopeContext {
        scope_id,
        current_pou_symbol: pou_symbol_id,
        this_type,
    }
}

fn class_owner_from_type(symbols: &SymbolTable, type_id: TypeId, allow_interface: bool) -> Option<SymbolId> {
    let base_type = symbols.resolve_alias_type(type_id);
    let name = match symbols.type_by_id(base_type)? {
        Type::FunctionBlock { name } | Type::Class { name } => name,
        Type::Interface { name } if allow_interface => name,
        _ => return None,
    };
    symbols.resolve_global_or_qualified_name(name.as_str())
}

fn current_class_owner(
    symbols: &SymbolTable,
    current_pou_symbol: Option<SymbolId>,
    this_type: Option<TypeId>,
) -> Option<SymbolId> {
    if let Some(pou_id) = current_pou_symbol {
        if let Some(symbol) = symbols.get(pou_id) {
            match symbol.kind {
                SymbolKind::Class | SymbolKind::FunctionBlock => return Some(pou_id),
                SymbolKind::Method { .. } | SymbolKind::Property { .. } => return symbol.parent,
                _ => {}
            }
        }
    }

    let this_type = this_type?;
    class_owner_from_type(symbols, this_type, false)
}

fn resolve_name_symbol_in_scope(
    symbols: &SymbolTable,
    scope_id: ScopeId,
    current_pou_symbol: Option<SymbolId>,
    this_type: Option<TypeId>,
    name: &str,
) -> Option<SymbolId> {
    let mut scope_id = Some(scope_id);
    let mut after_class_scope = None;
    let mut class_scope_id = None;

    while let Some(sid) = scope_id {
        let scope = match symbols.get_scope(sid) {
            Some(scope) => scope,
            None => break,
        };
        if let Some(symbol_id) = scope.lookup_local(name) {
            return Some(symbol_id);
        }

        if matches!(scope.kind, ScopeKind::Class | ScopeKind::FunctionBlock) {
            after_class_scope = scope.parent;
            class_scope_id = Some(sid);
            break;
        }

        match symbols.resolve_using_in_scope(scope, name) {
            UsingResolution::Single(symbol_id) => return Some(symbol_id),
            UsingResolution::Ambiguous => return None,
            UsingResolution::None => {}
        }

        scope_id = scope.parent;
    }

    if let Some(owner_id) = current_class_owner(symbols, current_pou_symbol, this_type) {
        if let Some(member_id) = symbols.resolve_member_symbol_in_hierarchy(owner_id, name) {
            return Some(member_id);
        }
    }

    if let Some(class_sid) = class_scope_id {
        let scope = symbols.get_scope(class_sid)?;
        match symbols.resolve_using_in_scope(scope, name) {
            UsingResolution::Single(symbol_id) => return Some(symbol_id),
            UsingResolution::Ambiguous => return None,
            UsingResolution::None => {}
        }
    }

    let mut scope_id = after_class_scope;
    while let Some(sid) = scope_id {
        let scope = match symbols.get_scope(sid) {
            Some(scope) => scope,
            None => break,
        };
        if let Some(symbol_id) = scope.lookup_local(name) {
            return Some(symbol_id);
        }
        match symbols.resolve_using_in_scope(scope, name) {
            UsingResolution::Single(symbol_id) => return Some(symbol_id),
            UsingResolution::Ambiguous => return None,
            UsingResolution::None => {}
        }
        scope_id = scope.parent;
    }

    None
}

fn semantic_enum_type_name(symbols: &SymbolTable, type_id: TypeId) -> Option<SmolStr> {
    let resolved = symbols.resolve_alias_type(type_id);
    match symbols.type_by_id(resolved)? {
        Type::Enum { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn runtime_enum_type_name(registry: &TypeRegistry, type_id: TypeId) -> Option<SmolStr> {
    let mut current = type_id;
    let mut guard = 0;
    while guard < 16 {
        match registry.get(current)? {
            Type::Alias { target, .. } => {
                current = *target;
                guard += 1;
            }
            Type::Enum { name, .. } => return Some(name.clone()),
            _ => return None,
        }
    }
    None
}

/// Rewrite an initializer expression so that an unqualified `NameRef`
/// resolved by HIR as an enum value of `target_type_id` becomes an
/// `Expr::Literal` with the corresponding `Value::Enum`.
pub(in crate::harness) fn resolve_initializer_enum_variant(
    node: &SyntaxNode,
    expr: Expr,
    target_type_id: TypeId,
    ctx: &LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    if node.kind() != SyntaxKind::NameRef {
        return Ok(expr);
    }
    let Expr::Name(name) = &expr else {
        return Ok(expr);
    };

    let (semantic_db, semantic_file_id) = match (ctx.semantic_db, ctx.semantic_file_id) {
        (Some(db), Some(file_id)) => (db, file_id),
        _ => return Ok(expr),
    };
    let analysis = semantic_db.analyze(semantic_file_id);
    let symbols = analysis.symbols.as_ref();
    let scope_context = expression_scope_context(symbols, node);
    let Some(symbol_id) = resolve_name_symbol_in_scope(
        symbols,
        scope_context.scope_id,
        scope_context.current_pou_symbol,
        scope_context.this_type,
        name.as_str(),
    ) else {
        return Ok(expr);
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return Ok(expr);
    };
    if !matches!(symbol.kind, SymbolKind::EnumValue { .. }) {
        return Ok(expr);
    }

    let Some(symbol_enum_name) = semantic_enum_type_name(symbols, symbol.type_id) else {
        return Err(CompileError::new(format!(
            "resolved enum variant '{name}' has no enum type"
        )));
    };
    let Some(target_enum_name) = runtime_enum_type_name(ctx.registry, target_type_id) else {
        return Ok(expr);
    };
    if !symbol_enum_name.eq_ignore_ascii_case(target_enum_name.as_str()) {
        return Ok(expr);
    }

    if let Some(value) = enum_literal_value(name.as_str(), target_type_id, ctx.registry) {
        return Ok(Expr::Literal(value));
    }
    Err(CompileError::new(format!(
        "failed to lower enum variant '{target_enum_name}#{name}'"
    )))
}
