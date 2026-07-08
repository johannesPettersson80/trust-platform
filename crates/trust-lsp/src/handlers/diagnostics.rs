//! Diagnostics publishing helpers.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tower_lsp::jsonrpc::{Error as JsonRpcError, ErrorCode, Result as JsonRpcResult};
use tower_lsp::lsp_types::{
    CodeDescription, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, Location, NumberOrString, Range,
    RelatedFullDocumentDiagnosticReport, RelatedUnchangedDocumentDiagnosticReport,
    UnchangedDocumentDiagnosticReport, Url, WorkspaceDiagnosticParams, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport, WorkspaceUnchangedDocumentDiagnosticReport,
};
use tower_lsp::Client;

use trust_hir::db::FileId;
use trust_hir::symbols::SymbolKind;
use trust_hir::DiagnosticSeverity as HirSeverity;
#[cfg(test)]
use trust_runtime::bundle_builder::resolve_sources_root;
#[cfg(test)]
use trust_runtime::debug::DebugSnapshot;
#[cfg(test)]
use trust_runtime::harness::{CompileSession, SourceFile as HarnessSourceFile};
#[cfg(test)]
use trust_runtime::hmi::{self as runtime_hmi, HmiSourceRef};
use trust_syntax::parser::parse;

use crate::config::{DiagnosticSettings, ProjectConfig, CONFIG_FILES};
use crate::external_diagnostics::collect_external_diagnostics;
use crate::library_graph::library_dependency_issues;
use crate::state::{path_to_uri, uri_to_path, ServerState};

use super::lsp_utils::{offset_to_position, position_to_offset};

pub(crate) async fn publish_diagnostics(
    client: &Client,
    state: &ServerState,
    uri: &Url,
    content: &str,
    file_id: FileId,
) {
    let request_ticket = state.begin_semantic_request();
    let Some(diagnostics) =
        publish_diagnostics_items_with_ticket(state, uri, content, file_id, request_ticket)
    else {
        return;
    };

    client
        .publish_diagnostics(uri.clone(), diagnostics, None)
        .await;
}

fn publish_diagnostics_items_with_ticket(
    state: &ServerState,
    uri: &Url,
    content: &str,
    file_id: FileId,
    request_ticket: u64,
) -> Option<Vec<Diagnostic>> {
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let diagnostics =
        collect_diagnostics_with_ticket(state, uri, content, file_id, Some(request_ticket));
    if state.semantic_request_cancelled(request_ticket) {
        return None;
    }

    let content_hash = hash_content(content);
    let diagnostic_hash = hash_diagnostics(&diagnostics);
    let _ = state.store_diagnostics(uri.clone(), content_hash, diagnostic_hash);
    Some(diagnostics)
}

#[cfg(test)]
pub(crate) fn publish_diagnostics_items_with_ticket_for_tests(
    state: &ServerState,
    uri: &Url,
    content: &str,
    file_id: FileId,
    request_ticket: u64,
) -> Option<Vec<Diagnostic>> {
    publish_diagnostics_items_with_ticket(state, uri, content, file_id, request_ticket)
}

#[cfg(test)]
pub(crate) fn document_diagnostic(
    state: &ServerState,
    params: DocumentDiagnosticParams,
) -> DocumentDiagnosticReportResult {
    document_diagnostic_result(state, params).unwrap_or_else(|_| empty_document_diagnostic_report())
}

pub(crate) fn document_diagnostic_result(
    state: &ServerState,
    params: DocumentDiagnosticParams,
) -> JsonRpcResult<DocumentDiagnosticReportResult> {
    let request_ticket = state.begin_semantic_request();
    document_diagnostic_result_with_ticket(state, params, request_ticket)
}

fn document_diagnostic_result_with_ticket(
    state: &ServerState,
    params: DocumentDiagnosticParams,
    request_ticket: u64,
) -> JsonRpcResult<DocumentDiagnosticReportResult> {
    if state.semantic_request_cancelled(request_ticket) {
        return Err(content_modified_error());
    }

    let uri = params.text_document.uri;
    let Some(doc) = state.ensure_document(&uri) else {
        return Ok(empty_document_diagnostic_report());
    };

    let diagnostics = collect_diagnostics_with_ticket(
        state,
        &uri,
        &doc.content,
        doc.file_id,
        Some(request_ticket),
    );
    if state.semantic_request_cancelled(request_ticket) {
        return Err(content_modified_error());
    }

    let content_hash = hash_content(&doc.content);
    let diagnostic_hash = hash_diagnostics(&diagnostics);
    let result_id = state.store_diagnostics(uri.clone(), content_hash, diagnostic_hash);

    if params
        .previous_result_id
        .as_ref()
        .is_some_and(|previous| previous == &result_id)
    {
        return Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Unchanged(RelatedUnchangedDocumentDiagnosticReport {
                related_documents: None,
                unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                    result_id,
                },
            }),
        ));
    }

    Ok(DocumentDiagnosticReportResult::Report(
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: Some(result_id),
                items: diagnostics,
            },
        }),
    ))
}

fn empty_document_diagnostic_report() -> DocumentDiagnosticReportResult {
    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
        RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: None,
                items: Vec::new(),
            },
        },
    ))
}

fn content_modified_error() -> JsonRpcError {
    JsonRpcError::new(ErrorCode::ContentModified)
}

#[cfg(test)]
pub(crate) fn document_diagnostic_result_with_ticket_for_tests(
    state: &ServerState,
    params: DocumentDiagnosticParams,
    request_ticket: u64,
) -> JsonRpcResult<DocumentDiagnosticReportResult> {
    document_diagnostic_result_with_ticket(state, params, request_ticket)
}

pub(crate) fn workspace_diagnostic(
    state: &ServerState,
    params: WorkspaceDiagnosticParams,
) -> WorkspaceDiagnosticReportResult {
    let request_ticket = state.begin_semantic_request();
    let mut previous = std::collections::HashMap::new();
    for entry in params.previous_result_ids {
        previous.insert(entry.uri, entry.value);
    }

    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for doc in state.documents() {
        if state.semantic_request_cancelled(request_ticket) {
            break;
        }
        seen.insert(doc.uri.clone());
        let diagnostics = collect_diagnostics_with_ticket(
            state,
            &doc.uri,
            &doc.content,
            doc.file_id,
            Some(request_ticket),
        );
        let content_hash = hash_content(&doc.content);
        let diagnostic_hash = hash_diagnostics(&diagnostics);
        let result_id = state.store_diagnostics(doc.uri.clone(), content_hash, diagnostic_hash);

        if previous
            .get(&doc.uri)
            .is_some_and(|prev| prev == &result_id)
        {
            items.push(WorkspaceDocumentDiagnosticReport::Unchanged(
                WorkspaceUnchangedDocumentDiagnosticReport {
                    uri: doc.uri.clone(),
                    version: doc.is_open.then_some(doc.version as i64),
                    unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                        result_id,
                    },
                },
            ));
        } else {
            items.push(WorkspaceDocumentDiagnosticReport::Full(
                WorkspaceFullDocumentDiagnosticReport {
                    uri: doc.uri.clone(),
                    version: doc.is_open.then_some(doc.version as i64),
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: Some(result_id),
                        items: diagnostics,
                    },
                },
            ));
        }
    }

    for (root, config) in state.workspace_configs() {
        let Some(config_path) = config.config_path.clone() else {
            continue;
        };
        let Some(uri) = path_to_uri(&config_path) else {
            continue;
        };
        if seen.contains(&uri) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            continue;
        };
        let diagnostics = collect_config_diagnostics(state, &uri, &content, Some(&root));
        let content_hash = hash_content(&content);
        let diagnostic_hash = hash_diagnostics(&diagnostics);
        let result_id = state.store_diagnostics(uri.clone(), content_hash, diagnostic_hash);
        if previous.get(&uri).is_some_and(|prev| prev == &result_id) {
            items.push(WorkspaceDocumentDiagnosticReport::Unchanged(
                WorkspaceUnchangedDocumentDiagnosticReport {
                    uri: uri.clone(),
                    version: None,
                    unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                        result_id,
                    },
                },
            ));
        } else {
            items.push(WorkspaceDocumentDiagnosticReport::Full(
                WorkspaceFullDocumentDiagnosticReport {
                    uri: uri.clone(),
                    version: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: Some(result_id),
                        items: diagnostics,
                    },
                },
            ));
        }
    }

    WorkspaceDiagnosticReportResult::Report(WorkspaceDiagnosticReport { items })
}

pub(crate) fn collect_diagnostics_with_ticket(
    state: &ServerState,
    uri: &Url,
    content: &str,
    file_id: FileId,
    request_ticket: Option<u64>,
) -> Vec<Diagnostic> {
    let is_cancelled =
        request_ticket.is_some_and(|ticket| state.semantic_request_cancelled(ticket));
    if is_cancelled {
        return Vec::new();
    }

    if is_config_uri(uri) {
        let diagnostics = collect_config_diagnostics(state, uri, content, None);
        let mut diagnostics = diagnostics;
        apply_diagnostic_filters(state, uri, &mut diagnostics);
        apply_diagnostic_overrides(state, uri, &mut diagnostics);
        attach_explainers(state, uri, content, None, &mut diagnostics);
        return diagnostics;
    }

    if is_hmi_toml_uri(uri) {
        let mut diagnostics = collect_hmi_toml_diagnostics(state, uri, content);
        apply_diagnostic_filters(state, uri, &mut diagnostics);
        apply_diagnostic_overrides(state, uri, &mut diagnostics);
        attach_explainers(state, uri, content, None, &mut diagnostics);
        return diagnostics;
    }

    let parsed = parse(content);

    let mut diagnostics: Vec<Diagnostic> = parsed
        .errors()
        .iter()
        .map(|err| {
            let range = Range {
                start: offset_to_position(content, err.range.start().into()),
                end: offset_to_position(content, err.range.end().into()),
            };
            let code = if err.message.starts_with("expected ") {
                "E002"
            } else {
                "E001"
            };

            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(code.to_string())),
                source: Some("truST".to_string()),
                message: err.message.clone(),
                ..Default::default()
            }
        })
        .collect();

    let semantic = state.with_database(|db| {
        if request_ticket.is_some_and(|ticket| state.semantic_request_cancelled(ticket)) {
            Vec::new()
        } else {
            trust_ide::diagnostics::collect_diagnostics(db, file_id)
        }
    });

    if request_ticket.is_some_and(|ticket| state.semantic_request_cancelled(ticket)) {
        return diagnostics;
    }

    for diag in semantic {
        let range = Range {
            start: offset_to_position(content, diag.range.start().into()),
            end: offset_to_position(content, diag.range.end().into()),
        };
        let severity = match diag.severity {
            HirSeverity::Error => DiagnosticSeverity::ERROR,
            HirSeverity::Warning => DiagnosticSeverity::WARNING,
            HirSeverity::Info => DiagnosticSeverity::INFORMATION,
            HirSeverity::Hint => DiagnosticSeverity::HINT,
        };
        let related_information = if diag.related.is_empty() {
            None
        } else {
            Some(
                diag.related
                    .into_iter()
                    .map(|rel| DiagnosticRelatedInformation {
                        location: Location {
                            uri: uri.clone(),
                            range: Range {
                                start: offset_to_position(content, rel.range.start().into()),
                                end: offset_to_position(content, rel.range.end().into()),
                            },
                        },
                        message: rel.message,
                    })
                    .collect(),
            )
        };

        diagnostics.push(Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(diag.code.code().to_string())),
            source: Some("truST".to_string()),
            message: diag.message,
            related_information,
            ..Default::default()
        });
    }

    if let Some(config) = state.workspace_config_for_uri(uri) {
        diagnostics.extend(collect_external_diagnostics(&config, uri));
    }

    let learner_context = build_learner_context(state, file_id);
    apply_diagnostic_filters(state, uri, &mut diagnostics);
    apply_diagnostic_overrides(state, uri, &mut diagnostics);
    attach_explainers(
        state,
        uri,
        content,
        Some(&learner_context),
        &mut diagnostics,
    );
    diagnostics
}

#[cfg(test)]
pub(crate) fn collect_diagnostics_with_ticket_for_tests(
    state: &ServerState,
    uri: &Url,
    content: &str,
    file_id: FileId,
    request_ticket: u64,
) -> Vec<Diagnostic> {
    collect_diagnostics_with_ticket(state, uri, content, file_id, Some(request_ticket))
}

include!("diagnostics/collection_and_filters.rs");
include!("diagnostics/mapping_and_explainers.rs");
include!("diagnostics/publish_hmi_and_tests.rs");
