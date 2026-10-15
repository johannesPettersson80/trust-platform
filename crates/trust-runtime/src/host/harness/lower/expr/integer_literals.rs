/// Magnitude of an integer literal. The sign is a separate token, so the text is unsigned and
/// the full u64 range is parsed exactly; the caller decides which range the destination takes.
fn parse_int_literal(text: &str) -> Result<u64, CompileError> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str
            .parse()
            .map_err(|_| CompileError::new("invalid integer literal base"))?;
        return u64::from_str_radix(digits, base).map_err(integer_digits_error);
    }
    cleaned.parse::<u64>().map_err(integer_digits_error)
}

/// Digits that overflow u64 are out of range; any other parse failure is an invalid literal.
fn integer_digits_error(error: std::num::ParseIntError) -> CompileError {
    if matches!(error.kind(), std::num::IntErrorKind::PosOverflow) {
        integer_out_of_range()
    } else {
        CompileError::new("invalid integer literal")
    }
}

/// Integer types that an untyped literal takes from its destination. Any other destination keeps
/// the i32 default.
const INTEGER_DESTINATIONS: [TypeId; 8] = [
    TypeId::SINT,
    TypeId::INT,
    TypeId::DINT,
    TypeId::LINT,
    TypeId::USINT,
    TypeId::UINT,
    TypeId::UDINT,
    TypeId::ULINT,
];

/// The integer type a destination names, directly or through an alias chain, when it is one of
/// `INTEGER_DESTINATIONS`. A subrange takes the integer type it is built on; its bounds are checked
/// by the analysis, not here.
fn integer_destination(registry: &TypeRegistry, type_id: TypeId) -> Option<TypeId> {
    let mut current = type_id;
    let mut guard = 0;
    while guard < 16 {
        if INTEGER_DESTINATIONS.contains(&current) {
            return Some(current);
        }
        let Some(Type::Alias { target, .. } | Type::Subrange { base: target, .. }) =
            registry.get(current)
        else {
            return None;
        };
        current = *target;
        guard += 1;
    }
    None
}

/// Value of an integer literal with its sign applied. A destination integer type takes the value
/// when it fits. Without one, a typed literal keeps the i64 range and an untyped literal takes the
/// i32 default.
fn integer_literal_value(
    magnitude: u64,
    negative: bool,
    target: Option<TypeId>,
    typed: bool,
) -> Result<Value, CompileError> {
    let value = signed_integer(magnitude, negative);
    if let Some(target) = target {
        return integer_value_for(target, value);
    }
    if typed {
        let Ok(value) = i64::try_from(value) else {
            return Err(integer_out_of_range());
        };
        return Ok(Value::LInt(value));
    }
    let Ok(value) = i32::try_from(value) else {
        return Err(integer_out_of_range());
    };
    Ok(Value::DInt(value))
}

/// The value `value` takes as `target`, which is one of `INTEGER_DESTINATIONS`.
fn integer_value_for(target: TypeId, value: i128) -> Result<Value, CompileError> {
    let converted = match target {
        TypeId::SINT => i8::try_from(value).map(Value::SInt),
        TypeId::INT => i16::try_from(value).map(Value::Int),
        TypeId::DINT => i32::try_from(value).map(Value::DInt),
        TypeId::LINT => i64::try_from(value).map(Value::LInt),
        TypeId::USINT => u8::try_from(value).map(Value::USInt),
        TypeId::UINT => u16::try_from(value).map(Value::UInt),
        TypeId::UDINT => u32::try_from(value).map(Value::UDInt),
        TypeId::ULINT => u64::try_from(value).map(Value::ULInt),
        _ => return Err(integer_out_of_range()),
    };
    converted.map_err(|_| integer_out_of_range())
}

fn signed_integer(magnitude: u64, negative: bool) -> i128 {
    let value = i128::from(magnitude);
    if negative {
        -value
    } else {
        value
    }
}

fn integer_out_of_range() -> CompileError {
    CompileError::new("integer literal out of range")
}

/// A minus sign written directly before an untyped integer literal belongs to the literal, so
/// `-2147483648` fits DINT. Returns None, which keeps the unary operator, unless the destination is
/// one of `INTEGER_DESTINATIONS`.
fn lower_negated_integer_literal(
    node: &SyntaxNode,
    ctx: &LoweringContext<'_>,
    expected_type: Option<TypeId>,
) -> Result<Option<Expr>, CompileError> {
    if node.kind() != SyntaxKind::Literal {
        return Ok(None);
    }
    let target = expected_type.and_then(|type_id| integer_destination(ctx.registry, type_id));
    let Some(target) = target else {
        return Ok(None);
    };
    let mut digits = None;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::IntLiteral => digits = Some(token.text().to_string()),
            SyntaxKind::TypedLiteralPrefix | SyntaxKind::Plus | SyntaxKind::Minus => {
                return Ok(None);
            }
            _ => {}
        }
    }
    let Some(digits) = digits else {
        return Ok(None);
    };
    let magnitude = parse_int_literal(&digits)?;
    let value = integer_value_for(target, signed_integer(magnitude, true))?;
    Ok(Some(Expr::Literal(value)))
}
