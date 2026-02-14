//! Diagnostics publishing helpers.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
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
use trust_runtime::bundle_builder::resolve_sources_root;
use trust_runtime::debug::DebugSnapshot;
use trust_runtime::harness::{CompileSession, SourceFile as HarnessSourceFile};
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
    let diagnostics =
        collect_diagnostics_with_ticket(state, uri, content, file_id, Some(request_ticket));
    let content_hash = hash_content(content);
    let diagnostic_hash = hash_diagnostics(&diagnostics);
    let _ = state.store_diagnostics(uri.clone(), content_hash, diagnostic_hash);

    client
        .publish_diagnostics(uri.clone(), diagnostics, None)
        .await;
}

pub(crate) fn document_diagnostic(
    state: &ServerState,
    params: DocumentDiagnosticParams,
) -> DocumentDiagnosticReportResult {
    let request_ticket = state.begin_semantic_request();
    let uri = params.text_document.uri;
    let Some(doc) = state.ensure_document(&uri) else {
        return DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items: Vec::new(),
                },
            },
        ));
    };

    let diagnostics = collect_diagnostics_with_ticket(
        state,
        &uri,
        &doc.content,
        doc.file_id,
        Some(request_ticket),
    );
    let content_hash = hash_content(&doc.content);
    let diagnostic_hash = hash_diagnostics(&diagnostics);
    let result_id = state.store_diagnostics(uri.clone(), content_hash, diagnostic_hash);

    if params
        .previous_result_id
        .as_ref()
        .is_some_and(|previous| previous == &result_id)
    {
        return DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(
            RelatedUnchangedDocumentDiagnosticReport {
                related_documents: None,
                unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                    result_id,
                },
            },
        ));
    }

    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
        RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: Some(result_id),
                items: diagnostics,
            },
        },
    ))
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
                source: Some("trust-lsp".to_string()),
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
            source: Some("trust-lsp".to_string()),
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

fn apply_diagnostic_filters(state: &ServerState, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
    let Some(config) = state.workspace_config_for_uri(uri) else {
        return;
    };
    let settings = config.diagnostics;
    diagnostics.retain(|diagnostic| diagnostic_allowed(&settings, diagnostic));
}

fn apply_diagnostic_overrides(state: &ServerState, uri: &Url, diagnostics: &mut [Diagnostic]) {
    let Some(config) = state.workspace_config_for_uri(uri) else {
        return;
    };
    let overrides = &config.diagnostics.severity_overrides;
    if overrides.is_empty() {
        return;
    }
    for diagnostic in diagnostics {
        let Some(code) = diagnostic_code(diagnostic) else {
            continue;
        };
        if let Some(severity) = overrides.get(&code) {
            diagnostic.severity = Some(*severity);
        }
    }
}

fn diagnostic_allowed(settings: &DiagnosticSettings, diagnostic: &Diagnostic) -> bool {
    let Some(code) = diagnostic_code(diagnostic) else {
        return true;
    };
    match code.as_str() {
        "W001" | "W002" | "W009" => settings.warn_unused,
        "W003" => settings.warn_unreachable,
        "W004" => settings.warn_missing_else,
        "W005" => settings.warn_implicit_conversion,
        "W006" => settings.warn_shadowed,
        "W007" => settings.warn_deprecated,
        "W008" => settings.warn_complexity,
        "W010" | "W011" => settings.warn_nondeterminism,
        _ => true,
    }
}

#[derive(Default, Debug, Clone)]
struct LearnerContext {
    value_candidates: Vec<String>,
    type_candidates: Vec<String>,
}

fn build_learner_context(state: &ServerState, file_id: FileId) -> LearnerContext {
    state.with_database(|db| {
        let symbols = db.file_symbols_with_project(file_id);
        let mut value_map = BTreeMap::<String, String>::new();
        let mut type_map = BTreeMap::<String, String>::new();

        for symbol in symbols.iter() {
            if is_value_suggestion_kind(&symbol.kind) {
                let name = symbol.name.to_string();
                value_map.entry(name.to_ascii_uppercase()).or_insert(name);
            }
            if symbol.is_type() {
                let name = symbol.name.to_string();
                type_map.entry(name.to_ascii_uppercase()).or_insert(name);
            }
        }

        for builtin in BUILTIN_TYPE_NAMES {
            let name = builtin.to_string();
            type_map
                .entry(name.to_ascii_uppercase())
                .or_insert_with(|| name);
        }

        LearnerContext {
            value_candidates: value_map.into_values().collect(),
            type_candidates: type_map.into_values().collect(),
        }
    })
}

const BUILTIN_TYPE_NAMES: &[&str] = &[
    "BOOL", "BYTE", "WORD", "DWORD", "LWORD", "SINT", "INT", "DINT", "LINT", "USINT", "UINT",
    "UDINT", "ULINT", "REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT",
    "LDT", "STRING", "WSTRING", "CHAR", "WCHAR", "POINTER",
];

const HMI_DIAG_UNKNOWN_BIND: &str = "HMI_BIND_UNKNOWN_PATH";
const HMI_DIAG_INVALID_PROPERTIES: &str = "HMI_INVALID_WIDGET_PROPERTIES";

fn is_value_suggestion_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Variable { .. }
            | SymbolKind::Constant
            | SymbolKind::Parameter { .. }
            | SymbolKind::EnumValue { .. }
            | SymbolKind::ProgramInstance
    )
}

fn attach_explainers(
    state: &ServerState,
    uri: &Url,
    content: &str,
    learner_context: Option<&LearnerContext>,
    diagnostics: &mut [Diagnostic],
) {
    for diagnostic in diagnostics {
        let Some(code) = diagnostic_code(diagnostic) else {
            continue;
        };
        let mut data = match diagnostic.data.take() {
            Some(Value::Object(map)) => map,
            _ => Map::new(),
        };

        if let Some(explainer) = diagnostic_explainer(&code, &diagnostic.message) {
            if diagnostic.code_description.is_none() {
                if let Some(href) = spec_url(state, explainer.spec_path) {
                    diagnostic.code_description = Some(CodeDescription { href });
                }
            }
            data.insert(
                "explain".to_string(),
                json!({
                    "iec": explainer.iec_ref,
                    "spec": explainer.spec_path,
                }),
            );
        }

        let mut hints = Vec::new();
        let mut did_you_mean = Vec::new();

        if let Some(context) = learner_context {
            let suggestions = did_you_mean_suggestions(&code, &diagnostic.message, context);
            if !suggestions.is_empty() {
                hints.push(format!(
                    "Did you mean {}?",
                    format_suggestion_list(&suggestions)
                ));
                did_you_mean = suggestions;
            }
        }

        hints.extend(syntax_habit_hints(&code, diagnostic, content));

        if let Some(hint) = conversion_guidance_hint(&code, &diagnostic.message) {
            hints.push(hint);
        }

        dedupe_preserve_order(&mut hints);
        for hint in &hints {
            push_related_hint(diagnostic, uri, hint);
        }
        if !hints.is_empty() {
            data.insert("hints".to_string(), json!(hints));
        }
        if !did_you_mean.is_empty() {
            data.insert("didYouMean".to_string(), json!(did_you_mean));
        }

        if !data.is_empty() {
            diagnostic.data = Some(Value::Object(data));
        }
    }
}

fn push_related_hint(diagnostic: &mut Diagnostic, uri: &Url, hint: &str) {
    let message = format!("Hint: {hint}");
    if diagnostic
        .related_information
        .as_ref()
        .is_some_and(|related| {
            related
                .iter()
                .any(|info| info.message.eq_ignore_ascii_case(&message))
        })
    {
        return;
    }
    diagnostic
        .related_information
        .get_or_insert_with(Vec::new)
        .push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: diagnostic.range,
            },
            message,
        });
}

fn did_you_mean_suggestions(code: &str, message: &str, context: &LearnerContext) -> Vec<String> {
    match code {
        "E101" => {
            let Some(query) = extract_quoted_after_prefix(message, "undefined identifier '") else {
                return Vec::new();
            };
            top_ranked_suggestions(&query, &context.value_candidates)
        }
        "E102" => {
            let query = extract_quoted_after_prefix(message, "cannot resolve type '")
                .or_else(|| extract_quoted_after_prefix(message, "cannot resolve interface '"));
            let Some(query) = query else {
                return Vec::new();
            };
            top_ranked_suggestions(&query, &context.type_candidates)
        }
        _ => Vec::new(),
    }
}

fn extract_quoted_after_prefix(message: &str, prefix: &str) -> Option<String> {
    let tail = message.strip_prefix(prefix)?;
    let end = tail.find('\'')?;
    let value = tail[..end].trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn format_suggestion_list(suggestions: &[String]) -> String {
    match suggestions {
        [] => String::new(),
        [one] => format!("'{one}'"),
        [one, two] => format!("'{one}' or '{two}'"),
        [one, two, three, ..] => format!("'{one}', '{two}', or '{three}'"),
    }
}

fn top_ranked_suggestions(query: &str, candidates: &[String]) -> Vec<String> {
    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return Vec::new();
    }
    let (min_score, max_distance) = suggestion_thresholds(normalized_query.len());
    let mut seen = std::collections::HashSet::new();
    let mut scored = Vec::new();

    for candidate in candidates {
        let normalized_candidate = normalize_identifier(candidate);
        if normalized_candidate.is_empty() || normalized_candidate == normalized_query {
            continue;
        }
        if !seen.insert(normalized_candidate.clone()) {
            continue;
        }

        let distance = levenshtein_distance(&normalized_query, &normalized_candidate);
        if distance > max_distance {
            continue;
        }
        let score = similarity_score(&normalized_query, &normalized_candidate, distance);
        if score < min_score {
            continue;
        }
        scored.push((score, distance, candidate.clone()));
    }

    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.len().cmp(&b.2.len()))
            .then_with(|| a.2.cmp(&b.2))
    });

    scored
        .into_iter()
        .take(3)
        .map(|(_, _, name)| name)
        .collect()
}

fn suggestion_thresholds(query_len: usize) -> (f32, usize) {
    match query_len {
        0..=2 => (1.0, 0),
        3..=4 => (0.80, 1),
        5..=7 => (0.67, 2),
        8..=12 => (0.60, 3),
        _ => (0.55, 4),
    }
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.')
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

fn similarity_score(query: &str, candidate: &str, distance: usize) -> f32 {
    let max_len = query.len().max(candidate.len()).max(1) as f32;
    let mut score = 1.0 - (distance as f32 / max_len);
    if candidate.starts_with(query) || query.starts_with(candidate) {
        score += 0.20;
    } else if candidate.contains(query) || query.contains(candidate) {
        score += 0.10;
    }
    if query
        .chars()
        .next()
        .zip(candidate.chars().next())
        .is_some_and(|(a, b)| a == b)
    {
        score += 0.06;
    }
    score.clamp(0.0, 1.0)
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }

    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr: Vec<usize> = vec![0; b_chars.len() + 1];

    for (i, a_char) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = usize::from(a_char != *b_char);
            let deletion = prev[j + 1] + 1;
            let insertion = curr[j] + 1;
            let substitution = prev[j] + cost;
            curr[j + 1] = deletion.min(insertion).min(substitution);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

fn syntax_habit_hints(code: &str, diagnostic: &Diagnostic, content: &str) -> Vec<String> {
    if !matches!(code, "E001" | "E002" | "E003") {
        return Vec::new();
    }
    let mut hints = Vec::new();
    let snippet = diagnostic_snippet(content, diagnostic).unwrap_or_default();
    let line = content
        .lines()
        .nth(diagnostic.range.start.line as usize)
        .unwrap_or_default();
    let combined = format!("{line} {snippet}");
    let message = diagnostic.message.to_ascii_lowercase();

    if combined.contains("==") {
        hints.push("In Structured Text, use '=' for comparison and ':=' for assignment.".into());
    }
    if combined.contains("&&") {
        hints.push("In Structured Text, use AND instead of &&.".into());
    }
    if combined.contains("||") {
        hints.push("In Structured Text, use OR instead of ||.".into());
    }
    if combined.contains('{') || combined.contains('}') {
        hints.push("Structured Text uses END_* keywords for block endings, not '{' or '}'.".into());
    }

    let plain_equal = snippet.trim() == "="
        || (message.contains(":=") && contains_plain_equal(&combined) && !combined.contains("=="));
    if plain_equal {
        hints.push("In Structured Text, assignments use ':='.".into());
    }

    hints
}

fn diagnostic_snippet(content: &str, diagnostic: &Diagnostic) -> Option<String> {
    let start = position_to_offset(content, diagnostic.range.start)? as usize;
    let end = position_to_offset(content, diagnostic.range.end)? as usize;
    if start >= content.len() {
        return None;
    }
    let end = end.min(content.len());
    if start >= end {
        return None;
    }
    Some(content[start..end].to_string())
}

fn contains_plain_equal(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'=' {
            continue;
        }
        let prev = if index > 0 { bytes[index - 1] } else { b'\0' };
        let next = if index + 1 < bytes.len() {
            bytes[index + 1]
        } else {
            b'\0'
        };
        if prev != b':' && prev != b'<' && prev != b'>' && next != b'=' {
            return true;
        }
    }
    false
}

fn conversion_guidance_hint(code: &str, message: &str) -> Option<String> {
    if !matches!(code, "E201" | "E203" | "E207" | "W005") {
        return None;
    }
    let quoted = collect_quoted_segments(message);
    let (source, target) = if message
        .to_ascii_lowercase()
        .starts_with("return type mismatch: expected '")
    {
        if quoted.len() < 2 {
            return None;
        }
        (quoted[1], quoted[0])
    } else if quoted.len() >= 2 {
        (quoted[0], quoted[1])
    } else {
        return None;
    };

    let source = normalize_identifier(source);
    let target = normalize_identifier(target);
    if source.is_empty() || target.is_empty() || source == target {
        return None;
    }
    if !source
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
        || !target
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return None;
    }

    Some(format!(
        "Use an explicit conversion to make intent clear, e.g. `{source}_TO_{target}(<expr>)`."
    ))
}

fn collect_quoted_segments(message: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    while let Some(open) = message[start..].find('\'') {
        let open = start + open + 1;
        let Some(close) = message[open..].find('\'') else {
            break;
        };
        let close = open + close;
        if close > open {
            result.push(&message[open..close]);
        }
        start = close + 1;
    }
    result
}

fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.to_ascii_lowercase()));
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic.code.as_ref().map(|code| match code {
        NumberOrString::String(value) => value.clone(),
        NumberOrString::Number(value) => value.to_string(),
    })
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn hash_diagnostics(diagnostics: &[Diagnostic]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for diagnostic in diagnostics {
        diagnostic.range.start.line.hash(&mut hasher);
        diagnostic.range.start.character.hash(&mut hasher);
        diagnostic.range.end.line.hash(&mut hasher);
        diagnostic.range.end.character.hash(&mut hasher);
        let severity_key = match diagnostic.severity {
            Some(severity) if severity == DiagnosticSeverity::ERROR => 1,
            Some(severity) if severity == DiagnosticSeverity::WARNING => 2,
            Some(severity) if severity == DiagnosticSeverity::INFORMATION => 3,
            Some(severity) if severity == DiagnosticSeverity::HINT => 4,
            Some(_) => 0,
            None => 0,
        };
        severity_key.hash(&mut hasher);
        diagnostic_code(diagnostic).hash(&mut hasher);
        diagnostic.source.hash(&mut hasher);
        diagnostic.message.hash(&mut hasher);
        if let Some(related) = &diagnostic.related_information {
            related.len().hash(&mut hasher);
            for item in related {
                item.location.range.start.line.hash(&mut hasher);
                item.location.range.start.character.hash(&mut hasher);
                item.location.range.end.line.hash(&mut hasher);
                item.location.range.end.character.hash(&mut hasher);
                item.message.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

struct DiagnosticExplainer {
    iec_ref: &'static str,
    spec_path: &'static str,
}

fn diagnostic_explainer(code: &str, message: &str) -> Option<DiagnosticExplainer> {
    if code == "E202" && is_oop_access_message(message) {
        return Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.6.5; Table 50",
            spec_path: "docs/specs/09-semantic-rules.md",
        });
    }
    match code {
        "E001" | "E002" | "E003" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3",
            spec_path: "docs/specs/06-statements.md",
        }),
        "E101" | "E104" | "E105" | "W001" | "W002" | "W006" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.5.2.2",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E102" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "E103" | "E204" | "E205" | "E206" | "E207" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.6.1",
            spec_path: "docs/specs/04-pou-declarations.md",
        }),
        "E106" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.1.2",
            spec_path: "docs/specs/01-lexical-elements.md",
        }),
        "E201" | "E202" | "E203" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.2",
            spec_path: "docs/specs/05-expressions.md",
        }),
        "E301" | "E302" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.1",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E306" | "E307" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E303" | "E304" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2.6",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "W004" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.3.3.3",
            spec_path: "docs/specs/06-statements.md",
        }),
        "W003" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W005" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.4.2",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "W008" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W009" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W010" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); TIME/DATE types per IEC 61131-3 Ed.3 §6.4.2 (Table 10)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W011" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); Direct variables per IEC 61131-3 Ed.3 §6.5.5 (Table 16)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W012" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); shared globals across tasks (IEC 61131-3 Ed.3 §6.5.2.2 Tables 13-16; §6.2/§6.8.2 Table 62)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "L001" | "L002" | "L003" | "L005" | "L006" | "L007" => Some(DiagnosticExplainer {
            iec_ref: "Tooling config lint (non-IEC)",
            spec_path: "docs/specs/10-runtime.md",
        }),
        _ => None,
    }
}

fn is_oop_access_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("cannot access")
        || lower.contains("access specifier")
        || lower.contains("must be public or internal")
}

fn spec_url(state: &ServerState, spec_path: &str) -> Option<Url> {
    for root in state.workspace_folders() {
        let Some(root_path) = uri_to_path(&root) else {
            continue;
        };
        let candidate = root_path.join(spec_path);
        if candidate.exists() {
            return path_to_uri(&candidate);
        }
    }
    None
}

fn is_config_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| CONFIG_FILES.iter().any(|candidate| candidate == &name))
        .unwrap_or(false)
}

fn is_hmi_toml_uri(uri: &Url) -> bool {
    let Some(path) = uri_to_path(uri) else {
        return false;
    };
    if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
        return false;
    }
    path.components()
        .any(|component| component.as_os_str() == "hmi")
}

fn collect_hmi_toml_diagnostics(state: &ServerState, uri: &Url, content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = collect_hmi_toml_parse_diagnostics(content);
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let Some(path) = uri_to_path(uri) else {
        return diagnostics;
    };

    let root = state
        .workspace_config_for_uri(uri)
        .map(|config| config.root)
        .or_else(|| infer_hmi_root_from_path(path.as_path()));
    let Some(root) = root else {
        return diagnostics;
    };

    diagnostics.extend(collect_hmi_toml_semantic_diagnostics(
        root.as_path(),
        path.as_path(),
        content,
    ));
    diagnostics
}

fn collect_hmi_toml_parse_diagnostics(content: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Err(error) = toml::from_str::<toml::Value>(content) {
        let range = if let Some(span) = error.span() {
            Range {
                start: offset_to_position(content, span.start as u32),
                end: offset_to_position(content, span.end as u32),
            }
        } else {
            fallback_range(content)
        };
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("HMI_TOML_PARSE".to_string())),
            source: Some("trust-lsp".to_string()),
            message: error.to_string(),
            ..Default::default()
        });
    }
    diagnostics
}

fn infer_hmi_root_from_path(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) != Some("hmi") {
        return None;
    }
    parent.parent().map(Path::to_path_buf)
}

fn collect_hmi_toml_semantic_diagnostics(
    root: &Path,
    current_file: &Path,
    content: &str,
) -> Vec<Diagnostic> {
    let Some(descriptor) = runtime_hmi::load_hmi_dir(root) else {
        return Vec::new();
    };
    let loaded_sources = match load_hmi_sources_for_diagnostics(root) {
        Ok(sources) => sources,
        Err(_error) => return Vec::new(),
    };
    let compile_sources = loaded_sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let runtime = match CompileSession::from_sources(compile_sources).build_runtime() {
        Ok(runtime) => runtime,
        Err(_error) => return Vec::new(),
    };
    let metadata = runtime.metadata_snapshot();
    let snapshot = DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let source_refs = loaded_sources
        .iter()
        .map(|source| HmiSourceRef {
            path: source.path.as_path(),
            text: source.text.as_str(),
        })
        .collect::<Vec<_>>();
    let catalog =
        runtime_hmi::collect_hmi_bindings_catalog(&metadata, Some(&snapshot), &source_refs);
    let known_paths = catalog
        .programs
        .iter()
        .flat_map(|program| program.variables.iter().map(|entry| entry.path.clone()))
        .chain(catalog.globals.iter().map(|entry| entry.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let file_name = current_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let current_page_id = if file_name == "_config.toml" {
        None
    } else {
        current_file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(ToString::to_string)
    };

    let mut diagnostics = Vec::new();
    let binding_diagnostics =
        runtime_hmi::validate_hmi_bindings("RESOURCE", &metadata, Some(&snapshot), &descriptor);
    for binding in binding_diagnostics {
        if let Some(page_id) = current_page_id.as_ref() {
            if binding.page != *page_id {
                continue;
            }
        } else {
            continue;
        }
        let mut message = binding.message.clone();
        if binding.code == HMI_DIAG_UNKNOWN_BIND {
            let suggestions = top_ranked_suggestions(binding.bind.as_str(), &known_paths);
            if !suggestions.is_empty() {
                message = format!(
                    "{message}. Did you mean {}?",
                    format_suggestion_list(&suggestions)
                );
            }
        }
        diagnostics.push(Diagnostic {
            range: find_name_range(content, binding.bind.as_str()),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(binding.code.to_string())),
            source: Some("trust-lsp".to_string()),
            message,
            ..Default::default()
        });
    }

    if let Some(page_id) = current_page_id {
        if let Some(page) = descriptor.pages.iter().find(|page| page.id == page_id) {
            for section in &page.sections {
                for widget in &section.widgets {
                    let Some(kind) = widget.widget_type.as_ref() else {
                        continue;
                    };
                    let kind = kind.trim().to_ascii_lowercase();
                    if kind.is_empty() {
                        continue;
                    }
                    let bind = widget.bind.trim();
                    if let (Some(min), Some(max)) = (widget.min, widget.max) {
                        if min > max {
                            diagnostics.push(Diagnostic {
                                range: find_name_range(content, bind),
                                severity: Some(DiagnosticSeverity::WARNING),
                                code: Some(NumberOrString::String(
                                    HMI_DIAG_INVALID_PROPERTIES.to_string(),
                                )),
                                source: Some("trust-lsp".to_string()),
                                message: format!(
                                    "invalid widget property combination: min ({min}) is greater than max ({max})"
                                ),
                                ..Default::default()
                            });
                        }
                    }
                    if kind != "indicator"
                        && (widget.on_color.is_some() || widget.off_color.is_some())
                    {
                        diagnostics.push(Diagnostic {
                            range: find_name_range(content, bind),
                            severity: Some(DiagnosticSeverity::WARNING),
                            code: Some(NumberOrString::String(
                                HMI_DIAG_INVALID_PROPERTIES.to_string(),
                            )),
                            source: Some("trust-lsp".to_string()),
                            message: format!(
                                "invalid widget property combination: on_color/off_color only apply to indicator widgets (found '{kind}')"
                            ),
                            ..Default::default()
                        });
                    }
                    if kind == "indicator" && (widget.min.is_some() || widget.max.is_some()) {
                        diagnostics.push(Diagnostic {
                            range: find_name_range(content, bind),
                            severity: Some(DiagnosticSeverity::WARNING),
                            code: Some(NumberOrString::String(
                                HMI_DIAG_INVALID_PROPERTIES.to_string(),
                            )),
                            source: Some("trust-lsp".to_string()),
                            message:
                                "invalid widget property combination: indicator widgets do not support min/max"
                                    .to_string(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        let left_code = diagnostic_code(left).unwrap_or_default();
        let right_code = diagnostic_code(right).unwrap_or_default();
        left_code
            .cmp(&right_code)
            .then_with(|| left.message.cmp(&right.message))
    });
    diagnostics
}

#[derive(Debug, Clone)]
struct LoadedHmiSource {
    path: PathBuf,
    text: String,
}

fn load_hmi_sources_for_diagnostics(root: &Path) -> anyhow::Result<Vec<LoadedHmiSource>> {
    let sources_root = resolve_sources_root(root, None)?;
    let mut source_paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        let glob_pattern = format!("{}/{}", sources_root.display(), pattern);
        let entries = glob::glob(&glob_pattern)?;
        for entry in entries {
            source_paths.insert(entry?);
        }
    }
    if source_paths.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let mut sources = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let text = std::fs::read_to_string(&path)?;
        sources.push(LoadedHmiSource { path, text });
    }
    Ok(sources)
}

#[cfg(test)]
fn collect_hmi_toml_diagnostics_for_root(
    root: &Path,
    current_file: &Path,
    content: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = collect_hmi_toml_parse_diagnostics(content);
    if !diagnostics.is_empty() {
        return diagnostics;
    }
    diagnostics.extend(collect_hmi_toml_semantic_diagnostics(
        root,
        current_file,
        content,
    ));
    diagnostics
}

fn collect_config_diagnostics(
    state: &ServerState,
    uri: &Url,
    content: &str,
    root_hint: Option<&Url>,
) -> Vec<Diagnostic> {
    let root = root_hint
        .and_then(uri_to_path)
        .or_else(|| config_root_for_uri(state, uri))
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = uri_to_path(uri);
    let config = ProjectConfig::from_contents(&root, config_path, content);
    let mut diagnostics = Vec::new();
    for issue in &config.dependency_resolution_issues {
        let range = find_name_range(content, issue.dependency.as_str());
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(issue.code.to_string())),
            source: Some("trust-lsp".to_string()),
            message: issue.message.clone(),
            ..Default::default()
        });
    }
    for issue in library_dependency_issues(&config) {
        let target = issue
            .dependency
            .as_deref()
            .unwrap_or(issue.subject.as_str());
        let range = find_name_range(content, target);
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(issue.code.to_string())),
            source: Some("trust-lsp".to_string()),
            message: issue.message,
            ..Default::default()
        });
    }
    diagnostics
}

fn config_root_for_uri(state: &ServerState, uri: &Url) -> Option<PathBuf> {
    state
        .workspace_config_for_uri(uri)
        .map(|config| config.root)
        .or_else(|| uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf)))
}

fn find_name_range(content: &str, name: &str) -> Range {
    if name.is_empty() {
        return fallback_range(content);
    }
    let quoted = format!("\"{name}\"");
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(pos) = line.find(&quoted) {
            let start = pos + 1;
            let end = start + name.len();
            return Range {
                start: tower_lsp::lsp_types::Position::new(line_idx as u32, start as u32),
                end: tower_lsp::lsp_types::Position::new(line_idx as u32, end as u32),
            };
        }
        if let Some(pos) = line.find(name) {
            let end = pos + name.len();
            return Range {
                start: tower_lsp::lsp_types::Position::new(line_idx as u32, pos as u32),
                end: tower_lsp::lsp_types::Position::new(line_idx as u32, end as u32),
            };
        }
    }
    fallback_range(content)
}

fn fallback_range(content: &str) -> Range {
    let end = content
        .lines()
        .next()
        .map(|line| line.len() as u32)
        .unwrap_or(0);
    Range {
        start: tower_lsp::lsp_types::Position::new(0, 0),
        end: tower_lsp::lsp_types::Position::new(0, end),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_hmi_toml_diagnostics_for_root, diagnostic_code, top_ranked_suggestions,
        LearnerContext, HMI_DIAG_INVALID_PROPERTIES, HMI_DIAG_UNKNOWN_BIND,
    };
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, content).expect("write file");
    }

    #[test]
    fn suggestion_ranking_prefers_closest_match() {
        let context = LearnerContext {
            value_candidates: vec![
                "speedValue".to_string(),
                "seedValue".to_string(),
                "setpoint".to_string(),
            ],
            type_candidates: Vec::new(),
        };
        let suggestions = top_ranked_suggestions("speadValue", &context.value_candidates);
        assert_eq!(
            suggestions.first().map(String::as_str),
            Some("speedValue"),
            "closest typo fix should rank first"
        );
    }

    #[test]
    fn suggestion_ranking_suppresses_low_confidence_noise() {
        let context = LearnerContext {
            value_candidates: vec![
                "temperature".to_string(),
                "counter".to_string(),
                "runtimeTicks".to_string(),
            ],
            type_candidates: Vec::new(),
        };
        let suggestions = top_ranked_suggestions("zzzzzzz", &context.value_candidates);
        assert!(
            suggestions.is_empty(),
            "unrelated names should not produce misleading suggestions"
        );
    }

    #[test]
    fn hmi_toml_diagnostics_report_unknown_bind_with_near_match_hint() {
        let root = temp_dir("trust-lsp-hmi-diag-unknown-bind");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r#"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.spead"
"#;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic_code(diagnostic).as_deref() == Some(HMI_DIAG_UNKNOWN_BIND)
                && diagnostic.message.contains("Main.speed")
        }));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_toml_diagnostics_report_type_widget_and_property_issues() {
        let root = temp_dir("trust-lsp-hmi-diag-invalid-widget");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    run : BOOL;
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.run"

[[section.widget]]
type = "rocket"
bind = "Main.speed"

[[section.widget]]
type = "bar"
bind = "Main.speed"
on_color = "#22c55e"

[[section.widget]]
type = "indicator"
bind = "Main.run"
min = 10
max = 1
"##;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        let codes = diagnostics
            .iter()
            .filter_map(diagnostic_code)
            .collect::<Vec<_>>();
        assert!(codes.iter().any(|code| code == "HMI_BIND_TYPE_MISMATCH"));
        assert!(codes.iter().any(|code| code == "HMI_UNKNOWN_WIDGET_KIND"));
        assert!(codes.iter().any(|code| code == HMI_DIAG_INVALID_PROPERTIES));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_toml_diagnostics_avoid_false_positives_for_valid_page() {
        let root = temp_dir("trust-lsp-hmi-diag-valid");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    run : BOOL;
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "indicator"
bind = "Main.run"
on_color = "#22c55e"
off_color = "#94a3b8"

[[section.widget]]
type = "gauge"
bind = "Main.speed"
min = 0
max = 100
"##;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        assert!(
            diagnostics.is_empty(),
            "valid descriptor should not produce diagnostics: {diagnostics:#?}"
        );

        std::fs::remove_dir_all(root).ok();
    }
}
