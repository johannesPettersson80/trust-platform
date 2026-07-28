use super::*;

pub(crate) fn native_completion(
    documents: &[DocumentInput],
    request: &CompletionRequest,
) -> Vec<CompletionItem> {
    let project = native_project(documents);
    let source = documents
        .iter()
        .find(|doc| doc.uri == request.uri)
        .map(|doc| doc.text.as_str())
        .expect("source exists");
    let file_id = project
        .file_id_for_key(&SourceKey::from_virtual(request.uri.clone()))
        .expect("file id exists");
    let offset = position_to_offset_utf16(source, request.position.clone()).expect("offset");

    let mut items = project.with_database(|db| {
        trust_ide::complete_with_filter(
            db,
            file_id,
            TextSize::from(offset),
            &StdlibFilter::allow_all(),
        )
    });
    let typed_prefix = completion_prefix_at_offset(source, offset);
    items.sort_by(|left, right| {
        completion_match_rank(left.label.as_str(), typed_prefix.as_deref())
            .cmp(&completion_match_rank(
                right.label.as_str(),
                typed_prefix.as_deref(),
            ))
            .then_with(|| left.sort_priority.cmp(&right.sort_priority))
            .then_with(|| left.label.cmp(&right.label))
    });
    let limit = request.limit.unwrap_or(50).clamp(1, 500) as usize;
    items
        .into_iter()
        .take(limit)
        .map(|item| CompletionItem {
            label: item.label.to_string(),
            kind: completion_kind_label(item.kind).to_string(),
            detail: item.detail.map(|value| value.to_string()),
            documentation: item.documentation.map(|value| value.to_string()),
            insert_text: item.insert_text.map(|value| value.to_string()),
            text_edit: item
                .text_edit
                .map(|edit| trust_wasm_analysis::CompletionTextEditItem {
                    range: text_range_to_lsp(source, edit.range),
                    new_text: edit.new_text.to_string(),
                }),
            sort_priority: item.sort_priority,
        })
        .collect()
}

pub(crate) fn completion_prefix_at_offset(source: &str, offset: u32) -> Option<String> {
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

pub(crate) fn completion_match_rank(label: &str, typed_prefix: Option<&str>) -> u8 {
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

pub(crate) fn is_ident_byte(byte: u8) -> bool {
    matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

pub(crate) fn completion_kind_label(kind: trust_ide::CompletionKind) -> &'static str {
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

pub(crate) fn severity_label(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Hint => "hint",
    }
}

pub(crate) fn native_project(documents: &[DocumentInput]) -> Project {
    let mut project = Project::default();
    for document in documents {
        project.set_source_text(
            SourceKey::from_virtual(document.uri.clone()),
            document.text.clone(),
        );
    }
    project
}

pub(crate) fn text_range_to_lsp(content: &str, range: TextRange) -> Range {
    Range {
        start: offset_to_position_utf16(content, u32::from(range.start())),
        end: offset_to_position_utf16(content, u32::from(range.end())),
    }
}

pub(crate) fn offset_to_position_utf16(content: &str, offset: u32) -> Position {
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

pub(crate) fn position_to_offset_utf16(content: &str, position: Position) -> Option<u32> {
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

pub(crate) fn load_plant_demo_documents() -> Vec<DocumentInput> {
    let files = [
        (
            "types.st",
            include_str!("../../../../examples/plant_demo/src/types.st"),
        ),
        (
            "fb_pump.st",
            include_str!("../../../../examples/plant_demo/src/fb_pump.st"),
        ),
        (
            "program.st",
            include_str!("../../../../examples/plant_demo/src/program.st"),
        ),
        (
            "config.st",
            include_str!("../../../../examples/plant_demo/src/config.st"),
        ),
    ];
    files
        .into_iter()
        .map(|(name, text)| DocumentInput {
            uri: format!("memory:///plant_demo/{name}"),
            text: text.to_string(),
        })
        .collect()
}
