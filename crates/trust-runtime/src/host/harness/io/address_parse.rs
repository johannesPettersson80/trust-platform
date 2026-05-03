fn parse_field_address(text: &str) -> Result<FieldAddress, CompileError> {
    let trimmed = text.trim();
    if !trimmed.starts_with('%') || trimmed.len() < 2 {
        return Err(CompileError::new("invalid direct address"));
    }
    let mut chars = trimmed[1..].chars();
    let Some(prefix) = chars.next() else {
        return Err(CompileError::new("invalid direct address"));
    };
    match prefix {
        'I' | 'Q' | 'M' => {
            let address = IoAddress::parse(trimmed)
                .map_err(|err| CompileError::new(format!("invalid I/O address: {err}")))?;
            Ok(FieldAddress::Absolute(address))
        }
        'X' | 'B' | 'W' | 'D' | 'L' => parse_relative_address(trimmed),
        _ => Err(CompileError::new("invalid direct address")),
    }
}

fn parse_relative_address(text: &str) -> Result<FieldAddress, CompileError> {
    let trimmed = text.trim();
    if !trimmed.starts_with('%') || trimmed.len() < 3 {
        return Err(CompileError::new("invalid relative address"));
    }
    let mut chars = trimmed[1..].chars();
    let Some(size) = chars.next() else {
        return Err(CompileError::new("invalid relative address"));
    };
    let rest = chars.as_str();
    if rest.is_empty() {
        return Err(CompileError::new("invalid relative address"));
    }
    match size {
        'X' => {
            let mut parts = rest.split('.');
            let byte_part = parts
                .next()
                .ok_or_else(|| CompileError::new("invalid relative address"))?;
            let byte = byte_part
                .parse::<u64>()
                .map_err(|_| CompileError::new("invalid relative address"))?;
            let bit = match parts.next() {
                Some(bit_part) if !bit_part.is_empty() => bit_part
                    .parse::<u8>()
                    .map_err(|_| CompileError::new("invalid relative address"))?,
                _ => 0,
            };
            if bit > 7 || parts.next().is_some() {
                return Err(CompileError::new("invalid relative address"));
            }
            Ok(FieldAddress::Relative {
                offset_bytes: byte,
                bit_offset: bit,
            })
        }
        'B' | 'W' | 'D' | 'L' => {
            if rest.contains('.') {
                return Err(CompileError::new("invalid relative address"));
            }
            let byte = rest
                .parse::<u64>()
                .map_err(|_| CompileError::new("invalid relative address"))?;
            Ok(FieldAddress::Relative {
                offset_bytes: byte,
                bit_offset: 0,
            })
        }
        _ => Err(CompileError::new("invalid relative address")),
    }
}
