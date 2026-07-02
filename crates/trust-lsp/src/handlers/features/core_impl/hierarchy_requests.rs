use super::*;

pub fn code_lens(state: &ServerState, params: CodeLensParams) -> Option<Vec<CodeLens>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    struct CodeLensData {
        range: TextRange,
        references: Vec<trust_ide::Reference>,
    }

    let entries = state.with_database(|db| {
        let symbols = db.file_symbols(doc.file_id);
        symbols
            .iter()
            .filter(|symbol| {
                is_code_lens_symbol(&symbol.kind)
                    && symbol.origin.is_none()
                    && !symbol.range.is_empty()
            })
            .map(|symbol| {
                let references = trust_ide::find_references(
                    db,
                    doc.file_id,
                    symbol.range.start(),
                    trust_ide::FindReferencesOptions {
                        include_declaration: false,
                    },
                );
                CodeLensData {
                    range: symbol.range,
                    references,
                }
            })
            .collect::<Vec<_>>()
    });

    let mut lenses = Vec::new();
    for entry in entries {
        let range = Range {
            start: offset_to_position(&doc.content, entry.range.start().into()),
            end: offset_to_position(&doc.content, entry.range.end().into()),
        };

        let mut locations = Vec::new();
        for reference in entry.references {
            let Some(target_doc) = state.document_for_file_id(reference.file_id) else {
                continue;
            };
            let start = offset_to_position(&target_doc.content, reference.range.start().into());
            let end = offset_to_position(&target_doc.content, reference.range.end().into());
            locations.push(Location {
                uri: target_doc.uri,
                range: Range { start, end },
            });
        }
        if locations.is_empty() {
            continue;
        }

        let title = format!("References: {}", locations.len());
        let position = offset_to_position(&doc.content, entry.range.start().into());

        let command = Command {
            title,
            command: "editor.action.showReferences".to_string(),
            arguments: Some(vec![json!(doc.uri), json!(position), json!(locations)]),
        };

        lenses.push(CodeLens {
            range,
            command: Some(command),
            data: None,
        });
    }

    Some(lenses)
}

pub fn prepare_call_hierarchy(
    state: &ServerState,
    params: CallHierarchyPrepareParams,
) -> Option<Vec<CallHierarchyItem>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;
    let allowed_files = call_hierarchy_allowed_files(state, uri);

    let item = state.with_database(|db| {
        trust_ide::prepare_call_hierarchy_in_files(
            db,
            doc.file_id,
            TextSize::from(offset),
            allowed_files.as_ref(),
        )
    })?;

    let lsp_item = call_hierarchy_item_to_lsp(state, &item)?;
    Some(vec![lsp_item])
}

pub fn incoming_calls(
    state: &ServerState,
    params: CallHierarchyIncomingCallsParams,
) -> Option<Vec<CallHierarchyIncomingCall>> {
    let item = call_hierarchy_item_from_lsp(state, &params.item)?;
    let allowed_files = call_hierarchy_allowed_files(state, &params.item.uri);
    let incoming = state
        .with_database(|db| trust_ide::incoming_calls_in_files(db, &item, allowed_files.as_ref()));

    let mut result = Vec::new();
    for call in incoming {
        let from_item = call_hierarchy_item_to_lsp(state, &call.from)?;
        let (_from_uri, from_content) = file_info_for_file_id(state, call.from.file_id)?;
        let from_ranges = call
            .from_ranges
            .into_iter()
            .map(|range| Range {
                start: offset_to_position(&from_content, range.start().into()),
                end: offset_to_position(&from_content, range.end().into()),
            })
            .collect();
        result.push(CallHierarchyIncomingCall {
            from: from_item,
            from_ranges,
        });
    }

    Some(result)
}

pub fn outgoing_calls(
    state: &ServerState,
    params: CallHierarchyOutgoingCallsParams,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
    let item = call_hierarchy_item_from_lsp(state, &params.item)?;
    let (_caller_uri, caller_content) = file_info_for_file_id(state, item.file_id)?;
    let allowed_files = call_hierarchy_allowed_files(state, &params.item.uri);
    let outgoing = state
        .with_database(|db| trust_ide::outgoing_calls_in_files(db, &item, allowed_files.as_ref()));

    let mut result = Vec::new();
    for call in outgoing {
        let to_item = call_hierarchy_item_to_lsp(state, &call.to)?;
        let from_ranges = call
            .from_ranges
            .into_iter()
            .map(|range| Range {
                start: offset_to_position(&caller_content, range.start().into()),
                end: offset_to_position(&caller_content, range.end().into()),
            })
            .collect();
        result.push(CallHierarchyOutgoingCall {
            to: to_item,
            from_ranges,
        });
    }

    Some(result)
}

pub fn prepare_type_hierarchy(
    state: &ServerState,
    params: TypeHierarchyPrepareParams,
) -> Option<Vec<TypeHierarchyItem>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let item = state.with_database(|db| {
        trust_ide::prepare_type_hierarchy(db, doc.file_id, TextSize::from(offset))
    })?;

    let lsp_item = type_hierarchy_item_to_lsp(state, &item)?;
    Some(vec![lsp_item])
}

pub fn type_hierarchy_supertypes(
    state: &ServerState,
    params: TypeHierarchySupertypesParams,
) -> Option<Vec<TypeHierarchyItem>> {
    let item = type_hierarchy_item_from_lsp(state, &params.item)?;
    let supertypes = state.with_database(|db| trust_ide::supertypes(db, &item));
    let mut result = Vec::new();
    for supertype in supertypes {
        let lsp_item = type_hierarchy_item_to_lsp(state, &supertype)?;
        result.push(lsp_item);
    }
    Some(result)
}

pub fn type_hierarchy_subtypes(
    state: &ServerState,
    params: TypeHierarchySubtypesParams,
) -> Option<Vec<TypeHierarchyItem>> {
    let item = type_hierarchy_item_from_lsp(state, &params.item)?;
    let subtypes = state.with_database(|db| trust_ide::subtypes(db, &item));
    let mut result = Vec::new();
    for subtype in subtypes {
        let lsp_item = type_hierarchy_item_to_lsp(state, &subtype)?;
        result.push(lsp_item);
    }
    Some(result)
}
