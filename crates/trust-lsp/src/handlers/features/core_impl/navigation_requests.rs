use super::*;

pub fn goto_definition(
    state: &ServerState,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let result = state
        .with_database(|db| trust_ide::goto_definition(db, doc.file_id, TextSize::from(offset)))?;

    let (target_uri, target_content) = if result.file_id == doc.file_id {
        (uri.clone(), doc.content.clone())
    } else {
        let target_doc = state.document_for_file_id(result.file_id)?;
        (target_doc.uri, target_doc.content)
    };

    let range = Range {
        start: offset_to_position(&target_content, result.range.start().into()),
        end: offset_to_position(&target_content, result.range.end().into()),
    };

    Some(GotoDefinitionResponse::Scalar(Location {
        uri: target_uri,
        range,
    }))
}

pub fn goto_declaration(
    state: &ServerState,
    params: GotoDeclarationParams,
) -> Option<GotoDeclarationResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let result = state
        .with_database(|db| trust_ide::goto_declaration(db, doc.file_id, TextSize::from(offset)))?;

    let (target_uri, target_content) = if result.file_id == doc.file_id {
        (uri.clone(), doc.content.clone())
    } else {
        let target_doc = state.document_for_file_id(result.file_id)?;
        (target_doc.uri, target_doc.content)
    };

    let range = Range {
        start: offset_to_position(&target_content, result.range.start().into()),
        end: offset_to_position(&target_content, result.range.end().into()),
    };

    Some(GotoDeclarationResponse::Scalar(Location {
        uri: target_uri,
        range,
    }))
}

pub fn goto_type_definition(
    state: &ServerState,
    params: GotoTypeDefinitionParams,
) -> Option<GotoTypeDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let result = state.with_database(|db| {
        trust_ide::goto_type_definition(db, doc.file_id, TextSize::from(offset))
    })?;

    let (target_uri, target_content) = if result.file_id == doc.file_id {
        (uri.clone(), doc.content.clone())
    } else {
        let target_doc = state.document_for_file_id(result.file_id)?;
        (target_doc.uri, target_doc.content)
    };

    let range = Range {
        start: offset_to_position(&target_content, result.range.start().into()),
        end: offset_to_position(&target_content, result.range.end().into()),
    };

    Some(GotoTypeDefinitionResponse::Scalar(Location {
        uri: target_uri,
        range,
    }))
}

pub fn goto_implementation(
    state: &ServerState,
    params: GotoImplementationParams,
) -> Option<GotoImplementationResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let results = state.with_database(|db| {
        trust_ide::goto_implementation(db, doc.file_id, TextSize::from(offset))
    });

    if results.is_empty() {
        return None;
    }

    let locations = results
        .into_iter()
        .filter_map(|result| {
            let target_doc = state.document_for_file_id(result.file_id)?;
            let range = Range {
                start: offset_to_position(&target_doc.content, result.range.start().into()),
                end: offset_to_position(&target_doc.content, result.range.end().into()),
            };
            Some(Location {
                uri: target_doc.uri,
                range,
            })
        })
        .collect();

    Some(GotoImplementationResponse::Array(locations))
}

pub fn references(state: &ServerState, params: ReferenceParams) -> Option<Vec<Location>> {
    let request_ticket = state.begin_semantic_request();
    references_with_ticket(state, params, request_ticket)
}

fn references_with_ticket(
    state: &ServerState,
    params: ReferenceParams,
    request_ticket: u64,
) -> Option<Vec<Location>> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let options = trust_ide::references::FindReferencesOptions {
        include_declaration: params.context.include_declaration,
    };
    let refs = state.with_database(|db| {
        trust_ide::find_references(db, doc.file_id, TextSize::from(offset), options)
    });

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let mut locations = Vec::new();
    for reference in refs {
        if state.semantic_request_cancelled(request_ticket) {
            return None;
        }
        let Some(target_doc) = state.document_for_file_id(reference.file_id) else {
            continue;
        };
        let range = Range {
            start: offset_to_position(&target_doc.content, reference.range.start().into()),
            end: offset_to_position(&target_doc.content, reference.range.end().into()),
        };
        locations.push(Location {
            uri: target_doc.uri,
            range,
        });
    }

    Some(locations)
}

pub async fn references_with_progress(
    client: &Client,
    state: &ServerState,
    params: ReferenceParams,
) -> Option<Vec<Location>> {
    let work_done_token = params.work_done_progress_params.work_done_token.clone();
    let partial_token = params.partial_result_params.partial_result_token.clone();
    send_work_done_begin(client, &work_done_token, "Finding references", None).await;
    let result = references(state, params);

    let streamed_partials = partial_token.is_some();
    if let Some(locations) = result.as_ref() {
        if streamed_partials {
            let total = locations.len().max(1);
            let mut emitted = 0usize;
            for chunk in locations.chunks(PARTIAL_CHUNK_SIZE) {
                send_partial_result(client, &partial_token, chunk.to_vec()).await;
                emitted = emitted.saturating_add(chunk.len());
                let percentage = ((emitted as f64 / total as f64) * 100.0).round() as u32;
                send_work_done_report(
                    client,
                    &work_done_token,
                    Some(format!("References: {emitted}/{total}")),
                    Some(percentage.min(100)),
                )
                .await;
            }
        }
    }

    let count = result.as_ref().map(|items| items.len()).unwrap_or(0);
    send_work_done_end(
        client,
        &work_done_token,
        Some(format!("Found {count} reference(s)")),
    )
    .await;
    if streamed_partials {
        result.map(|_| Vec::new())
    } else {
        result
    }
}

pub fn document_highlight(
    state: &ServerState,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let references = state.with_database(|db| {
        trust_ide::references::find_references_in_file(
            db,
            doc.file_id,
            TextSize::from(offset),
            trust_ide::FindReferencesOptions {
                include_declaration: true,
            },
        )
    });

    let highlights = references
        .into_iter()
        .filter(|reference| reference.file_id == doc.file_id)
        .map(|reference| DocumentHighlight {
            range: Range {
                start: offset_to_position(&doc.content, reference.range.start().into()),
                end: offset_to_position(&doc.content, reference.range.end().into()),
            },
            kind: Some(if reference.is_write {
                DocumentHighlightKind::WRITE
            } else {
                DocumentHighlightKind::READ
            }),
        })
        .collect::<Vec<_>>();

    Some(highlights)
}
