use super::*;

pub fn hover(state: &ServerState, params: HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;
    let stdlib_filter = stdlib_filter_for_uri(state, uri);

    let mut result = state.with_database(|db| {
        trust_ide::hover_with_filter(db, doc.file_id, TextSize::from(offset), &stdlib_filter)
    })?;

    if let Some(docs) = state.library_docs_for_uri(uri)
        && !docs.is_empty()
    {
        let symbol_name = state.with_database(|db| {
            trust_ide::symbol_name_at_position(db, doc.file_id, TextSize::from(offset))
        });
        if let Some(name) = symbol_name
            && let Some(extra) = doc_for_name(docs.as_ref(), name.as_str())
            && !result.contents.contains(extra)
        {
            result.contents.push_str("\n\n---\n\n");
            result.contents.push_str(extra);
        }
    }

    let range = result.range.map(|r| Range {
        start: offset_to_position(&doc.content, r.start().into()),
        end: offset_to_position(&doc.content, r.end().into()),
    });

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: result.contents,
        }),
        range,
    })
}

pub fn completion(state: &ServerState, params: CompletionParams) -> Option<CompletionResponse> {
    let request_ticket = state.begin_semantic_request();
    completion_with_ticket(state, params, request_ticket)
}

fn completion_with_ticket(
    state: &ServerState,
    params: CompletionParams,
    request_ticket: u64,
) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;
    let stdlib_filter = stdlib_filter_for_uri(state, uri);

    // Get completions from trust_ide
    let items = state.with_database(|db| {
        trust_ide::complete_with_filter(db, doc.file_id, TextSize::from(offset), &stdlib_filter)
    });

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    // Convert to LSP completion items
    let mut lsp_items: Vec<CompletionItem> = items
        .into_iter()
        .map(|item| {
            let kind = match item.kind {
                trust_ide::CompletionKind::Keyword => CompletionItemKind::KEYWORD,
                trust_ide::CompletionKind::Function => CompletionItemKind::FUNCTION,
                trust_ide::CompletionKind::FunctionBlock => CompletionItemKind::CLASS,
                trust_ide::CompletionKind::Method => CompletionItemKind::METHOD,
                trust_ide::CompletionKind::Property => CompletionItemKind::PROPERTY,
                trust_ide::CompletionKind::Variable => CompletionItemKind::VARIABLE,
                trust_ide::CompletionKind::Constant => CompletionItemKind::CONSTANT,
                trust_ide::CompletionKind::Type => CompletionItemKind::CLASS,
                trust_ide::CompletionKind::EnumValue => CompletionItemKind::ENUM_MEMBER,
                trust_ide::CompletionKind::Snippet => CompletionItemKind::SNIPPET,
            };

            let text_edit = item.text_edit.as_ref().map(|edit| {
                let range = Range {
                    start: offset_to_position(&doc.content, edit.range.start().into()),
                    end: offset_to_position(&doc.content, edit.range.end().into()),
                };
                CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: edit.new_text.to_string(),
                })
            });

            let insert_text = if text_edit.is_some() {
                None
            } else {
                item.insert_text.as_ref().map(|s| s.to_string())
            };

            CompletionItem {
                label: item.label.to_string(),
                kind: Some(kind),
                detail: item.detail.map(|s| s.to_string()),
                documentation: item.documentation.map(|s| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: s.to_string(),
                    })
                }),
                insert_text,
                insert_text_format: if item.insert_text.is_some()
                    || item.text_edit.is_some()
                    || matches!(item.kind, trust_ide::CompletionKind::Snippet)
                {
                    Some(InsertTextFormat::SNIPPET)
                } else {
                    None
                },
                sort_text: Some(format!("{:05}", item.sort_priority)),
                text_edit,
                ..Default::default()
            }
        })
        .collect();

    if let Some(docs) = state.library_docs_for_uri(uri)
        && !docs.is_empty()
    {
        for item in &mut lsp_items {
            if let Some(extra) = doc_for_name(docs.as_ref(), &item.label) {
                append_completion_doc(item, extra);
            }
        }
    }

    Some(CompletionResponse::Array(lsp_items))
}

#[cfg(test)]
pub(crate) fn completion_with_ticket_for_tests(
    state: &ServerState,
    params: CompletionParams,
    request_ticket: u64,
) -> Option<CompletionResponse> {
    completion_with_ticket(state, params, request_ticket)
}

pub fn completion_resolve(_state: &ServerState, mut item: CompletionItem) -> CompletionItem {
    if item.detail.is_none() {
        if item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
            item.detail = Some("snippet".to_string());
        } else if let Some(kind) = item.kind {
            let detail = match kind {
                CompletionItemKind::KEYWORD => "keyword",
                CompletionItemKind::FUNCTION => "function",
                CompletionItemKind::METHOD => "method",
                CompletionItemKind::PROPERTY => "property",
                CompletionItemKind::VARIABLE => "variable",
                CompletionItemKind::CONSTANT => "constant",
                CompletionItemKind::CLASS => "type",
                CompletionItemKind::ENUM_MEMBER => "enum",
                _ => "symbol",
            };
            item.detail = Some(detail.to_string());
        }
    }
    if item.documentation.is_none()
        && item.insert_text_format == Some(InsertTextFormat::SNIPPET)
        && let Some(insert_text) = item.insert_text.as_deref()
    {
        let snippet = insert_text.replace('\t', "    ");
        let value = format!("```st\n{}\n```", snippet);
        item.documentation = Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }));
    }
    item
}

pub fn signature_help(state: &ServerState, params: SignatureHelpParams) -> Option<SignatureHelp> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let result = state
        .with_database(|db| trust_ide::signature_help(db, doc.file_id, TextSize::from(offset)))?;

    if result.signatures.is_empty() {
        return None;
    }

    let signatures = result
        .signatures
        .into_iter()
        .map(|sig| {
            let parameters = if sig.parameters.is_empty() {
                None
            } else {
                Some(
                    sig.parameters
                        .into_iter()
                        .map(|param| ParameterInformation {
                            label: ParameterLabel::Simple(param.label),
                            documentation: None,
                        })
                        .collect(),
                )
            };
            SignatureInformation {
                label: sig.label,
                documentation: None,
                parameters,
                active_parameter: None,
            }
        })
        .collect();

    Some(SignatureHelp {
        signatures,
        active_signature: Some(result.active_signature as u32),
        active_parameter: Some(result.active_parameter as u32),
    })
}
