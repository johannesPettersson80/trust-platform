use tower_lsp::lsp_types::request::{
    GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
    GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
};
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

use crate::state::ServerState;

pub fn goto_definition(
    state: &ServerState,
    params: GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    super::core_impl::goto_definition(state, params)
}

pub fn goto_declaration(
    state: &ServerState,
    params: GotoDeclarationParams,
) -> Option<GotoDeclarationResponse> {
    super::core_impl::goto_declaration(state, params)
}

pub fn goto_type_definition(
    state: &ServerState,
    params: GotoTypeDefinitionParams,
) -> Option<GotoTypeDefinitionResponse> {
    super::core_impl::goto_type_definition(state, params)
}

pub fn goto_implementation(
    state: &ServerState,
    params: GotoImplementationParams,
) -> Option<GotoImplementationResponse> {
    super::core_impl::goto_implementation(state, params)
}

#[cfg(test)]
pub fn references(state: &ServerState, params: ReferenceParams) -> Option<Vec<Location>> {
    super::core_impl::references(state, params)
}

pub async fn references_with_progress(
    client: &Client,
    state: &ServerState,
    params: ReferenceParams,
) -> Option<Vec<Location>> {
    super::core_impl::references_with_progress(client, state, params).await
}

pub fn document_highlight(
    state: &ServerState,
    params: DocumentHighlightParams,
) -> Option<Vec<DocumentHighlight>> {
    super::core_impl::document_highlight(state, params)
}

pub fn selection_range(
    state: &ServerState,
    params: SelectionRangeParams,
) -> Option<Vec<SelectionRange>> {
    super::core_impl::selection_range(state, params)
}

pub fn prepare_rename(
    state: &ServerState,
    params: TextDocumentPositionParams,
) -> Option<PrepareRenameResponse> {
    super::core_impl::prepare_rename(state, params)
}

pub fn rename(
    state: &ServerState,
    params: RenameParams,
) -> tower_lsp::jsonrpc::Result<Option<WorkspaceEdit>> {
    super::core_impl::rename(state, params)
}
