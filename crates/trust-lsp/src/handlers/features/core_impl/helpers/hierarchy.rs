//! Responsibility: focused helper module in the core LSP feature layer.
use super::*;

pub(in super::super) fn is_code_lens_symbol(kind: &HirSymbolKind) -> bool {
    matches!(
        kind,
        HirSymbolKind::Program
            | HirSymbolKind::Function { .. }
            | HirSymbolKind::FunctionBlock
            | HirSymbolKind::Class
            | HirSymbolKind::Interface
            | HirSymbolKind::Method { .. }
            | HirSymbolKind::Property { .. }
    )
}

pub(in super::super) fn call_hierarchy_item_to_lsp(
    state: &ServerState,
    item: &trust_ide::CallHierarchyItem,
) -> Option<CallHierarchyItem> {
    let (uri, content) = file_info_for_file_id(state, item.file_id)?;
    let range = Range {
        start: offset_to_position(&content, item.range.start().into()),
        end: offset_to_position(&content, item.range.end().into()),
    };
    let selection_range = Range {
        start: offset_to_position(&content, item.selection_range.start().into()),
        end: offset_to_position(&content, item.selection_range.end().into()),
    };
    let kind = call_hierarchy_symbol_kind(&item.kind);

    Some(CallHierarchyItem {
        name: item.name.to_string(),
        kind,
        tags: None,
        detail: None,
        uri,
        range,
        selection_range,
        data: Some(json!({
            "fileId": item.file_id.0,
            "symbolId": item.symbol_id.0,
        })),
    })
}

pub(in super::super) fn call_hierarchy_item_from_lsp(
    state: &ServerState,
    item: &CallHierarchyItem,
) -> Option<trust_ide::CallHierarchyItem> {
    if let Some(serde_json::Value::Object(map)) = &item.data {
        let file_id = map
            .get("fileId")
            .and_then(|value| value.as_u64())
            .map(|value| trust_hir::db::FileId(value as u32));
        let symbol_id = map
            .get("symbolId")
            .and_then(|value| value.as_u64())
            .map(|value| trust_hir::symbols::SymbolId(value as u32));
        if let (Some(file_id), Some(symbol_id)) = (file_id, symbol_id) {
            return state.with_database(|db| {
                let symbols = db.file_symbols(file_id);
                let symbol = symbols.get(symbol_id)?;
                Some(trust_ide::CallHierarchyItem {
                    name: symbol.name.clone(),
                    kind: symbol.kind.clone(),
                    file_id,
                    range: symbol.range,
                    selection_range: symbol.range,
                    symbol_id,
                })
            });
        }
    }

    let doc = state.get_document(&item.uri)?;
    let offset = position_to_offset(&doc.content, item.selection_range.start)?;
    let allowed_files = call_hierarchy_allowed_files(state, &item.uri);
    state.with_database(|db| {
        trust_ide::prepare_call_hierarchy_in_files(
            db,
            doc.file_id,
            TextSize::from(offset),
            allowed_files.as_ref(),
        )
    })
}

pub(in super::super) fn call_hierarchy_allowed_files(
    state: &ServerState,
    uri: &Url,
) -> Option<FxHashSet<trust_hir::db::FileId>> {
    let config = state.workspace_config_for_uri(uri)?;
    let files = state.file_ids_for_config(&config);
    if files.is_empty() {
        None
    } else {
        Some(files)
    }
}

pub(in super::super) fn file_info_for_file_id(
    state: &ServerState,
    file_id: trust_hir::db::FileId,
) -> Option<(Url, String)> {
    if let Some(doc) = state.document_for_file_id(file_id) {
        return Some((doc.uri, doc.content));
    }
    let uri = state.uri_for_file_id(file_id)?;
    let content = state.with_database(|db| db.source_text(file_id).as_ref().clone());
    Some((uri, content))
}

pub(in super::super) fn type_hierarchy_item_to_lsp(
    state: &ServerState,
    item: &trust_ide::TypeHierarchyItem,
) -> Option<TypeHierarchyItem> {
    let doc = state.document_for_file_id(item.file_id)?;
    let range = Range {
        start: offset_to_position(&doc.content, item.range.start().into()),
        end: offset_to_position(&doc.content, item.range.end().into()),
    };
    let selection_range = Range {
        start: offset_to_position(&doc.content, item.selection_range.start().into()),
        end: offset_to_position(&doc.content, item.selection_range.end().into()),
    };
    let kind = call_hierarchy_symbol_kind(&item.kind);

    Some(TypeHierarchyItem {
        name: item.name.to_string(),
        kind,
        tags: None,
        detail: None,
        uri: doc.uri,
        range,
        selection_range,
        data: None,
    })
}

pub(in super::super) fn type_hierarchy_item_from_lsp(
    state: &ServerState,
    item: &TypeHierarchyItem,
) -> Option<trust_ide::TypeHierarchyItem> {
    let doc = state.get_document(&item.uri)?;
    let offset = position_to_offset(&doc.content, item.selection_range.start)?;
    state.with_database(|db| {
        trust_ide::prepare_type_hierarchy(db, doc.file_id, TextSize::from(offset))
    })
}

pub(in super::super) fn call_hierarchy_symbol_kind(kind: &HirSymbolKind) -> SymbolKind {
    match kind {
        HirSymbolKind::Program => SymbolKind::MODULE,
        HirSymbolKind::Configuration => SymbolKind::MODULE,
        HirSymbolKind::Resource => SymbolKind::NAMESPACE,
        HirSymbolKind::Task => SymbolKind::EVENT,
        HirSymbolKind::ProgramInstance => SymbolKind::OBJECT,
        HirSymbolKind::Namespace => SymbolKind::NAMESPACE,
        HirSymbolKind::Function { .. } => SymbolKind::FUNCTION,
        HirSymbolKind::FunctionBlock => SymbolKind::CLASS,
        HirSymbolKind::Class => SymbolKind::CLASS,
        HirSymbolKind::Method { .. } => SymbolKind::METHOD,
        HirSymbolKind::Property { .. } => SymbolKind::PROPERTY,
        HirSymbolKind::Interface => SymbolKind::INTERFACE,
        HirSymbolKind::Type => SymbolKind::STRUCT,
        HirSymbolKind::EnumValue { .. } => SymbolKind::ENUM_MEMBER,
        HirSymbolKind::Variable { .. } => SymbolKind::VARIABLE,
        HirSymbolKind::Constant => SymbolKind::CONSTANT,
        HirSymbolKind::Parameter { .. } => SymbolKind::VARIABLE,
        HirSymbolKind::Action => SymbolKind::FUNCTION,
    }
}
