//! LSP request handlers.
//!
//! This module wires handler submodules together.

mod commands;
mod config;
mod diagnostics;
mod features;
mod formatting;
mod lsp_utils;
mod progress;
mod refresh;
mod runtime_values;
mod sync;
mod workspace;

#[cfg(test)]
pub(crate) use commands::namespace_move_workspace_edit;
pub use commands::{execute_command, MOVE_NAMESPACE_COMMAND, PROJECT_INFO_COMMAND};
pub(crate) use diagnostics::{document_diagnostic, workspace_diagnostic};
pub use features::{
    code_action, code_lens, completion, completion_resolve, document_highlight, document_link,
    document_symbol, folding_range, goto_declaration, goto_definition, goto_implementation,
    goto_type_definition, hover, incoming_calls, inlay_hint, inline_value, linked_editing_range,
    outgoing_calls, prepare_call_hierarchy, prepare_rename, prepare_type_hierarchy,
    references_with_progress, rename, selection_range, semantic_tokens_full,
    semantic_tokens_full_delta, semantic_tokens_range, signature_help, type_hierarchy_subtypes,
    type_hierarchy_supertypes, workspace_symbol_with_progress,
};
pub use formatting::{formatting, on_type_formatting, range_formatting};
pub use refresh::{refresh_diagnostics, refresh_semantic_tokens};
pub use sync::{did_change, did_close, did_open, did_save};
pub use workspace::{
    did_change_configuration, did_change_watched_files, did_rename_files,
    index_workspace_background_with_refresh, register_file_watchers, register_type_hierarchy,
    will_rename_files,
};

#[cfg(test)]
pub use workspace::index_workspace;

#[cfg(test)]
mod tests;
