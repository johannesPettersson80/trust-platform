use super::*;

pub fn document_symbol(
    state: &ServerState,
    params: DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let symbols = state.with_database(|db| db.file_symbols(doc.file_id));
    let result: Vec<SymbolInformation> = symbols
        .iter()
        .filter(|symbol| is_outline_symbol_kind(&symbol.kind))
        // Exclude builtin symbols (they have empty range at offset 0)
        .filter(|symbol| !symbol.range.is_empty())
        .map(|symbol| {
            let kind = lsp_symbol_kind(&symbols, symbol);
            let container_name = symbol_container_name(&symbols, symbol);

            #[allow(deprecated)]
            SymbolInformation {
                name: display_symbol_name(&symbols, symbol),
                kind,
                location: Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: offset_to_position(&doc.content, symbol.range.start().into()),
                        end: offset_to_position(&doc.content, symbol.range.end().into()),
                    },
                },
                container_name,
                tags: None,
                deprecated: None,
            }
        })
        .collect();

    Some(DocumentSymbolResponse::Flat(result))
}

fn is_outline_symbol_kind(kind: &HirSymbolKind) -> bool {
    matches!(
        kind,
        HirSymbolKind::Program
            | HirSymbolKind::Configuration
            | HirSymbolKind::Resource
            | HirSymbolKind::Task
            | HirSymbolKind::ProgramInstance
            | HirSymbolKind::Namespace
            | HirSymbolKind::Function { .. }
            | HirSymbolKind::FunctionBlock
            | HirSymbolKind::Class
            | HirSymbolKind::Interface
            | HirSymbolKind::Type
            | HirSymbolKind::EnumValue { .. }
            | HirSymbolKind::Method { .. }
            | HirSymbolKind::Property { .. }
    )
}

pub fn workspace_symbol(
    state: &ServerState,
    params: WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    let request_ticket = state.begin_semantic_request();
    workspace_symbol_with_ticket(state, params, request_ticket)
}

fn workspace_symbol_with_ticket(
    state: &ServerState,
    params: WorkspaceSymbolParams,
    request_ticket: u64,
) -> Option<Vec<SymbolInformation>> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let query = params.query.trim().to_lowercase();
    let query_empty = query.is_empty();

    let file_ids = state.with_database(|db| db.file_ids());
    let mut result = Vec::new();

    for file_id in file_ids {
        if state.semantic_request_cancelled(request_ticket) {
            return None;
        }

        let doc = match state.document_for_file_id(file_id) {
            Some(doc) => doc,
            None => continue,
        };

        let config = state.workspace_config_for_uri(&doc.uri);
        let (priority, visibility) = config
            .map(|config| (config.workspace.priority, config.workspace.visibility))
            .unwrap_or((0, WorkspaceVisibility::default()));
        if !visibility.allows_query(query_empty) {
            continue;
        }

        let symbols = state.with_database(|db| db.file_symbols(file_id));
        for symbol in symbols.iter() {
            if state.semantic_request_cancelled(request_ticket) {
                return None;
            }

            let name = display_symbol_name(&symbols, symbol);
            if !query_empty && !name.to_lowercase().contains(&query) {
                continue;
            }

            let kind = lsp_symbol_kind(&symbols, symbol);
            let range = Range {
                start: offset_to_position(&doc.content, symbol.range.start().into()),
                end: offset_to_position(&doc.content, symbol.range.end().into()),
            };
            let container_name = symbol_container_name(&symbols, symbol);

            #[allow(deprecated)]
            result.push((
                priority,
                SymbolInformation {
                    name,
                    kind,
                    location: Location {
                        uri: doc.uri.clone(),
                        range,
                    },
                    container_name,
                    tags: None,
                    deprecated: None,
                },
            ));
        }
    }

    result.sort_by(|(prio_a, sym_a), (prio_b, sym_b)| {
        prio_b
            .cmp(prio_a)
            .then_with(|| sym_a.name.cmp(&sym_b.name))
            .then_with(|| sym_a.location.uri.as_str().cmp(sym_b.location.uri.as_str()))
            // Keep snapshot order deterministic when multiple symbols share name+uri.
            .then_with(|| sym_a.container_name.cmp(&sym_b.container_name))
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .start
                    .line
                    .cmp(&sym_b.location.range.start.line)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .start
                    .character
                    .cmp(&sym_b.location.range.start.character)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .end
                    .line
                    .cmp(&sym_b.location.range.end.line)
            })
            .then_with(|| {
                sym_a
                    .location
                    .range
                    .end
                    .character
                    .cmp(&sym_b.location.range.end.character)
            })
    });
    let result = result.into_iter().map(|(_, symbol)| symbol).collect();
    Some(result)
}

pub async fn workspace_symbol_with_progress(
    client: &Client,
    state: &ServerState,
    params: WorkspaceSymbolParams,
) -> Option<Vec<SymbolInformation>> {
    let work_done_token = params.work_done_progress_params.work_done_token.clone();
    let partial_token = params.partial_result_params.partial_result_token.clone();
    let message = if params.query.trim().is_empty() {
        None
    } else {
        Some(format!("Query: {}", params.query))
    };
    send_work_done_begin(
        client,
        &work_done_token,
        "Searching workspace symbols",
        message,
    )
    .await;

    let result = workspace_symbol(state, params);

    let streamed_partials = partial_token.is_some();
    if let Some(symbols) = result.as_ref() {
        if streamed_partials {
            let total = symbols.len().max(1);
            let mut emitted = 0usize;
            for chunk in symbols.chunks(PARTIAL_CHUNK_SIZE) {
                send_partial_result(client, &partial_token, chunk.to_vec()).await;
                emitted = emitted.saturating_add(chunk.len());
                let percentage = ((emitted as f64 / total as f64) * 100.0).round() as u32;
                send_work_done_report(
                    client,
                    &work_done_token,
                    Some(format!("Symbols: {emitted}/{total}")),
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
        Some(format!("Found {count} symbol(s)")),
    )
    .await;
    if streamed_partials {
        result.map(|_| Vec::new())
    } else {
        result
    }
}
