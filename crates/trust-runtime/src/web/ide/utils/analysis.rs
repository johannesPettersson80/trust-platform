use super::*;

pub(in crate::web::ide) fn map_definition_location(
    context: &AnalysisContext,
    result: trust_ide::DefinitionResult,
) -> Option<IdeLocation> {
    let path = context.path_by_file_id.get(&result.file_id)?.clone();
    let text = context.text_by_file.get(&result.file_id)?;
    Some(IdeLocation {
        path,
        range: text_range_to_ide_range(text, result.range),
    })
}

pub(in crate::web::ide) fn map_reference_location(
    context: &AnalysisContext,
    reference: trust_ide::Reference,
) -> Option<IdeLocation> {
    let path = context.path_by_file_id.get(&reference.file_id)?.clone();
    let text = context.text_by_file.get(&reference.file_id)?;
    Some(IdeLocation {
        path,
        range: text_range_to_ide_range(text, reference.range),
    })
}

pub(in crate::web::ide) fn text_range_to_ide_range(text: &str, range: TextRange) -> IdeRange {
    IdeRange {
        start: text_offset_to_position(text, range.start()),
        end: text_offset_to_position(text, range.end()),
    }
}

pub(in crate::web::ide) fn position_to_text_size(text: &str, position: &Position) -> TextSize {
    let line_idx = position.line as usize;
    let char_idx = position.character as usize;
    let mut start = 0usize;
    for (current, line) in text.split('\n').enumerate() {
        if current == line_idx {
            let mut byte_in_line = line.len();
            if char_idx == 0 {
                byte_in_line = 0;
            } else {
                for (count, (idx, _)) in line.char_indices().enumerate() {
                    if count == char_idx {
                        byte_in_line = idx;
                        break;
                    }
                }
            }
            return TextSize::from((start + byte_in_line) as u32);
        }
        start = start.saturating_add(line.len() + 1);
    }
    TextSize::from(text.len() as u32)
}

pub(in crate::web::ide) fn text_offset_to_position(text: &str, offset: TextSize) -> IdePosition {
    let offset = u32::from(offset) as usize;
    let safe_offset = offset.min(text.len());
    let prefix = &text[..safe_offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let char_idx = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count())
        .unwrap_or_else(|| prefix.chars().count());
    IdePosition {
        line: line as u32,
        character: char_idx as u32,
    }
}

pub(in crate::web::ide) fn apply_text_edits(
    text: &str,
    edits: &[trust_ide::rename::TextEdit],
) -> Result<String, IdeError> {
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|edit| std::cmp::Reverse(edit.range.start()));

    let mut output = text.to_string();
    for edit in sorted {
        let start = usize::try_from(u32::from(edit.range.start())).map_err(|_| {
            IdeError::new(
                IdeErrorKind::InvalidInput,
                "invalid rename edit range start",
            )
        })?;
        let end = usize::try_from(u32::from(edit.range.end())).map_err(|_| {
            IdeError::new(IdeErrorKind::InvalidInput, "invalid rename edit range end")
        })?;
        if start > end || end > output.len() {
            return Err(IdeError::new(
                IdeErrorKind::InvalidInput,
                "rename edit range out of bounds",
            ));
        }
        output.replace_range(start..end, edit.new_text.as_str());
    }
    Ok(output)
}

pub(in crate::web::ide) fn symbol_kind_label(
    kind: &trust_hir::symbols::SymbolKind,
) -> &'static str {
    use trust_hir::symbols::SymbolKind;
    match kind {
        SymbolKind::Program => "program",
        SymbolKind::Configuration => "configuration",
        SymbolKind::Resource => "resource",
        SymbolKind::Task => "task",
        SymbolKind::ProgramInstance => "program_instance",
        SymbolKind::Namespace => "namespace",
        SymbolKind::Function { .. } => "function",
        SymbolKind::FunctionBlock => "function_block",
        SymbolKind::Class => "class",
        SymbolKind::Method { .. } => "method",
        SymbolKind::Property { .. } => "property",
        SymbolKind::Interface => "interface",
        SymbolKind::Variable { .. } => "variable",
        SymbolKind::Constant => "constant",
        SymbolKind::Type => "type",
        SymbolKind::EnumValue { .. } => "enum_value",
        SymbolKind::Parameter { .. } => "parameter",
    }
}

pub(in crate::web::ide) fn extract_symbol_hits(
    context: &AnalysisContext,
    filter_path: Option<&str>,
    query: &str,
    limit: usize,
) -> Vec<IdeSymbolHit> {
    let query = query.trim().to_ascii_lowercase();
    let mut hits = Vec::new();
    for (path, file_id) in &context.file_id_by_path {
        if let Some(expected) = filter_path {
            if path != expected {
                continue;
            }
        }
        let symbols = context.db.file_symbols(*file_id);
        let Some(source) = context.text_by_file.get(file_id) else {
            continue;
        };
        for symbol in symbols.iter() {
            if symbol.name.is_empty() {
                continue;
            }
            if !query.is_empty() && !symbol.name.to_ascii_lowercase().contains(&query) {
                continue;
            }
            let pos = text_offset_to_position(source, symbol.range.start());
            hits.push(IdeSymbolHit {
                path: path.clone(),
                name: symbol.name.to_string(),
                kind: symbol_kind_label(&symbol.kind).to_string(),
                line: pos.line,
                character: pos.character,
            });
            if hits.len() >= limit {
                return hits;
            }
        }
    }
    hits
}

pub(in crate::web::ide) fn map_analysis_error(error: impl std::fmt::Display) -> IdeError {
    IdeError::new(
        IdeErrorKind::InvalidInput,
        format!("analysis error: {error}"),
    )
}

pub(in crate::web::ide) fn apply_completion_relevance_contract(
    items: &mut Vec<CompletionItem>,
    text: &str,
    position: Position,
    limit: Option<u32>,
) {
    let prefix = completion_prefix(text, position);
    if prefix.is_empty() {
        if let Some(max) = limit {
            items.truncate(max as usize);
        }
        return;
    }
    let prefix_lower = prefix.to_ascii_lowercase();

    let mut seen_labels: BTreeSet<String> = items
        .iter()
        .map(|item| item.label.to_ascii_lowercase())
        .collect();
    let mut fallback_symbols = extract_in_scope_symbols(text)
        .into_iter()
        .filter(|symbol| {
            let lowered = symbol.to_ascii_lowercase();
            lowered.starts_with(&prefix_lower) && !seen_labels.contains(&lowered)
        })
        .collect::<Vec<_>>();
    fallback_symbols.sort();

    if !fallback_symbols.is_empty() {
        let mut prefixed = Vec::with_capacity(fallback_symbols.len() + items.len());
        for symbol in fallback_symbols {
            seen_labels.insert(symbol.to_ascii_lowercase());
            prefixed.push(CompletionItem {
                label: symbol.clone(),
                kind: "symbol".to_string(),
                detail: Some("in-scope symbol".to_string()),
                documentation: None,
                insert_text: Some(symbol),
                text_edit: None,
                sort_priority: 0,
            });
        }
        prefixed.append(items);
        *items = prefixed;
    }

    items.sort_by(|a, b| {
        let rank_a = completion_rank(a, &prefix_lower);
        let rank_b = completion_rank(b, &prefix_lower);
        rank_a
            .cmp(&rank_b)
            .then(a.sort_priority.cmp(&b.sort_priority))
            .then(a.label.cmp(&b.label))
    });

    let mut deduped = Vec::with_capacity(items.len());
    let mut seen = BTreeSet::new();
    for item in items.drain(..) {
        let key = item.label.to_ascii_lowercase();
        if seen.insert(key) {
            deduped.push(item);
        }
    }
    if let Some(max) = limit {
        deduped.truncate(max as usize);
    }
    *items = deduped;
}

pub(in crate::web::ide) fn completion_rank(item: &CompletionItem, prefix_lower: &str) -> u8 {
    let label_lower = item.label.to_ascii_lowercase();
    if label_lower.starts_with(prefix_lower) {
        let kind_lower = item.kind.to_ascii_lowercase();
        if kind_lower == "keyword" {
            return 1;
        }
        return 0;
    }
    2
}

pub(in crate::web::ide) fn completion_prefix(text: &str, position: Position) -> String {
    let line = text
        .split('\n')
        .nth(position.line as usize)
        .unwrap_or_default();
    let mut char_to_byte = 0_usize;
    for (count, (idx, _ch)) in line.char_indices().enumerate() {
        if count == position.character as usize {
            char_to_byte = idx;
            break;
        }
        char_to_byte = line.len();
    }
    if (position.character as usize) == 0 {
        char_to_byte = 0;
    } else if (position.character as usize) >= line.chars().count() {
        char_to_byte = line.len();
    }
    let before = &line[..char_to_byte];
    let start = before
        .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    before[start..].trim().to_string()
}

pub(in crate::web::ide) fn extract_in_scope_symbols(text: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("(*") {
            continue;
        }
        for keyword in ["PROGRAM", "FUNCTION", "FUNCTION_BLOCK", "TYPE", "CLASS"] {
            if let Some(rest) = line.strip_prefix(keyword) {
                let candidate = rest.split_whitespace().next().unwrap_or_default();
                if is_identifier(candidate) {
                    symbols.insert(candidate.to_string());
                }
            }
        }
        if let Some((lhs, _rhs)) = line.split_once(':') {
            for part in lhs.split(',') {
                let candidate = part
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(';');
                if is_identifier(candidate) {
                    symbols.insert(candidate.to_string());
                }
            }
        }
    }
    symbols
}

pub(in crate::web::ide) fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
