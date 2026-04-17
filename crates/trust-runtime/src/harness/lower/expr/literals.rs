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
        return Some(Value::Enum(Box::new(EnumValue {
            type_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            numeric_value: *numeric_value,
        })));
    }
    None
}
