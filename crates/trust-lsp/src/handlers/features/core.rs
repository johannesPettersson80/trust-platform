//! LSP language feature handlers.

use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::json;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use smol_str::SmolStr;
use text_size::{TextRange, TextSize};
use trust_hir::db::{SemanticDatabase, SourceDatabase};
use trust_hir::symbols::{ParamDirection, ScopeId, SymbolKind as HirSymbolKind, SymbolTable};
use trust_hir::TypeId;
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::config::{find_config_file, WorkspaceVisibility, CONFIG_FILES};
use crate::external_diagnostics::ExternalFixData;
use crate::handlers::diagnostics::collect_diagnostics_with_ticket;
use crate::library_docs::doc_for_name;
use crate::state::{path_to_uri, uri_to_path, ServerState};
use tracing::{debug, warn};
use trust_ide::goto_def::goto_definition as ide_goto_definition;
use trust_ide::util::scope_at_position;
use trust_ide::{
    call_signature_info, convert_function_block_to_function, convert_function_to_function_block,
    extract_method, extract_pou, extract_property, inline_value_data, InlineTargetKind,
    InlineValueScope, StdlibFilter,
};

use super::super::config::{bool_with_aliases, lsp_runtime_section, string_with_aliases};
use super::super::lsp_utils::{
    display_symbol_name, is_primary_pou_symbol_kind, lsp_symbol_kind, offset_to_line_col,
    offset_to_position, position_to_offset, rename_result_to_changes, semantic_tokens_to_lsp,
    st_file_stem, symbol_container_name, text_document_identifier_for_edit,
};
use super::super::progress::{
    send_partial_result, send_work_done_begin, send_work_done_end, send_work_done_report,
};
use super::super::runtime_values::{fetch_runtime_inline_values, RuntimeInlineValues};

const PARTIAL_CHUNK_SIZE: usize = 200;

fn runtime_inline_values_enabled(state: &ServerState) -> bool {
    let value = state.config();
    let Some(runtime) = lsp_runtime_section(&value) else {
        return true;
    };
    bool_with_aliases(runtime, &["inlineValuesEnabled", "inline_values_enabled"]).unwrap_or(true)
}

fn runtime_control_override(state: &ServerState) -> (Option<String>, Option<String>) {
    let value = state.config();
    let runtime = match lsp_runtime_section(&value) {
        Some(runtime) => runtime,
        None => return (None, None),
    };
    let control_enabled = bool_with_aliases(
        runtime,
        &["controlEndpointEnabled", "control_endpoint_enabled"],
    )
    .unwrap_or(true);
    if !control_enabled {
        return (None, None);
    }
    let endpoint = string_with_aliases(runtime, &["controlEndpoint", "control_endpoint"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let auth = string_with_aliases(runtime, &["controlAuthToken", "control_auth_token"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    (endpoint, auth)
}

fn stdlib_filter_for_uri(state: &ServerState, uri: &Url) -> StdlibFilter {
    if let Some(config) = state.workspace_config_for_uri(uri) {
        if let Some(allow) = config.stdlib.allow {
            return StdlibFilter::with_allowlists(Some(allow.clone()), Some(allow));
        }
        if let Some(profile) = config.stdlib.profile.as_deref() {
            if profile.trim().eq_ignore_ascii_case("full") {
                // Defer to vendor defaults when profile is the implicit full setting.
            } else {
                return StdlibFilter::from_profile(profile);
            }
        }
        if let Some(profile) = stdlib_profile_for_vendor(config.vendor_profile.as_deref()) {
            return StdlibFilter::from_profile(profile);
        }
    }
    StdlibFilter::allow_all()
}

fn stdlib_profile_for_vendor(profile: Option<&str>) -> Option<&'static str> {
    let profile = profile?.trim().to_ascii_lowercase();
    match profile.as_str() {
        "codesys" | "beckhoff" | "twincat" | "siemens" => Some("iec"),
        _ => None,
    }
}

pub fn hover(state: &ServerState, params: HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;
    let stdlib_filter = stdlib_filter_for_uri(state, uri);

    let mut result = state.with_database(|db| {
        trust_ide::hover_with_filter(db, doc.file_id, TextSize::from(offset), &stdlib_filter)
    })?;

    if let Some(docs) = state.library_docs_for_uri(uri) {
        if !docs.is_empty() {
            let symbol_name = state.with_database(|db| {
                trust_ide::symbol_name_at_position(db, doc.file_id, TextSize::from(offset))
            });
            if let Some(name) = symbol_name {
                if let Some(extra) = doc_for_name(docs.as_ref(), name.as_str()) {
                    if !result.contents.contains(extra) {
                        result.contents.push_str("\n\n---\n\n");
                        result.contents.push_str(extra);
                    }
                }
            }
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

    if let Some(docs) = state.library_docs_for_uri(uri) {
        if !docs.is_empty() {
            for item in &mut lsp_items {
                if let Some(extra) = doc_for_name(docs.as_ref(), &item.label) {
                    append_completion_doc(item, extra);
                }
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
    if item.documentation.is_none() && item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
        if let Some(insert_text) = item.insert_text.as_deref() {
            let snippet = insert_text.replace('\t', "    ");
            let value = format!("```st\n{}\n```", snippet);
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }));
        }
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

    if let Some(locations) = result.as_ref() {
        if partial_token.is_some() {
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
    result
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
        trust_ide::find_references(
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

    if let Some(symbols) = result.as_ref() {
        if partial_token.is_some() {
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
    result
}

pub fn code_action(state: &ServerState, params: CodeActionParams) -> Option<CodeActionResponse> {
    let request_ticket = state.begin_semantic_request();
    code_action_with_ticket(state, params, request_ticket)
}

fn code_action_with_ticket(
    state: &ServerState,
    params: CodeActionParams,
    request_ticket: u64,
) -> Option<CodeActionResponse> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    let parsed = parse(&doc.content);
    let root = parsed.syntax();
    let mut actions = Vec::new();
    let target_range = params.range;
    let mut diagnostics = params.context.diagnostics.clone();
    let collected = collect_diagnostics_with_ticket(
        state,
        uri,
        &doc.content,
        doc.file_id,
        Some(request_ticket),
    );
    diagnostics.extend(collected);
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    let mut seen = FxHashSet::default();
    diagnostics.retain(|diag| {
        let code = diagnostic_code(diag).unwrap_or_default();
        let key = (
            code,
            diag.range.start.line,
            diag.range.start.character,
            diag.range.end.line,
            diag.range.end.character,
            diag.message.clone(),
        );
        seen.insert(key)
    });
    diagnostics.retain(|diag| ranges_intersect(diag.range, target_range));

    for diagnostic in &diagnostics {
        if state.semantic_request_cancelled(request_ticket) {
            return None;
        }
        if let Some(edit) = external_fix_text_edit(diagnostic) {
            let title = external_fix_title(diagnostic);
            push_quickfix_action(&mut actions, &title, diagnostic, uri, edit);
            continue;
        }
        let code = diagnostic_code(diagnostic);
        match code.as_deref() {
            Some("W001") | Some("W002") => {
                let title = if code.as_deref() == Some("W001") {
                    "Remove unused variable"
                } else {
                    "Remove unused parameter"
                };
                let start = match position_to_offset(&doc.content, diagnostic.range.start) {
                    Some(offset) => offset,
                    None => continue,
                };
                let end = match position_to_offset(&doc.content, diagnostic.range.end) {
                    Some(offset) => offset,
                    None => continue,
                };
                let symbol_range = TextRange::new(TextSize::from(start), TextSize::from(end));
                let removal_range =
                    match unused_symbol_removal_range(&doc.content, &root, symbol_range) {
                        Some(range) => range,
                        None => continue,
                    };

                let edit = TextEdit {
                    range: Range {
                        start: offset_to_position(&doc.content, removal_range.start().into()),
                        end: offset_to_position(&doc.content, removal_range.end().into()),
                    },
                    new_text: String::new(),
                };

                let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
                    std::collections::HashMap::new();
                changes.insert(uri.clone(), vec![edit]);

                let action = CodeAction {
                    title: title.to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnostic.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    is_preferred: Some(true),
                    ..Default::default()
                };

                actions.push(CodeActionOrCommand::CodeAction(action));
            }
            Some("E101") => {
                if let Some(edit) = missing_var_text_edit(state, &doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Create VAR declaration",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
            }
            Some("E102") => {
                if let Some(edit) = missing_type_text_edit(&doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Create TYPE definition",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
            }
            Some("E002") | Some("E003") => {
                if let Some(edit) = missing_end_text_edit(&doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Insert missing END_*",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
            }
            Some("E205") => {
                if let Some(edit) = fix_output_binding_text_edit(&doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Fix output binding operator",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
                if let Some(edits) = convert_call_style_text_edit(state, &doc, &root, diagnostic) {
                    for (title, edit) in edits {
                        push_quickfix_action(&mut actions, &title, diagnostic, uri, edit);
                    }
                }
            }
            Some("E105") => {
                let namespace_actions =
                    namespace_disambiguation_actions(state, &doc, &root, diagnostic);
                actions.extend(namespace_actions);
            }
            Some("E206") => {
                if let Some(edit) = missing_return_text_edit(state, &doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Insert missing RETURN",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
            }
            Some("W004") => {
                let edit = match missing_else_text_edit(&doc.content, &root, diagnostic.range) {
                    Some(edit) => edit,
                    None => continue,
                };

                let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
                    std::collections::HashMap::new();
                changes.insert(uri.clone(), vec![edit]);

                let action = CodeAction {
                    title: "Insert missing ELSE branch".to_string(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnostic.clone()]),
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    is_preferred: Some(true),
                    ..Default::default()
                };

                actions.push(CodeActionOrCommand::CodeAction(action));
            }
            Some("W005") | Some("E203") => {
                if let Some(edit) = implicit_conversion_text_edit(&doc, &root, diagnostic) {
                    push_quickfix_action(
                        &mut actions,
                        "Wrap with conversion function",
                        diagnostic,
                        uri,
                        edit,
                    );
                }
            }
            _ => continue,
        }
    }

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    if let Some(action) = interface_stub_action(state, &doc, &params) {
        actions.push(action);
    }

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    if let Some(action) = inline_symbol_action(state, &doc, &params) {
        actions.push(action);
    }

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    actions.extend(extract_actions(state, &doc, &params));

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    if let Some(action) = convert_function_action(state, &doc, &params) {
        actions.push(action);
    }

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    if let Some(action) = convert_function_block_action(state, &doc, &params) {
        actions.push(action);
    }

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }
    if let Some(action) = namespace_move_action(&doc, &root, &params) {
        actions.push(action);
    }

    Some(actions)
}

pub fn rename(state: &ServerState, params: RenameParams) -> Option<WorkspaceEdit> {
    let request_ticket = state.begin_semantic_request();
    rename_with_ticket(state, params, request_ticket)
}

fn rename_with_ticket(
    state: &ServerState,
    params: RenameParams,
    request_ticket: u64,
) -> Option<WorkspaceEdit> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = &params.new_name;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let result = state
        .with_database(|db| trust_ide::rename(db, doc.file_id, TextSize::from(offset), new_name))?;

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let file_rename = maybe_rename_pou_file(state, doc.file_id, TextSize::from(offset), new_name);

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let changes = rename_result_to_changes(state, result)?;

    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    if let Some(rename_op) = file_rename {
        let mut document_changes = changes_to_document_operations(state, changes);
        document_changes.push(DocumentChangeOperation::Op(ResourceOp::Rename(rename_op)));
        return Some(WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Operations(document_changes)),
            change_annotations: None,
        });
    }

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn changes_to_document_operations(
    state: &ServerState,
    changes: std::collections::HashMap<Url, Vec<TextEdit>>,
) -> Vec<DocumentChangeOperation> {
    let mut entries: Vec<_> = changes.into_iter().collect();
    entries.sort_by_key(|(uri, _)| uri.to_string());

    let mut operations = Vec::new();
    for (uri, edits) in entries {
        let text_document = text_document_identifier_for_edit(state, &uri);
        let text_edits = edits.into_iter().map(OneOf::Left).collect();
        operations.push(DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document,
            edits: text_edits,
        }));
    }
    operations
}

fn maybe_rename_pou_file(
    state: &ServerState,
    file_id: trust_hir::db::FileId,
    position: TextSize,
    new_name: &str,
) -> Option<RenameFile> {
    if new_name.contains('.') {
        return None;
    }

    let (pou_name, definition_file_id) = state.with_database(|db| {
        let definition = ide_goto_definition(db, file_id, position)?;
        let symbols = db.file_symbols(definition.file_id);
        let mut candidate = None;
        for symbol in symbols.iter() {
            if symbol.origin.is_some() || symbol.range.is_empty() {
                continue;
            }
            if !is_primary_pou_symbol_kind(&symbol.kind) {
                continue;
            }
            if candidate.is_some() {
                return None;
            }
            candidate = Some(symbol);
        }
        let symbol = candidate?;
        if symbol.range != definition.range {
            return None;
        }
        Some((symbol.name.to_string(), definition.file_id))
    })?;

    let doc = state.document_for_file_id(definition_file_id)?;
    let old_uri = doc.uri.clone();
    let old_stem = st_file_stem(&old_uri)?;
    if !pou_name.eq_ignore_ascii_case(&old_stem) {
        return None;
    }

    if !trust_hir::is_valid_identifier(new_name) || trust_hir::is_reserved_keyword(new_name) {
        return None;
    }

    if new_name.eq_ignore_ascii_case(&old_stem) {
        return None;
    }

    let old_path = uri_to_path(&old_uri);
    let extension = old_path
        .as_ref()
        .and_then(|path| path.extension().and_then(|ext| ext.to_str()))
        .or_else(|| {
            Path::new(old_uri.path())
                .extension()
                .and_then(|ext| ext.to_str())
        })?;
    let file_name = format!("{new_name}.{extension}");

    if let Some(path) = old_path.as_ref() {
        let new_path = path.with_file_name(&file_name);
        if &new_path == path || new_path.exists() {
            return None;
        }
        if let Some(new_uri) = path_to_uri(&new_path) {
            return Some(RenameFile {
                old_uri,
                new_uri,
                options: Some(RenameFileOptions {
                    overwrite: Some(false),
                    ignore_if_exists: Some(true),
                }),
                annotation_id: None,
            });
        }
    }

    let mut new_uri = old_uri.clone();
    {
        let mut segments = new_uri.path_segments_mut().ok()?;
        segments.pop_if_empty();
        segments.pop();
        segments.push(&file_name);
    }
    Some(RenameFile {
        old_uri,
        new_uri,
        options: Some(RenameFileOptions {
            overwrite: Some(false),
            ignore_if_exists: Some(true),
        }),
        annotation_id: None,
    })
}

pub fn prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> Option<PrepareRenameResponse> {
    let uri = &params.text_document.uri;
    let position = params.position;

    let doc = state.get_document(uri)?;
    let offset = position_to_offset(&doc.content, position)?;

    let range = state.with_database(|db| {
        trust_ide::rename::prepare_rename(db, doc.file_id, TextSize::from(offset))
    })?;

    Some(PrepareRenameResponse::Range(Range {
        start: offset_to_position(&doc.content, range.start().into()),
        end: offset_to_position(&doc.content, range.end().into()),
    }))
}

pub fn semantic_tokens_full(
    state: &ServerState,
    params: SemanticTokensParams,
) -> Option<SemanticTokensResult> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    // Get semantic tokens from trust_ide
    let tokens = state.with_database(|db| trust_ide::semantic_tokens(db, doc.file_id));

    let data = semantic_tokens_to_lsp(&doc.content, tokens, 0, 0);
    let result_id = state.store_semantic_tokens(uri.clone(), data.clone());

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: Some(result_id),
        data,
    }))
}

pub fn semantic_tokens_full_delta(
    state: &ServerState,
    params: SemanticTokensDeltaParams,
) -> Option<SemanticTokensFullDeltaResult> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let tokens = state.with_database(|db| trust_ide::semantic_tokens(db, doc.file_id));
    let data = semantic_tokens_to_lsp(&doc.content, tokens, 0, 0);

    let previous = state.semantic_tokens_cache(uri);
    let result_id = state.store_semantic_tokens(uri.clone(), data.clone());

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

pub fn document_link(state: &ServerState, params: DocumentLinkParams) -> Option<Vec<DocumentLink>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let mut links = Vec::new();
    let config_root = config_root_for_uri(state, uri)
        .or_else(|| uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf)));

    if let Some(path) = uri_to_path(uri) {
        if is_config_file(&path) {
            let root = config_root
                .clone()
                .or_else(|| path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| path.clone());
            links.extend(document_links_for_config_paths(&doc.content, &root));
        } else if is_st_file(&path) {
            links.extend(document_links_for_using(state, &doc));
        }
    }

    if let Some(root) = config_root {
        links.extend(document_links_for_config_mentions(&doc.content, &root));
    }

    Some(links)
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

pub fn inline_value(state: &ServerState, params: InlineValueParams) -> Option<Vec<InlineValue>> {
    let uri = &params.text_document.uri;
    let doc = state.get_document(uri)?;

    let start_offset = position_to_offset(&doc.content, params.range.start)?;
    let end_offset = position_to_offset(&doc.content, params.range.end)?;
    if end_offset < start_offset {
        return Some(Vec::new());
    }

    if !runtime_inline_values_enabled(state) {
        debug!("inlineValue skipped: disabled via settings");
        return Some(Vec::new());
    }

    let data = state.with_database(|db| {
        inline_value_data(
            db,
            doc.file_id,
            TextRange::new(TextSize::from(start_offset), TextSize::from(end_offset)),
        )
    });
    debug!(
        "inlineValue request uri={} frame_id={} range=({},{})->({},{}) targets={} hints={}",
        uri,
        params.context.frame_id,
        params.range.start.line,
        params.range.start.character,
        params.range.end.line,
        params.range.end.character,
        data.targets.len(),
        data.hints.len()
    );

    let mut values = Vec::new();
    let mut seen = FxHashSet::default();

    for hint in data.hints {
        seen.insert(hint.range);
        values.push(InlineValue::Text(InlineValueText {
            range: text_range_to_lsp(&doc.content, hint.range),
            text: hint.text,
        }));
    }

    let frame_id = u32::try_from(params.context.frame_id).ok();
    let mut owner_hints = Vec::new();
    for target in &data.targets {
        if let Some(owner) = target.owner.as_ref() {
            if !owner_hints
                .iter()
                .any(|name: &SmolStr| name.eq_ignore_ascii_case(owner))
            {
                owner_hints.push(owner.clone());
            }
        }
    }
    let (override_endpoint, override_auth) = runtime_control_override(state);
    let config = state.workspace_config_for_uri(uri);
    let endpoint = config
        .as_ref()
        .and_then(|config| config.runtime.control_endpoint.as_deref())
        .or(override_endpoint.as_deref());
    let auth = config
        .as_ref()
        .and_then(|config| config.runtime.control_auth_token.as_deref())
        .or(override_auth.as_deref());
    if let (Some(frame_id), Some(endpoint)) = (frame_id, endpoint) {
        debug!(
            "inlineValue runtime fetch uri={} endpoint={} auth_present={} owner_hints={}",
            uri,
            endpoint,
            auth.is_some(),
            owner_hints.len()
        );
        if let Some(runtime_values) =
            fetch_runtime_inline_values(endpoint, auth, frame_id, &owner_hints)
        {
            debug!(
                "inlineValue runtime values locals={} globals={} retain={}",
                runtime_values.locals.len(),
                runtime_values.globals.len(),
                runtime_values.retain.len()
            );
            let normalized_values = NormalizedInlineValues::new(&runtime_values);
            for target in data.targets {
                if seen.contains(&target.range) {
                    continue;
                }
                let value = normalized_values.lookup(target.scope, &target.name);
                if let Some(value) = value {
                    seen.insert(target.range);
                    values.push(InlineValue::Text(InlineValueText {
                        range: text_range_to_lsp(&doc.content, target.range),
                        text: format!(" = {value}"),
                    }));
                }
            }
        }
    } else if frame_id.is_none() {
        warn!(
            "inlineValue skipped: invalid frame_id={} for uri={}",
            params.context.frame_id, uri
        );
    } else {
        warn!(
            "inlineValue skipped: missing runtime control endpoint for uri={}",
            uri
        );
    }

    Some(values)
}

struct NormalizedInlineValues {
    locals: FxHashMap<SmolStr, String>,
    globals: FxHashMap<SmolStr, String>,
    retain: FxHashMap<SmolStr, String>,
}

impl NormalizedInlineValues {
    fn new(values: &RuntimeInlineValues) -> Self {
        Self {
            locals: normalize_inline_values(&values.locals),
            globals: normalize_inline_values(&values.globals),
            retain: normalize_inline_values(&values.retain),
        }
    }

    fn lookup(&self, scope: InlineValueScope, name: &SmolStr) -> Option<&String> {
        match scope {
            InlineValueScope::Local => lookup_inline_value(&self.locals, name)
                .or_else(|| lookup_inline_value(&self.globals, name))
                .or_else(|| lookup_inline_value(&self.retain, name)),
            InlineValueScope::Global => lookup_inline_value(&self.globals, name),
            InlineValueScope::Retain => lookup_inline_value(&self.retain, name),
        }
    }
}

fn normalize_inline_values(values: &FxHashMap<SmolStr, String>) -> FxHashMap<SmolStr, String> {
    let mut out = FxHashMap::default();
    for (name, value) in values {
        out.insert(normalize_inline_name(name), value.clone());
    }
    out
}

fn lookup_inline_value<'a>(
    values: &'a FxHashMap<SmolStr, String>,
    name: &SmolStr,
) -> Option<&'a String> {
    values
        .get(name)
        .or_else(|| values.get(&normalize_inline_name(name)))
}

fn normalize_inline_name(name: &SmolStr) -> SmolStr {
    SmolStr::new(name.as_str().to_ascii_uppercase())
}

fn document_links_for_using(
    state: &ServerState,
    doc: &crate::state::Document,
) -> Vec<DocumentLink> {
    let entries = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        let mut entries = Vec::new();
        for scope in symbols.scopes() {
            for using in &scope.using_directives {
                if using.range.is_empty() || using.path.is_empty() {
                    continue;
                }
                let Some(symbol_id) = symbols.resolve_qualified(&using.path) else {
                    continue;
                };
                let Some(symbol) = symbols.get(symbol_id) else {
                    continue;
                };
                let file_id = symbol
                    .origin
                    .map(|origin| origin.file_id)
                    .unwrap_or(doc.file_id);
                entries.push((using.range, file_id));
            }
        }
        entries
    });

    let mut links = Vec::new();
    for (range, file_id) in entries {
        let Some(target_doc) = state.document_for_file_id(file_id) else {
            continue;
        };
        links.push(DocumentLink {
            range: text_range_to_lsp(&doc.content, range),
            target: Some(target_doc.uri),
            tooltip: Some("Open namespace definition".to_string()),
            data: None,
        });
    }
    links
}

fn document_links_for_config_paths(source: &str, root: &Path) -> Vec<DocumentLink> {
    let mut links = Vec::new();
    let mut in_library_block = false;
    let mut offset = 0usize;

    for line in source.split_inclusive('\n') {
        let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
        let line_text = line_no_newline
            .strip_suffix('\r')
            .unwrap_or(line_no_newline);
        let trimmed = line_text.trim();

        if trimmed.starts_with('[') {
            in_library_block = trimmed == "[[libraries]]";
        }

        if let Some((key, value)) = line_text.split_once('=') {
            let key = key.trim();
            let value_start = line_text.find(value).unwrap_or(key.len() + 1);
            let should_scan = key == "include_paths"
                || key == "library_paths"
                || (in_library_block && key == "path");
            if should_scan {
                for (start, end, text) in extract_string_literals(value) {
                    let value_start_offset = offset + value_start;
                    let abs_start = value_start_offset + start;
                    let abs_end = value_start_offset + end;
                    let target =
                        resolve_config_path(root, &text).and_then(|path| path_to_uri(&path));
                    let Some(target) = target else {
                        continue;
                    };
                    links.push(DocumentLink {
                        range: Range {
                            start: offset_to_position(source, abs_start as u32),
                            end: offset_to_position(source, abs_end as u32),
                        },
                        target: Some(target),
                        tooltip: Some("Open config path".to_string()),
                        data: None,
                    });
                }
            }
        }

        offset = offset.saturating_add(line.len());
    }

    links
}

fn document_links_for_config_mentions(source: &str, root: &Path) -> Vec<DocumentLink> {
    let Some(config_path) = find_config_file(root) else {
        return Vec::new();
    };
    let Some(target) = path_to_uri(&config_path) else {
        return Vec::new();
    };

    let mut links = Vec::new();
    for name in CONFIG_FILES {
        let mut search_start = 0usize;
        while let Some(pos) = source[search_start..].find(name) {
            let start = search_start + pos;
            let end = start + name.len();
            links.push(DocumentLink {
                range: Range {
                    start: offset_to_position(source, start as u32),
                    end: offset_to_position(source, end as u32),
                },
                target: Some(target.clone()),
                tooltip: Some("Open trust-lsp config".to_string()),
                data: None,
            });
            search_start = end;
        }
    }
    links
}

fn extract_string_literals(value: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    let mut in_string: Option<char> = None;
    let mut start = 0usize;
    let mut escaped = false;

    for (idx, ch) in value.char_indices() {
        if let Some(delim) = in_string {
            if delim == '"' && escaped {
                escaped = false;
                continue;
            }
            if delim == '"' && ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == delim {
                let content_start = start + 1;
                if idx >= content_start {
                    let text = value[content_start..idx].to_string();
                    results.push((content_start, idx, text));
                }
                in_string = None;
            }
        } else if ch == '"' || ch == '\'' {
            in_string = Some(ch);
            start = idx;
            escaped = false;
        }
    }

    results
}

fn resolve_config_path(root: &Path, entry: &str) -> Option<PathBuf> {
    if entry.is_empty() {
        return None;
    }
    let path = PathBuf::from(entry);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(root.join(path))
    }
}

fn config_root_for_uri(state: &ServerState, uri: &Url) -> Option<PathBuf> {
    state
        .workspace_config_for_uri(uri)
        .map(|config| config.root)
        .or_else(|| uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf)))
}

fn is_st_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "st" | "pou"))
        .unwrap_or(false)
}

fn is_config_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| CONFIG_FILES.iter().any(|candidate| candidate == &name))
        .unwrap_or(false)
}

fn append_completion_doc(item: &mut CompletionItem, extra: &str) {
    let extra = extra.trim();
    if extra.is_empty() {
        return;
    }
    let merged = match &item.documentation {
        Some(Documentation::MarkupContent(content)) => {
            if content.value.contains(extra) {
                return;
            }
            format!("{}\n\n---\n\n{}", content.value, extra)
        }
        Some(Documentation::String(text)) => {
            if text.contains(extra) {
                return;
            }
            format!("{text}\n\n---\n\n{extra}")
        }
        None => extra.to_string(),
    };
    item.documentation = Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: merged,
    }));
}

fn external_fix_text_edit(diagnostic: &Diagnostic) -> Option<TextEdit> {
    let fix = diagnostic_external_fix(diagnostic)?;
    let range = fix.range.unwrap_or(diagnostic.range);
    Some(TextEdit {
        range,
        new_text: fix.new_text,
    })
}

fn external_fix_title(diagnostic: &Diagnostic) -> String {
    diagnostic_external_fix(diagnostic)
        .and_then(|fix| fix.title)
        .unwrap_or_else(|| "Apply external fix".to_string())
}

fn diagnostic_external_fix(diagnostic: &Diagnostic) -> Option<ExternalFixData> {
    let value = diagnostic.data.as_ref()?;
    let map = value.as_object()?;
    let fix_value = map.get("externalFix")?.clone();
    serde_json::from_value(fix_value).ok()
}

fn text_range_to_lsp(source: &str, range: TextRange) -> Range {
    Range {
        start: offset_to_position(source, range.start().into()),
        end: offset_to_position(source, range.end().into()),
    }
}

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

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic.code.as_ref().map(|code| match code {
        NumberOrString::String(value) => value.clone(),
        NumberOrString::Number(value) => value.to_string(),
    })
}

fn is_code_lens_symbol(kind: &HirSymbolKind) -> bool {
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

fn call_hierarchy_item_to_lsp(
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

fn call_hierarchy_item_from_lsp(
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

fn call_hierarchy_allowed_files(
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

fn file_info_for_file_id(
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

fn type_hierarchy_item_to_lsp(
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

fn type_hierarchy_item_from_lsp(
    state: &ServerState,
    item: &TypeHierarchyItem,
) -> Option<trust_ide::TypeHierarchyItem> {
    let doc = state.get_document(&item.uri)?;
    let offset = position_to_offset(&doc.content, item.selection_range.start)?;
    state.with_database(|db| {
        trust_ide::prepare_type_hierarchy(db, doc.file_id, TextSize::from(offset))
    })
}

fn call_hierarchy_symbol_kind(kind: &HirSymbolKind) -> SymbolKind {
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
    }
}

fn unused_symbol_removal_range(
    source: &str,
    root: &SyntaxNode,
    symbol_range: TextRange,
) -> Option<TextRange> {
    let var_decl = find_var_decl_for_range(root, symbol_range)?;
    let names: Vec<SyntaxToken> = var_decl
        .children()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .filter_map(ident_token_in_name)
        .collect();
    if names.is_empty() {
        return None;
    }
    let index = names
        .iter()
        .position(|token| token.text_range() == symbol_range)?;

    if names.len() == 1 {
        return Some(extend_range_to_line_end(source, var_decl.text_range()));
    }

    if index + 1 < names.len() {
        let end = names[index + 1].text_range().start();
        return Some(TextRange::new(symbol_range.start(), end));
    }

    let tokens: Vec<SyntaxToken> = var_decl
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .collect();
    let target_index = tokens
        .iter()
        .position(|token| token.text_range() == symbol_range)?;
    let comma = tokens[..target_index]
        .iter()
        .rev()
        .find(|token| token.kind() == SyntaxKind::Comma)?;

    let mut end = var_decl.text_range().end();
    for token in tokens.iter().skip(target_index + 1) {
        if token.kind().is_trivia() {
            end = token.text_range().end();
            continue;
        }
        end = token.text_range().start();
        break;
    }

    Some(TextRange::new(comma.text_range().start(), end))
}

fn missing_else_text_edit(source: &str, root: &SyntaxNode, range: Range) -> Option<TextEdit> {
    let start = position_to_offset(source, range.start)?;
    let end = position_to_offset(source, range.end)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(end));
    let case_stmt = find_case_stmt_for_range(root, diag_range)?;

    if case_stmt
        .children()
        .any(|child| child.kind() == SyntaxKind::ElseBranch)
    {
        return None;
    }

    let end_case_token = case_stmt
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::KwEndCase)?;
    let end_case_offset = usize::from(end_case_token.text_range().start());
    let line_start = line_start_offset(source, end_case_offset);

    let indent = case_stmt
        .children()
        .find(|child| child.kind() == SyntaxKind::CaseBranch)
        .map(|branch| indent_at_offset(source, usize::from(branch.text_range().start())))
        .unwrap_or_else(|| indent_at_offset(source, end_case_offset));

    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let insert_text = format!("{indent}ELSE{newline}");
    let insert_pos = offset_to_position(source, line_start as u32);

    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

fn find_case_stmt_for_range(root: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::CaseStmt)
        .filter(|node| {
            let node_range = node.text_range();
            node_range.contains(range.start()) && node_range.contains(range.end())
        })
        .min_by_key(|node| node.text_range().len())
}

fn line_start_offset(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    match source[..offset].rfind('\n') {
        Some(pos) => pos + 1,
        None => 0,
    }
}

fn indent_at_offset(source: &str, offset: usize) -> String {
    let line_start = line_start_offset(source, offset);
    let bytes = source.as_bytes();
    let mut end = line_start;
    while end < bytes.len() {
        match bytes[end] {
            b' ' | b'\t' => end += 1,
            _ => break,
        }
    }
    source[line_start..end].to_string()
}

fn find_var_decl_for_range(root: &SyntaxNode, symbol_range: TextRange) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|var_decl| {
            var_decl
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(ident_token_in_name)
                .any(|ident| ident.text_range() == symbol_range)
        })
}

fn ident_token_in_name(node: SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
}

fn extend_range_to_line_end(source: &str, range: TextRange) -> TextRange {
    let mut end = usize::from(range.end());
    let bytes = source.as_bytes();
    while end < bytes.len() {
        match bytes[end] {
            b'\n' => {
                end += 1;
                break;
            }
            b'\r' => {
                end += 1;
                if end < bytes.len() && bytes[end] == b'\n' {
                    end += 1;
                }
                break;
            }
            _ => end += 1,
        }
    }
    TextRange::new(range.start(), TextSize::from(end as u32))
}

fn is_foldable_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::PropertyGet
            | SyntaxKind::PropertySet
            | SyntaxKind::Interface
            | SyntaxKind::Namespace
            | SyntaxKind::Action
            | SyntaxKind::TypeDecl
            | SyntaxKind::StructDef
            | SyntaxKind::UnionDef
            | SyntaxKind::EnumDef
            | SyntaxKind::VarBlock
            | SyntaxKind::StmtList
            | SyntaxKind::IfStmt
            | SyntaxKind::CaseStmt
            | SyntaxKind::CaseBranch
            | SyntaxKind::ForStmt
            | SyntaxKind::WhileStmt
            | SyntaxKind::RepeatStmt
    )
}

fn selection_range_to_lsp(source: &str, range: trust_ide::SelectionRange) -> SelectionRange {
    let parent = range
        .parent
        .map(|parent| Box::new(selection_range_to_lsp(source, *parent)));
    SelectionRange {
        range: Range {
            start: offset_to_position(source, range.range.start().into()),
            end: offset_to_position(source, range.range.end().into()),
        },
        parent,
    }
}

fn push_quickfix_action(
    actions: &mut Vec<CodeActionOrCommand>,
    title: &str,
    diagnostic: &Diagnostic,
    uri: &Url,
    edit: TextEdit,
) {
    let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
        std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);
    let action = CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        is_preferred: Some(true),
        ..Default::default()
    };
    actions.push(CodeActionOrCommand::CodeAction(action));
}

fn extract_quoted_name(message: &str) -> Option<String> {
    if let Some(start) = message.find('\'') {
        let rest = &message[start + 1..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }

    let lower = message.to_ascii_lowercase();
    const MARKERS: [&str; 7] = [
        "ambiguous reference to ",
        "undefined function ",
        "undefined variable ",
        "undefined identifier ",
        "undefined type ",
        "unknown type ",
        "cannot resolve namespace ",
    ];
    for marker in MARKERS {
        if let Some(idx) = lower.find(marker) {
            let rest = &message[idx + marker.len()..];
            let mut name = String::new();
            for ch in rest.chars() {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' {
                    name.push(ch);
                } else {
                    break;
                }
            }
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

fn newline_for_source(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn infer_indent_unit(source: &str) -> String {
    let mut min_spaces = usize::MAX;
    for line in source.lines() {
        if line.starts_with('\t') {
            return "\t".to_string();
        }
        let spaces = line.chars().take_while(|c| *c == ' ').count();
        if spaces > 0 {
            min_spaces = min_spaces.min(spaces);
        }
    }
    if min_spaces != usize::MAX {
        return " ".repeat(min_spaces);
    }
    "    ".to_string()
}

fn line_end_offset(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    match source[offset..].find('\n') {
        Some(pos) => offset + pos + 1,
        None => source.len(),
    }
}

fn position_leq(a: Position, b: Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character <= b.character)
}

fn ranges_intersect(a: Range, b: Range) -> bool {
    position_leq(a.start, b.end) && position_leq(b.start, a.end)
}

fn missing_var_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let name = extract_quoted_name(&diagnostic.message)?;
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let text_range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let pou = trust_ide::util::find_enclosing_pou(root, text_range.start())?;

    let type_name =
        infer_missing_var_type(state, doc, root, text_range).unwrap_or_else(|| "INT".to_string());
    let newline = newline_for_source(&doc.content);

    if let Some(var_block) = find_var_block(&pou) {
        let end_var_token = var_block
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::KwEndVar)?;
        let insert_offset = line_start_offset(
            &doc.content,
            usize::from(end_var_token.text_range().start()),
        );
        let decl_indent = indent_for_var_block(&doc.content, &var_block, &end_var_token)
            .unwrap_or_else(|| {
                let base = indent_at_offset(&doc.content, insert_offset);
                format!("{base}{}", infer_indent_unit(&doc.content))
            });
        let insert_text = format!("{decl_indent}{name} : {type_name};{newline}");
        let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
        return Some(TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: insert_text,
        });
    }

    let header_indent = indent_at_offset(&doc.content, usize::from(pou.text_range().start()));
    let indent_unit = infer_indent_unit(&doc.content);
    let body_indent = format!("{header_indent}{indent_unit}");
    let insert_offset = line_end_offset(&doc.content, usize::from(pou.text_range().start()));
    let insert_text = format!(
        "{newline}{header_indent}VAR{newline}{body_indent}{name} : {type_name};{newline}{header_indent}END_VAR{newline}"
    );
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

fn find_var_block(pou: &SyntaxNode) -> Option<SyntaxNode> {
    pou.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarBlock)
        .find(|block| {
            block
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.kind() == SyntaxKind::KwVar)
        })
}

fn indent_for_var_block(
    source: &str,
    block: &SyntaxNode,
    end_var_token: &SyntaxToken,
) -> Option<String> {
    let mut decl_indent = None;
    for decl in block
        .children()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let indent = indent_at_offset(source, usize::from(decl.text_range().start()));
        if !indent.is_empty() {
            decl_indent = Some(indent);
            break;
        }
    }
    if decl_indent.is_some() {
        return decl_indent;
    }
    let base = indent_at_offset(source, usize::from(end_var_token.text_range().start()));
    Some(format!("{}{}", base, infer_indent_unit(source)))
}

fn infer_missing_var_type(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    range: TextRange,
) -> Option<String> {
    let token = root.token_at_offset(range.start()).right_biased()?;
    let name_node = token.parent().and_then(|parent| {
        parent
            .ancestors()
            .find(|node| matches!(node.kind(), SyntaxKind::NameRef | SyntaxKind::Name))
    })?;

    if name_node
        .ancestors()
        .any(|node| node.kind() == SyntaxKind::Condition)
    {
        return Some("BOOL".to_string());
    }

    if let Some(assign_stmt) = name_node
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
    {
        let mut children = assign_stmt.children();
        let lhs = children.next();
        if let Some(lhs) = lhs {
            if lhs.text_range().contains(name_node.text_range().start()) {
                if let Some(expr_node) = assign_stmt
                    .children()
                    .filter(|node| is_expression_kind(node.kind()))
                    .last()
                {
                    let expr_offset = u32::from(expr_node.text_range().start());
                    let type_id = state.with_database(|db| {
                        let expr_id = db.expr_id_at_offset(doc.file_id, expr_offset)?;
                        Some(db.type_of(doc.file_id, expr_id))
                    })?;
                    return type_name_for_type_id(state, doc, type_id);
                }
            }
        }
    }

    None
}

fn type_name_for_type_id(
    state: &ServerState,
    doc: &crate::state::Document,
    type_id: TypeId,
) -> Option<String> {
    state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        symbols
            .type_name(type_id)
            .map(|name| name.to_string())
            .or_else(|| type_id.builtin_name().map(|name| name.to_string()))
    })
}

fn missing_type_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let name = extract_quoted_name(&diagnostic.message)?;
    let newline = newline_for_source(&doc.content);

    let insert_offset = if let Some(last_type) = root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::TypeDecl)
        .max_by_key(|node| node.text_range().end())
    {
        line_end_offset(&doc.content, usize::from(last_type.text_range().end()))
    } else if let Some(first_pou) = root.descendants().find(|node| is_pou_kind(node.kind())) {
        line_start_offset(&doc.content, usize::from(first_pou.text_range().start()))
    } else {
        0
    };

    let insert_text = format!("{newline}TYPE {name} : INT;{newline}END_TYPE{newline}");
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

fn missing_end_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let expected = expected_end_keyword(&diagnostic.message)?;
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(start));

    let indent = if let Some(kind) = node_kind_for_end_keyword(expected) {
        if let Some(node) = find_enclosing_node_of_kind(root, diag_range, kind) {
            indent_at_offset(&doc.content, usize::from(node.text_range().start()))
        } else {
            indent_at_offset(&doc.content, start as usize)
        }
    } else {
        indent_at_offset(&doc.content, start as usize)
    };

    let newline = newline_for_source(&doc.content);
    let insert_text = format!("{indent}{expected}{newline}");
    let insert_offset = line_start_offset(&doc.content, start as usize);
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

fn expected_end_keyword(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("expected ")?;
    let token = rest.split_whitespace().next()?;
    if token.starts_with("END_") {
        Some(token)
    } else {
        None
    }
}

fn node_kind_for_end_keyword(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "END_IF" => Some(SyntaxKind::IfStmt),
        "END_CASE" => Some(SyntaxKind::CaseStmt),
        "END_FOR" => Some(SyntaxKind::ForStmt),
        "END_WHILE" => Some(SyntaxKind::WhileStmt),
        "END_REPEAT" => Some(SyntaxKind::RepeatStmt),
        "END_PROGRAM" => Some(SyntaxKind::Program),
        "END_FUNCTION" => Some(SyntaxKind::Function),
        "END_FUNCTION_BLOCK" => Some(SyntaxKind::FunctionBlock),
        "END_CLASS" => Some(SyntaxKind::Class),
        "END_INTERFACE" => Some(SyntaxKind::Interface),
        "END_NAMESPACE" => Some(SyntaxKind::Namespace),
        "END_TYPE" => Some(SyntaxKind::TypeDecl),
        "END_VAR" => Some(SyntaxKind::VarBlock),
        "END_METHOD" => Some(SyntaxKind::Method),
        "END_PROPERTY" => Some(SyntaxKind::Property),
        "END_ACTION" => Some(SyntaxKind::Action),
        "END_CONFIGURATION" => Some(SyntaxKind::Configuration),
        "END_RESOURCE" => Some(SyntaxKind::Resource),
        _ => None,
    }
}

fn find_enclosing_node_of_kind(
    root: &SyntaxNode,
    range: TextRange,
    kind: SyntaxKind,
) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == kind)
        .filter(|node| {
            let node_range = node.text_range();
            node_range.contains(range.start()) && node_range.contains(range.end())
        })
        .min_by_key(|node| node.text_range().len())
}

fn missing_return_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let func_node = find_enclosing_node_of_kind(root, diag_range, SyntaxKind::Function)?;
    let end_token = func_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::KwEndFunction)?;

    let name_node = func_node
        .children()
        .find(|node| node.kind() == SyntaxKind::Name)?;
    let func_name = name_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();

    let return_type = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        let symbol_id = symbols.resolve(&func_name, trust_hir::symbols::ScopeId::GLOBAL)?;
        let symbol = symbols.get(symbol_id)?;
        match symbol.kind {
            trust_hir::symbols::SymbolKind::Function { return_type, .. } => Some(return_type),
            _ => None,
        }
    })?;

    let default_value = default_literal_for_type(state, doc, return_type).unwrap_or("0".into());
    let end_offset = usize::from(end_token.text_range().start());
    let insert_offset = line_start_offset(&doc.content, end_offset);
    let base_indent = indent_at_offset(&doc.content, end_offset);
    let indent = format!("{base_indent}{}", infer_indent_unit(&doc.content));
    let newline = newline_for_source(&doc.content);
    let insert_text = format!("{indent}RETURN {default_value};{newline}");
    let insert_pos = offset_to_position(&doc.content, insert_offset as u32);
    Some(TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: insert_text,
    })
}

fn default_literal_for_type(
    state: &ServerState,
    doc: &crate::state::Document,
    type_id: TypeId,
) -> Option<String> {
    let resolved = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        symbols.resolve_alias_type(type_id)
    });

    match resolved {
        TypeId::BOOL => Some("FALSE".to_string()),
        TypeId::REAL | TypeId::LREAL => Some("0.0".to_string()),
        TypeId::STRING => Some("''".to_string()),
        TypeId::WSTRING => Some("\"\"".to_string()),
        TypeId::TIME => Some("T#0s".to_string()),
        TypeId::LTIME => Some("LTIME#0s".to_string()),
        TypeId::DATE => Some("DATE#1970-01-01".to_string()),
        TypeId::LDATE => Some("LDATE#1970-01-01".to_string()),
        TypeId::TOD => Some("TOD#00:00:00".to_string()),
        TypeId::LTOD => Some("LTOD#00:00:00".to_string()),
        TypeId::DT => Some("DT#1970-01-01-00:00:00".to_string()),
        TypeId::LDT => Some("LDT#1970-01-01-00:00:00".to_string()),
        _ => None,
    }
}

fn implicit_conversion_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let (source, target) = parse_conversion_types(&diagnostic.message)?;
    let func = format!("{}_TO_{}", source, target);
    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let end = position_to_offset(&doc.content, diagnostic.range.end)?;
    let diag_range = TextRange::new(TextSize::from(start), TextSize::from(end));
    let expr_range = find_enclosing_node_of_kind(root, diag_range, SyntaxKind::AssignStmt)
        .and_then(|assign| {
            assign
                .children()
                .filter(|node| is_expression_kind(node.kind()))
                .last()
                .map(|expr| expr.text_range())
        })
        .unwrap_or(diag_range);
    let expr_text = text_for_range(&doc.content, expr_range);
    if expr_text.is_empty() {
        return None;
    }
    let new_text = format!("{func}({expr_text})");
    Some(TextEdit {
        range: Range {
            start: offset_to_position(&doc.content, expr_range.start().into()),
            end: offset_to_position(&doc.content, expr_range.end().into()),
        },
        new_text,
    })
}

fn parse_conversion_types(message: &str) -> Option<(String, String)> {
    let message = message.trim();
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let mid = rest.find('\'')?;
    let source = rest[..mid].to_string();
    let rest = &rest[mid + 1..];
    let start = rest.find('\'')?;
    let rest = &rest[start + 1..];
    let end = rest.find('\'')?;
    let target = rest[..end].to_string();
    Some((source.to_ascii_uppercase(), target.to_ascii_uppercase()))
}

fn fix_output_binding_text_edit(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let message = diagnostic.message.as_str();
    let (expected, replacement) = if message.contains("must use '=>'") {
        (SyntaxKind::Assign, "=>")
    } else if message.contains("use ':='") {
        (SyntaxKind::Arrow, ":=")
    } else {
        return None;
    };

    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let arg_node = find_enclosing_node_of_kind(root, range, SyntaxKind::Arg)?;
    let assign_token = arg_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == expected)?;
    Some(TextEdit {
        range: Range {
            start: offset_to_position(&doc.content, assign_token.text_range().start().into()),
            end: offset_to_position(&doc.content, assign_token.text_range().end().into()),
        },
        new_text: replacement.to_string(),
    })
}

fn convert_call_style_text_edit(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Option<Vec<(String, TextEdit)>> {
    let message = diagnostic.message.as_str();
    let ordering_error = message.contains("positional arguments must precede formal arguments");
    if !message.contains("formal calls cannot mix positional arguments")
        && !message.contains("formal call arguments must be named")
        && !ordering_error
    {
        return None;
    }

    let start = position_to_offset(&doc.content, diagnostic.range.start)?;
    let range = TextRange::new(TextSize::from(start), TextSize::from(start));
    let call_expr = find_enclosing_node_of_kind(root, range, SyntaxKind::CallExpr)?;
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;

    let args = parse_call_args(&arg_list, &doc.content);
    if args.is_empty() {
        return None;
    }

    let params = state.with_database(|db| {
        call_signature_info(db, doc.file_id, TextSize::from(start)).map(|info| info.params)
    })?;
    let params = params
        .into_iter()
        .filter(|param| !is_execution_param(param.name.as_str()))
        .collect::<Vec<_>>();

    let mut edits = Vec::new();
    if ordering_error {
        if let Some(text) = build_positional_first_call(&args, &params) {
            edits.push((
                "Reorder to positional-first call".to_string(),
                replace_arg_list_edit(&doc.content, &arg_list, text),
            ));
        }
    }
    if let Some(text) = build_formal_call(&args, &params) {
        edits.push((
            "Convert to formal call".to_string(),
            replace_arg_list_edit(&doc.content, &arg_list, text),
        ));
    }
    if let Some(text) = build_positional_call(&args, &params) {
        edits.push((
            "Convert to positional call".to_string(),
            replace_arg_list_edit(&doc.content, &arg_list, text),
        ));
    }
    (!edits.is_empty()).then_some(edits)
}

fn namespace_disambiguation_actions(
    state: &ServerState,
    doc: &crate::state::Document,
    root: &SyntaxNode,
    diagnostic: &Diagnostic,
) -> Vec<CodeActionOrCommand> {
    if !diagnostic.message.contains("ambiguous reference to") {
        return Vec::new();
    }
    let name = extract_quoted_name(&diagnostic.message).or_else(|| {
        let start = position_to_offset(&doc.content, diagnostic.range.start)?;
        trust_ide::util::ident_at_offset(&doc.content, TextSize::from(start))
            .map(|(name, _)| name.to_string())
    });
    let Some(name) = name else {
        return Vec::new();
    };
    let Some(start) = position_to_offset(&doc.content, diagnostic.range.start) else {
        return Vec::new();
    };
    let scope_id = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        scope_at_position(&symbols, root, TextSize::from(start))
    });

    let candidates = state.with_database(|db| {
        let symbols = db.file_symbols_with_project(doc.file_id);
        collect_using_candidates(&symbols, scope_id, &name)
    });
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut actions = Vec::new();
    for parts in candidates {
        let qualified = join_namespace_path(&parts);
        let edit = TextEdit {
            range: diagnostic.range,
            new_text: qualified.clone(),
        };
        let mut changes: std::collections::HashMap<Url, Vec<TextEdit>> =
            std::collections::HashMap::new();
        changes.insert(doc.uri.clone(), vec![edit]);

        let action = CodeAction {
            title: format!("Qualify with {qualified}"),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            is_preferred: Some(true),
            ..Default::default()
        };
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    actions
}

fn namespace_move_action(
    doc: &crate::state::Document,
    root: &SyntaxNode,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let start = position_to_offset(&doc.content, params.range.start)?;
    let end = position_to_offset(&doc.content, params.range.end).unwrap_or(start);
    let range = TextRange::new(TextSize::from(start), TextSize::from(end));

    let namespace_node = find_enclosing_node_of_kind(root, range, SyntaxKind::Namespace)?;
    let name_node = namespace_node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))?;
    let name_range = name_node.text_range();
    if !name_range.contains(range.start()) || !name_range.contains(range.end()) {
        return None;
    }

    let title = "Move namespace (rename path)".to_string();
    let command = Command {
        title: title.clone(),
        command: "editor.action.rename".to_string(),
        arguments: None,
    };
    let action = CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        command: Some(command),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

fn interface_stub_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        trust_ide::generate_interface_stubs(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Generate interface stubs".to_string(),
        kind: Some(CodeActionKind::REFACTOR),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

fn inline_symbol_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state
        .with_database(|db| trust_ide::inline_symbol(db, doc.file_id, TextSize::from(offset)))?;
    let changes = rename_result_to_changes(state, result.edits)?;

    let title = match result.kind {
        InlineTargetKind::Constant => "Inline constant".to_string(),
        InlineTargetKind::Variable => "Inline variable".to_string(),
    };
    let action = CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR_INLINE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

fn extract_actions(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Vec<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return Vec::new();
    }
    let start = match position_to_offset(&doc.content, params.range.start) {
        Some(offset) => offset,
        None => return Vec::new(),
    };
    let end = position_to_offset(&doc.content, params.range.end).unwrap_or(start);
    if start == end {
        return Vec::new();
    }
    let range = TextRange::new(TextSize::from(start), TextSize::from(end));

    let mut actions = Vec::new();

    if let Some(result) = state.with_database(|db| extract_method(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract method".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    if let Some(result) = state.with_database(|db| extract_property(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract property".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    if let Some(result) = state.with_database(|db| extract_pou(db, doc.file_id, range)) {
        if let Some(changes) = rename_result_to_changes(state, result.edits) {
            let action = CodeAction {
                title: "Extract function".to_string(),
                kind: Some(CodeActionKind::REFACTOR_EXTRACT),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                ..Default::default()
            };
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
}

fn convert_function_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        convert_function_to_function_block(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Convert FUNCTION to FUNCTION_BLOCK".to_string(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

fn convert_function_block_action(
    state: &ServerState,
    doc: &crate::state::Document,
    params: &CodeActionParams,
) -> Option<CodeActionOrCommand> {
    if !allows_refactor_action(&params.context.only) {
        return None;
    }
    let offset = position_to_offset(&doc.content, params.range.start)?;
    let result = state.with_database(|db| {
        convert_function_block_to_function(db, doc.file_id, TextSize::from(offset))
    })?;
    let changes = rename_result_to_changes(state, result)?;

    let action = CodeAction {
        title: "Convert FUNCTION_BLOCK to FUNCTION".to_string(),
        kind: Some(CodeActionKind::REFACTOR_REWRITE),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        ..Default::default()
    };
    Some(CodeActionOrCommand::CodeAction(action))
}

fn allows_refactor_action(only: &Option<Vec<CodeActionKind>>) -> bool {
    let Some(only) = only else {
        return true;
    };
    only.iter().any(|kind| {
        let value = kind.as_str();
        value == CodeActionKind::REFACTOR.as_str()
            || value == CodeActionKind::REFACTOR_REWRITE.as_str()
            || value.starts_with(CodeActionKind::REFACTOR.as_str())
    })
}

fn collect_using_candidates(
    symbols: &SymbolTable,
    scope_id: ScopeId,
    name: &str,
) -> Vec<Vec<SmolStr>> {
    let mut candidates = Vec::new();
    let mut current = Some(scope_id);
    while let Some(scope_id) = current {
        let Some(scope) = symbols.get_scope(scope_id) else {
            break;
        };
        if scope.lookup_local(name).is_some() {
            break;
        }
        for using in &scope.using_directives {
            let mut parts = using.path.clone();
            parts.push(SmolStr::new(name));
            if symbols.resolve_qualified(&parts).is_some() {
                candidates.push(parts);
            }
        }
        current = scope.parent;
    }

    let mut seen = FxHashSet::default();
    let mut unique = Vec::new();
    for parts in candidates {
        let key = parts
            .iter()
            .map(|part| part.to_ascii_uppercase())
            .collect::<Vec<_>>()
            .join(".");
        if seen.insert(key) {
            unique.push(parts);
        }
    }
    unique
}

fn join_namespace_path(parts: &[SmolStr]) -> String {
    let mut out = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(part.as_str());
    }
    out
}

fn parse_call_args(arg_list: &SyntaxNode, source: &str) -> Vec<ParsedArg> {
    let mut args = Vec::new();
    for arg in arg_list
        .children()
        .filter(|node| node.kind() == SyntaxKind::Arg)
    {
        let name = arg
            .children()
            .find(|node| node.kind() == SyntaxKind::Name)
            .and_then(|node| {
                node.descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.kind() == SyntaxKind::Ident)
            })
            .map(|token| token.text().to_string());

        let expr_node = arg
            .children()
            .filter(|node| node.kind() != SyntaxKind::Name)
            .last();
        let expr_text = expr_node
            .map(|node| text_for_range(source, node.text_range()))
            .unwrap_or_default();

        args.push(ParsedArg { name, expr_text });
    }
    args
}

#[derive(Debug, Clone)]
struct ParsedArg {
    name: Option<String>,
    expr_text: String,
}

fn build_formal_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    if args.len() > params.len() {
        return None;
    }
    let mut out = Vec::new();
    for (idx, arg) in args.iter().enumerate() {
        let param = params.get(idx)?;
        let op = match param.direction {
            ParamDirection::Out => "=>",
            ParamDirection::In | ParamDirection::InOut => ":=",
        };
        out.push(format!("{} {} {}", param.name, op, arg.expr_text));
    }
    Some(format!("({})", out.join(", ")))
}

fn build_positional_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    let mut positional = args
        .iter()
        .filter(|arg| arg.name.is_none())
        .map(|arg| arg.expr_text.clone())
        .collect::<Vec<_>>();
    let mut positional_iter = positional.drain(..);
    let mut by_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for arg in args {
        if let Some(name) = &arg.name {
            by_name.insert(name.to_ascii_uppercase(), arg.expr_text.clone());
        }
    }

    let mut out = Vec::new();
    for param in params {
        let key = param.name.to_ascii_uppercase();
        if let Some(expr) = by_name.remove(&key) {
            out.push(expr);
        } else if let Some(expr) = positional_iter.next() {
            out.push(expr);
        } else {
            return None;
        }
    }

    Some(format!("({})", out.join(", ")))
}

fn build_positional_first_call(
    args: &[ParsedArg],
    params: &[trust_ide::CallSignatureParam],
) -> Option<String> {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    for arg in args {
        if arg.name.is_some() {
            named.push(arg);
        } else {
            positional.push(arg);
        }
    }
    if positional.is_empty() || named.is_empty() {
        return None;
    }

    let mut out = positional
        .iter()
        .map(|arg| arg.expr_text.clone())
        .collect::<Vec<_>>();

    for arg in named {
        let name = arg.name.as_ref()?;
        let param = params
            .iter()
            .find(|param| param.name.eq_ignore_ascii_case(name))?;
        let op = match param.direction {
            ParamDirection::Out => "=>",
            ParamDirection::In | ParamDirection::InOut => ":=",
        };
        out.push(format!("{name} {op} {}", arg.expr_text));
    }

    Some(format!("({})", out.join(", ")))
}

fn replace_arg_list_edit(source: &str, arg_list: &SyntaxNode, new_text: String) -> TextEdit {
    TextEdit {
        range: Range {
            start: offset_to_position(source, arg_list.text_range().start().into()),
            end: offset_to_position(source, arg_list.text_range().end().into()),
        },
        new_text,
    }
}

fn is_execution_param(name: &str) -> bool {
    name.eq_ignore_ascii_case("EN") || name.eq_ignore_ascii_case("ENO")
}

fn text_for_range(source: &str, range: TextRange) -> String {
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    source
        .get(start..end)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BinaryExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::DerefExpr
            | SyntaxKind::AddrExpr
            | SyntaxKind::SizeOfExpr
            | SyntaxKind::NameRef
            | SyntaxKind::Literal
            | SyntaxKind::ThisExpr
            | SyntaxKind::SuperExpr
            | SyntaxKind::InitializerList
            | SyntaxKind::ArrayInitializer
    )
}

fn is_pou_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Class
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::Interface
    )
}
