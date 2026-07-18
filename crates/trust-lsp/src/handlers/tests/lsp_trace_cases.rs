use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value as JsonValue};
use text_size::{TextRange, TextSize};
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use trust_ide::semantic_tokens::{SemanticToken as IdeSemanticToken, SemanticTokenType};
use verification_cases::{CaseRecord, TraceStep};

use crate::state::ServerState;
use crate::test_support::test_client;

use super::diagnostics;
use super::lsp_utils::{offset_to_position, position_to_offset, semantic_tokens_to_lsp};
use super::trace_support::{run_trace_cases, scenario};
use super::{did_change, did_close, document_diagnostic, hover};

const POSITION_TEST_ID: &str = "TEST_LSP_POSITION_ENCODING_TRACE_001";
const POSITION_CASE_FILE: &str = "verification/cases/editor_safety/EDIT_LSP_POS_001.toml";
const POSITION_CASE_FILE_DIGEST: &str =
    "sha256:9a8c6ae86d4cfff6f76db6e863b8e7147d0e94ee57d706a7003d1fa2c1d7c19d";
const DIAGNOSTIC_TEST_ID: &str = "TEST_LSP_DIAGNOSTIC_CANCELLATION_TRACE_001";
const DIAGNOSTIC_CASE_FILE: &str = "verification/cases/editor_safety/EDIT_DIAG_CANCEL_001.toml";
const DIAGNOSTIC_CASE_FILE_DIGEST: &str =
    "sha256:2fe98659ad5421a9e1874d10cdbf706c3c1c30e5380fdc2bf736a414d9a6c1dc";
const CLOSE_TEST_ID: &str = "TEST_LSP_DOCUMENT_CLOSE_TRACE_001";
const CLOSE_CASE_FILE: &str = "verification/cases/editor_safety/EDIT_DOC_CLOSE_001.toml";
const CLOSE_CASE_FILE_DIGEST: &str =
    "sha256:9ab5679c9ed203ba3cd56f60f2862d9aa38a28620b98497bbe69f9f31c0b2ff9";

#[test]
fn lsp_position_encoding_trace_cases() {
    run_trace_cases(
        POSITION_TEST_ID,
        POSITION_CASE_FILE,
        POSITION_CASE_FILE_DIGEST,
        execute_position_case,
    );
}

#[test]
fn lsp_diagnostic_cancellation_trace_cases() {
    run_trace_cases(
        DIAGNOSTIC_TEST_ID,
        DIAGNOSTIC_CASE_FILE,
        DIAGNOSTIC_CASE_FILE_DIGEST,
        execute_diagnostic_case,
    );
}

#[test]
fn lsp_document_close_trace_cases() {
    run_trace_cases(
        CLOSE_TEST_ID,
        CLOSE_CASE_FILE,
        CLOSE_CASE_FILE_DIGEST,
        execute_close_case,
    );
}

fn execute_position_case(case: &CaseRecord, _step: &TraceStep) -> Result<JsonValue, String> {
    match scenario(case)? {
        "INITIALIZE_UTF16" => {
            let runtime = test_runtime();
            let server = crate::StLanguageServer::new(test_client());
            let result = runtime
                .block_on(server.initialize(InitializeParams::default()))
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "encoding": result.capabilities.position_encoding.map(|value| value.as_str().to_string()).unwrap_or_default(),
            }))
        }
        "INCREMENTAL_EDIT_AFTER_EMOJI" => {
            let state = ServerState::new();
            let client = test_client();
            let uri = Url::parse("file:///position-trace.st").map_err(|error| error.to_string())?;
            state.open_document(uri.clone(), 1, "😀x := 1;\n".to_string());
            test_runtime().block_on(did_change(
                &client,
                &state,
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: Some(Range::new(Position::new(0, 2), Position::new(0, 3))),
                        range_length: None,
                        text: "y".to_string(),
                    }],
                },
            ));
            Ok(json!({
                "content": state.get_document(&uri).map(|doc| doc.content).unwrap_or_default(),
            }))
        }
        "LF_LINE_MAPPING" => line_ending_observation("\n"),
        "CRLF_LINE_MAPPING" => line_ending_observation("\r\n"),
        "BARE_CR_LINE_MAPPING" => line_ending_observation("\r"),
        "OVERLONG_COLUMN_CLAMP" => Ok(json!({
            "byte_offset": position_to_offset("😀x", Position::new(0, 99)).unwrap_or_default(),
        })),
        "HOVER_RANGE_AFTER_EMOJI" => hover_observation(),
        "SEMANTIC_TOKEN_AFTER_EMOJI" => {
            let token = IdeSemanticToken::new(
                TextRange::new(TextSize::from(4), TextSize::from(5)),
                SemanticTokenType::Variable,
            );
            let token = semantic_tokens_to_lsp("😀x", [token], 0, 0)
                .into_iter()
                .next()
                .ok_or_else(|| "missing semantic token".to_string())?;
            Ok(json!({"delta_start": token.delta_start, "length": token.length}))
        }
        "EOF_AFTER_TRAILING_NEWLINE" => {
            let source = "PROGRAM Test\nEND_PROGRAM\n";
            let position = offset_to_position(source, source.len() as u32);
            Ok(json!({
                "line": position.line,
                "character": position.character,
                "byte_offset": position_to_offset(source, position).unwrap_or_default(),
            }))
        }
        other => Err(format!("unreviewed position scenario {other}")),
    }
}

fn line_ending_observation(ending: &str) -> Result<JsonValue, String> {
    let source = format!("😀x{ending}y");
    let y_offset = source.find('y').ok_or_else(|| "missing y".to_string())? as u32;
    let terminator_offset = "😀x".len() as u32;
    let next = offset_to_position(&source, y_offset);
    let terminator = offset_to_position(&source, terminator_offset);
    Ok(json!({
        "line": next.line,
        "character": next.character,
        "terminator_line": terminator.line,
        "terminator_character": terminator.character,
    }))
}

fn hover_observation() -> Result<JsonValue, String> {
    let source = "PROGRAM Test\nVAR\n    (*😀*) x : INT;\nEND_VAR\nx := 1;\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = Url::parse("file:///hover-position-trace.st").map_err(|error| error.to_string())?;
    state.open_document(uri.clone(), 1, source.to_string());
    let offset = source
        .find("x : INT")
        .ok_or_else(|| "missing x".to_string())? as u32;
    let request = offset_to_position(source, offset);
    let result = hover(
        &state,
        HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: request,
            },
            work_done_progress_params: Default::default(),
        },
    )
    .ok_or_else(|| "hover returned no result".to_string())?;
    let range = result
        .range
        .ok_or_else(|| "hover returned no range".to_string())?;
    Ok(json!({
        "request_character": request.character,
        "range_start_character": range.start.character,
        "range_end_character": range.end.character,
    }))
}

fn execute_diagnostic_case(case: &CaseRecord, _step: &TraceStep) -> Result<JsonValue, String> {
    let source = "PROGRAM Test\nVAR\n    A__B : INT;\nEND_VAR\nEND_PROGRAM\n";
    let state = ServerState::new();
    let uri = Url::parse("file:///diagnostic-trace.st").map_err(|error| error.to_string())?;
    state.open_document(uri.clone(), 1, source.to_string());
    let doc = state
        .get_document(&uri)
        .ok_or_else(|| "missing diagnostic document".to_string())?;
    match scenario(case)? {
        "COMPLETED_PUSH" => {
            let ticket = state.begin_semantic_request();
            let items = diagnostics::publish_diagnostics_items_with_ticket_for_tests(
                &state,
                &uri,
                source,
                doc.file_id,
                ticket,
            )
            .ok_or_else(|| "completed push returned no diagnostics".to_string())?;
            Ok(json!({"result": "diagnostics", "contains_e106": contains_e106(&items)}))
        }
        "CANCELLED_PUSH" => {
            let stale = state.begin_semantic_request();
            let _newer = state.begin_semantic_request();
            let items = diagnostics::publish_diagnostics_items_with_ticket_for_tests(
                &state,
                &uri,
                source,
                doc.file_id,
                stale,
            );
            Ok(json!({
                "result": if items.is_none() { "no_publication" } else { "diagnostics" },
                "contains_e106": items.as_deref().is_some_and(contains_e106),
            }))
        }
        "CANCELLED_TEXT_PULL" => {
            let stale = state.begin_semantic_request();
            let _newer = state.begin_semantic_request();
            let error = diagnostics::document_diagnostic_result_with_ticket_for_tests(
                &state,
                document_params(uri),
                stale,
            )
            .expect_err("cancelled text pull must fail");
            Ok(json!({"result": error_name(error.code)}))
        }
        "CANCELLED_WORKSPACE_PULL" => {
            let stale = state.begin_semantic_request();
            let _newer = state.begin_semantic_request();
            let error = diagnostics::workspace_diagnostic_result_with_ticket_for_tests(
                &state,
                workspace_params(),
                stale,
            )
            .expect_err("cancelled workspace pull must fail");
            Ok(json!({"result": error_name(error.code), "report_count": 0}))
        }
        "COMPLETED_WORKSPACE_PULL" => {
            let ticket = state.begin_semantic_request();
            let report = diagnostics::workspace_diagnostic_result_with_ticket_for_tests(
                &state,
                workspace_params(),
                ticket,
            )
            .map_err(|error| error.message.to_string())?;
            let WorkspaceDiagnosticReportResult::Report(report) = report else {
                return Err("workspace diagnostic returned partial output".to_string());
            };
            let contains = report.items.iter().any(workspace_report_contains_e106);
            Ok(json!({
                "result": "reports",
                "report_count": report.items.len(),
                "contains_e106": contains,
            }))
        }
        other => Err(format!("unreviewed diagnostic scenario {other}")),
    }
}

fn execute_close_case(case: &CaseRecord, _step: &TraceStep) -> Result<JsonValue, String> {
    match scenario(case)? {
        "TRACKED_FILE_RELOAD" => tracked_reload_observation(false),
        "UNREADABLE_OR_NON_FILE_REMOVE" => non_file_remove_observation(),
        "CACHE_INVALIDATION" => tracked_reload_observation(true),
        "DEPENDENT_RECOMPUTE" => dependent_recompute_observation(false),
        "PULL_DIAGNOSTIC_RECOMPUTE" => dependent_recompute_observation(true),
        other => Err(format!("unreviewed close scenario {other}")),
    }
}

fn tracked_reload_observation(check_caches: bool) -> Result<JsonValue, String> {
    let root = temp_dir("trust-lsp-close-trace")?;
    let path = root.join("Tracked.st");
    let disk = "PROGRAM Disk\nEND_PROGRAM\n";
    let unsaved = "PROGRAM Unsaved\nEND_PROGRAM\n";
    std::fs::write(&path, disk).map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path).map_err(|_| "invalid file URI".to_string())?;
    let state = ServerState::new();
    state.open_document(uri.clone(), 1, unsaved.to_string());
    if check_caches {
        state
            .store_semantic_tokens(uri.clone(), Vec::new(), state.begin_semantic_request())
            .ok_or_else(|| "semantic token cache write was cancelled".to_string())?;
        state
            .store_diagnostics(uri.clone(), 1, 1, state.begin_semantic_request())
            .ok_or_else(|| "diagnostic cache write was cancelled".to_string())?;
    }
    state.close_document(&uri);
    let doc = state.get_document(&uri);
    let observed = if check_caches {
        json!({
            "semantic_tokens_cached": state.semantic_tokens_cache(&uri).is_some(),
            "diagnostics_cached": state.diagnostic_result_id(&uri).is_some(),
            "content": if doc.as_ref().is_some_and(|doc| doc.content == disk) { "disk" } else { "other" },
        })
    } else {
        json!({
            "present": doc.is_some(),
            "is_open": doc.as_ref().is_some_and(|doc| doc.is_open),
            "content": if doc.as_ref().is_some_and(|doc| doc.content == disk) { "disk" } else { "other" },
        })
    };
    std::fs::remove_dir_all(root).ok();
    Ok(observed)
}

fn non_file_remove_observation() -> Result<JsonValue, String> {
    let uri = Url::parse("untitled:close-trace").map_err(|error| error.to_string())?;
    let state = ServerState::new();
    let file_id = state.open_document(uri.clone(), 1, "PROGRAM Unsaved\nEND_PROGRAM\n".to_string());
    state.close_document(&uri);
    let project_source_present = state.with_database(|db| db.file_ids().contains(&file_id));
    Ok(json!({
        "present": state.get_document(&uri).is_some(),
        "project_source_present": project_source_present,
    }))
}

fn dependent_recompute_observation(use_handler: bool) -> Result<JsonValue, String> {
    let root = temp_dir("trust-lsp-close-dependent")?;
    let add_path = root.join("Add.st");
    let main_path = root.join("Main.st");
    let add_disk = "FUNCTION Add : INT\nVAR_INPUT\n    A : INT;\n    B : INT;\nEND_VAR\nAdd := A + B;\nEND_FUNCTION\n";
    let add_unsaved =
        "FUNCTION Add : INT\nVAR_INPUT\n    A : INT;\nEND_VAR\nAdd := A;\nEND_FUNCTION\n";
    let main = "PROGRAM Main\nVAR\n    Result : INT;\nEND_VAR\nResult := Add(1, 2);\nEND_PROGRAM\n";
    std::fs::write(&add_path, add_disk).map_err(|error| error.to_string())?;
    std::fs::write(&main_path, main).map_err(|error| error.to_string())?;
    let add_uri = Url::from_file_path(&add_path).map_err(|_| "invalid add URI".to_string())?;
    let main_uri = Url::from_file_path(&main_path).map_err(|_| "invalid main URI".to_string())?;
    let state = ServerState::new();
    state.open_document(add_uri.clone(), 1, add_unsaved.to_string());
    state.open_document(main_uri.clone(), 1, main.to_string());
    let before = full_document_diagnostics(&state, main_uri.clone())?;
    if use_handler {
        let client = test_client();
        state.set_diagnostic_pull_supported(true);
        test_runtime().block_on(did_close(
            &client,
            &state,
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier {
                    uri: add_uri.clone(),
                },
            },
        ));
    } else {
        state.close_document(&add_uri);
    }
    let after = full_document_diagnostics(&state, main_uri)?;
    let observed = if use_handler {
        json!({
            "closed_uri_is_open": state.get_document(&add_uri).is_some_and(|doc| doc.is_open),
            "dependent_argument_error": contains_argument_error(&after),
        })
    } else {
        json!({
            "before_argument_error": contains_argument_error(&before),
            "after_argument_error": contains_argument_error(&after),
        })
    };
    std::fs::remove_dir_all(root).ok();
    Ok(observed)
}

fn full_document_diagnostics(state: &ServerState, uri: Url) -> Result<Vec<Diagnostic>, String> {
    let report = document_diagnostic(state, document_params(uri));
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(full)) = report
    else {
        return Err("expected full document diagnostic report".to_string());
    };
    Ok(full.full_document_diagnostic_report.items)
}

fn document_params(uri: Url) -> DocumentDiagnosticParams {
    DocumentDiagnosticParams {
        text_document: TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

fn workspace_params() -> WorkspaceDiagnosticParams {
    WorkspaceDiagnosticParams {
        identifier: None,
        previous_result_ids: Vec::new(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

fn contains_e106(items: &[Diagnostic]) -> bool {
    items.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "E106"
        )
    })
}

fn contains_argument_error(items: &[Diagnostic]) -> bool {
    items
        .iter()
        .any(|diagnostic| diagnostic.message.contains("arguments"))
}

fn workspace_report_contains_e106(report: &WorkspaceDocumentDiagnosticReport) -> bool {
    match report {
        WorkspaceDocumentDiagnosticReport::Full(full) => {
            contains_e106(&full.full_document_diagnostic_report.items)
        }
        WorkspaceDocumentDiagnosticReport::Unchanged(_) => false,
    }
}

fn error_name(code: tower_lsp::jsonrpc::ErrorCode) -> &'static str {
    if code == tower_lsp::jsonrpc::ErrorCode::ContentModified {
        "content_modified"
    } else {
        "other"
    }
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn temp_dir(prefix: &str) -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let path = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}
