use super::*;

pub fn semantic_tokens_full(
    state: &ServerState,
    params: SemanticTokensParams,
) -> Option<SemanticTokensResult> {
    let request_ticket = state.begin_semantic_request();
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    // Get semantic tokens from trust_ide
    let tokens = state.with_database(|db| trust_ide::semantic_tokens(db, doc.file_id));

    let data = semantic_tokens_to_lsp(&doc.content, tokens, 0, 0);
    let result_id = state.store_semantic_tokens(uri.clone(), data.clone(), request_ticket)?;

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: Some(result_id),
        data,
    }))
}

pub fn semantic_tokens_full_delta(
    state: &ServerState,
    params: SemanticTokensDeltaParams,
) -> Option<SemanticTokensFullDeltaResult> {
    let request_ticket = state.begin_semantic_request();
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let tokens = state.with_database(|db| trust_ide::semantic_tokens(db, doc.file_id));
    let data = semantic_tokens_to_lsp(&doc.content, tokens, 0, 0);

    let previous = state.semantic_tokens_cache(uri);
    let result_id = state.store_semantic_tokens(uri.clone(), data.clone(), request_ticket)?;

    if let Some(previous) = previous {
        if previous.result_id == params.previous_result_id {
            if let Some(edits) = semantic_tokens_delta_edits(&previous.tokens, &data) {
                let delta = SemanticTokensDelta {
                    result_id: Some(result_id),
                    edits,
                };
                return Some(SemanticTokensFullDeltaResult::TokensDelta(delta));
            }
        }
    }

    Some(SemanticTokensFullDeltaResult::Tokens(SemanticTokens {
        result_id: Some(result_id),
        data,
    }))
}

pub fn semantic_tokens_range(
    state: &ServerState,
    params: SemanticTokensRangeParams,
) -> Option<SemanticTokensRangeResult> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let start_offset = position_to_offset(&doc.content, params.range.start)?;
    let end_offset = position_to_offset(&doc.content, params.range.end)?;
    if end_offset <= start_offset {
        return Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
            result_id: None,
            data: Vec::new(),
        }));
    }

    let tokens = state.with_database(|db| trust_ide::semantic_tokens(db, doc.file_id));
    let filtered = tokens
        .into_iter()
        .filter(|token| {
            let start = u32::from(token.range.start());
            start >= start_offset && start < end_offset
        })
        .collect::<Vec<_>>();

    let (origin_line, origin_col) = offset_to_line_col(&doc.content, start_offset);
    let data = semantic_tokens_to_lsp(&doc.content, filtered, origin_line, origin_col);

    Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}

fn semantic_tokens_delta_edits(
    previous: &[SemanticToken],
    current: &[SemanticToken],
) -> Option<Vec<SemanticTokensEdit>> {
    if previous == current {
        return Some(Vec::new());
    }

    let min_len = previous.len().min(current.len());
    let mut prefix = 0usize;
    while prefix < min_len && previous[prefix] == current[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min_len - prefix)
        && previous[previous.len() - 1 - suffix] == current[current.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_mid_len = previous.len().saturating_sub(prefix + suffix);
    let new_mid = &current[prefix..current.len().saturating_sub(suffix)];

    let edit = SemanticTokensEdit {
        start: (prefix * 5) as u32,
        delete_count: (old_mid_len * 5) as u32,
        data: if new_mid.is_empty() {
            None
        } else {
            Some(new_mid.to_vec())
        },
    };

    Some(vec![edit])
}

pub fn folding_range(state: &ServerState, params: FoldingRangeParams) -> Option<Vec<FoldingRange>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;
    let parsed = parse(&doc.content);
    let root = parsed.syntax();

    let mut ranges = Vec::new();
    for node in root.descendants() {
        if !is_foldable_kind(node.kind()) {
            continue;
        }
        let range = node.text_range();
        let (start_line, _) = offset_to_line_col(&doc.content, range.start().into());
        let (mut end_line, end_col) = offset_to_line_col(&doc.content, range.end().into());
        if end_line > start_line && end_col == 0 {
            end_line = end_line.saturating_sub(1);
        }
        if end_line > start_line {
            ranges.push(FoldingRange {
                start_line,
                start_character: None,
                end_line,
                end_character: None,
                kind: Some(FoldingRangeKind::Region),
                collapsed_text: None,
            });
        }
    }

    Some(ranges)
}

pub fn selection_range(
    state: &ServerState,
    params: SelectionRangeParams,
) -> Option<Vec<SelectionRange>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let mut offsets = Vec::with_capacity(params.positions.len());
    for position in &params.positions {
        offsets.push(TextSize::from(position_to_offset(&doc.content, *position)?));
    }

    let ranges = state.with_database(|db| trust_ide::selection_ranges(db, doc.file_id, &offsets));
    let lsp_ranges = ranges
        .into_iter()
        .map(|range| selection_range_to_lsp(&doc.content, range))
        .collect();

    Some(lsp_ranges)
}

pub fn linked_editing_range(
    state: &ServerState,
    params: LinkedEditingRangeParams,
) -> Option<LinkedEditingRanges> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let ranges = state.with_database(|db| {
        trust_ide::linked_editing_ranges(db, doc.file_id, TextSize::from(offset))
    })?;

    let lsp_ranges = ranges
        .into_iter()
        .map(|range| Range {
            start: offset_to_position(&doc.content, range.start().into()),
            end: offset_to_position(&doc.content, range.end().into()),
        })
        .collect();

    Some(LinkedEditingRanges {
        ranges: lsp_ranges,
        word_pattern: Some("[A-Za-z_][A-Za-z0-9_]*".to_string()),
    })
}

pub fn inlay_hint(state: &ServerState, params: InlayHintParams) -> Option<Vec<InlayHint>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let start_offset = position_to_offset(&doc.content, params.range.start)?;
    let end_offset = position_to_offset(&doc.content, params.range.end)?;
    if end_offset < start_offset {
        return Some(Vec::new());
    }

    let hints = state.with_database(|db| {
        trust_ide::inlay_hints(
            db,
            doc.file_id,
            TextRange::new(TextSize::from(start_offset), TextSize::from(end_offset)),
        )
    });

    let lsp_hints = hints
        .into_iter()
        .map(|hint| {
            let position = offset_to_position(&doc.content, u32::from(hint.position));
            let kind = match hint.kind {
                trust_ide::InlayHintKind::Parameter => InlayHintKind::PARAMETER,
                trust_ide::InlayHintKind::Telemetry => InlayHintKind::TYPE,
            };
            InlayHint {
                position,
                label: InlayHintLabel::from(hint.label.to_string()),
                kind: Some(kind),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: Some(true),
                data: None,
            }
        })
        .collect();

    Some(lsp_hints)
}
