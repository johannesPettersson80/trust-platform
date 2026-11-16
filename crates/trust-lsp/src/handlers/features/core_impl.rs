//! LSP language feature handlers.

use rustc_hash::FxHashSet;
use serde_json::json;
use std::path::Path;
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

use super::super::lsp_utils::{
    display_symbol_name, is_primary_pou_symbol_kind, lsp_symbol_kind, offset_to_line_col,
    offset_to_position, position_to_offset, rename_result_to_changes, semantic_tokens_to_lsp,
    st_file_stem, symbol_container_name, text_document_identifier_for_edit,
};
use super::super::progress::{
    send_partial_result, send_work_done_begin, send_work_done_end, send_work_done_report,
};
use crate::config::WorkspaceVisibility;
use crate::external_diagnostics::ExternalFixData;
use crate::handlers::diagnostics::collect_diagnostics_with_ticket;
use crate::library_docs::doc_for_name;
use crate::state::{path_to_uri, uri_to_path, ServerState};
use trust_ide::goto_def::goto_definition as ide_goto_definition;
use trust_ide::text_range::{extend_range_to_line_end, text_for_range};
use trust_ide::util::scope_at_position;
use trust_ide::var_decl::find_var_decl_for_range;
use trust_ide::{
    call_signature_info, convert_function_block_to_function, convert_function_to_function_block,
    extract_method, extract_pou, extract_property, InlineTargetKind, StdlibFilter,
};

const PARTIAL_CHUNK_SIZE: usize = 200;

mod action_requests;
mod completion_requests;
mod helpers;
mod hierarchy_requests;
mod navigation_requests;
mod rust_sources;
mod semantic_requests;
mod symbol_requests;

use helpers::*;

pub use action_requests::{code_action, prepare_rename, rename};
#[cfg(test)]
pub(crate) use completion_requests::completion_with_ticket_for_tests;
pub use completion_requests::{completion, completion_resolve, hover, signature_help};
pub use hierarchy_requests::{
    code_lens, incoming_calls, outgoing_calls, prepare_call_hierarchy, prepare_type_hierarchy,
    type_hierarchy_subtypes, type_hierarchy_supertypes,
};
#[cfg(test)]
pub use navigation_requests::references;
pub use navigation_requests::{
    document_highlight, goto_declaration, goto_definition, goto_implementation,
    goto_type_definition, references_with_progress,
};
pub use semantic_requests::{
    folding_range, inlay_hint, linked_editing_range, selection_range, semantic_tokens_full,
    semantic_tokens_full_delta, semantic_tokens_range,
};
#[cfg(test)]
pub use symbol_requests::workspace_symbol;
pub use symbol_requests::{document_symbol, workspace_symbol_with_progress};

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
        "codesys" | "beckhoff" | "twincat" | "siemens" | "mitsubishi" | "gxworks3" => Some("iec"),
        _ => None,
    }
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
