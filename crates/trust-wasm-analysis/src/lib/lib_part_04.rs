impl From<EngineError> for String {
    fn from(value: EngineError) -> Self {
        value.to_string()
    }
}

fn json_string<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|err| format!("json serialization failed: {err}"))
}

fn source_key(uri: &str) -> SourceKey {
    SourceKey::from_virtual(uri.to_string())
}

fn completion_prefix_at_offset(source: &str, offset: u32) -> Option<String> {
    let bytes = source.as_bytes();
    let mut cursor = (offset as usize).min(bytes.len());
    let end = cursor;
    while cursor > 0 && is_ident_byte(bytes[cursor - 1]) {
        cursor -= 1;
    }
    if cursor == end {
        return None;
    }
    let prefix = &source[cursor..end];
    if prefix.is_empty() {
        return None;
    }
    Some(prefix.to_ascii_uppercase())
}

fn completion_match_rank(label: &str, typed_prefix: Option<&str>) -> u8 {
    let Some(prefix) = typed_prefix else {
        return 2;
    };
    if prefix.is_empty() {
        return 2;
    }
    let label_upper = label.to_ascii_uppercase();
    if label_upper == prefix {
        return 0;
    }
    if label_upper.starts_with(prefix) {
        return 1;
    }
    if label_upper.contains(prefix) {
        return 2;
    }
    3
}

fn is_ident_byte(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

fn completion_kind_label(kind: trust_ide::CompletionKind) -> &'static str {
    match kind {
        trust_ide::CompletionKind::Keyword => "keyword",
        trust_ide::CompletionKind::Function => "function",
        trust_ide::CompletionKind::FunctionBlock => "function_block",
        trust_ide::CompletionKind::Method => "method",
        trust_ide::CompletionKind::Property => "property",
        trust_ide::CompletionKind::Variable => "variable",
        trust_ide::CompletionKind::Constant => "constant",
        trust_ide::CompletionKind::Type => "type",
        trust_ide::CompletionKind::EnumValue => "enum_value",
        trust_ide::CompletionKind::Snippet => "snippet",
    }
}

fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

fn byte_span(range: TextRange) -> ByteSpan {
    ByteSpan {
        start: u32::from(range.start()),
        end: u32::from(range.end()),
    }
}

fn lsp_range(content: &str, range: TextRange) -> Range {
    Range {
        start: offset_to_position(content, u32::from(range.start())),
        end: offset_to_position(content, u32::from(range.end())),
    }
}

fn offset_to_position(content: &str, offset: u32) -> Position {
    let clamped_offset = (offset as usize).min(content.len());
    let mut line = 0u32;
    let mut character = 0u32;
    for (index, ch) in content.char_indices() {
        if index >= clamped_offset {
            break;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = character.saturating_add(ch.len_utf16() as u32);
        }
    }
    Position { line, character }
}

fn position_to_offset(content: &str, position: Position) -> Option<u32> {
    let mut line = 0u32;
    let mut character = 0u32;
    for (index, ch) in content.char_indices() {
        if line == position.line {
            if character == position.character {
                return Some(index as u32);
            }
            if ch == '\n' {
                return Some(index as u32);
            }
            let width = ch.len_utf16() as u32;
            if character.saturating_add(width) > position.character {
                return Some(index as u32);
            }
            character = character.saturating_add(width);
            continue;
        }

        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        }
    }
    if line == position.line {
        Some(content.len() as u32)
    } else {
        None
    }
}
