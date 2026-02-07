use tower_lsp::lsp_types::{SemanticToken, Url};

use super::{DiagnosticCache, SemanticTokensCache, ServerState};

pub(super) fn semantic_tokens_cache(state: &ServerState, uri: &Url) -> Option<SemanticTokensCache> {
    state.semantic_tokens.read().get(uri).cloned()
}

pub(super) fn store_semantic_tokens(
    state: &ServerState,
    uri: Url,
    tokens: Vec<SemanticToken>,
) -> String {
    let result_id = next_semantic_tokens_id(state);
    let cache = SemanticTokensCache {
        result_id: result_id.clone(),
        tokens,
    };
    state.semantic_tokens.write().insert(uri, cache);
    result_id
}

pub(super) fn store_diagnostics(
    state: &ServerState,
    uri: Url,
    content_hash: u64,
    diagnostic_hash: u64,
) -> String {
    let mut cache = state.diagnostics.write();
    if let Some(existing) = cache.get(&uri) {
        if existing.content_hash == content_hash && existing.diagnostic_hash == diagnostic_hash {
            return existing.result_id.clone();
        }
    }
    let result_id = next_diagnostic_id(state);
    cache.insert(
        uri,
        DiagnosticCache {
            result_id: result_id.clone(),
            content_hash,
            diagnostic_hash,
        },
    );
    result_id
}

fn next_semantic_tokens_id(state: &ServerState) -> String {
    state
        .semantic_tokens_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string()
}

fn next_diagnostic_id(state: &ServerState) -> String {
    format!(
        "diag-{}",
        state
            .diagnostic_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
