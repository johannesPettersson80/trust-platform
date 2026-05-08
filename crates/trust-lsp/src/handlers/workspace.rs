//! Workspace indexing handlers.

use rustc_hash::FxHashSet;
use serde_json::json;
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_lsp::lsp_types::notification::Progress;
use tower_lsp::lsp_types::request::WorkDoneProgressCreate;
use tower_lsp::lsp_types::{
    DidChangeConfigurationParams, DidChangeWatchedFilesParams, FileChangeType, MessageType,
    ProgressParams, ProgressParamsValue, ProgressToken, Registration, RenameFilesParams, TextEdit,
    Url, WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressCreateParams,
    WorkDoneProgressEnd, WorkDoneProgressReport, WorkspaceEdit,
};
use tower_lsp::Client;
use tracing::info;

use crate::config::{ProjectConfig, CONFIG_FILES};
use crate::index_cache::IndexCache;
use crate::state::{path_to_uri, uri_to_path, ServerState};
use trust_hir::db::SemanticDatabase;
use trust_hir::symbols::{ScopeId, SymbolId, SymbolTable};
use trust_hir::{is_reserved_keyword, is_valid_identifier, SymbolKind};

use super::lsp_utils;
use super::refresh::{refresh_diagnostics, refresh_semantic_tokens};

pub async fn register_file_watchers(client: &Client) {
    let mut watchers = Vec::new();
    watchers.push(json!({ "globPattern": "**/*.{st,ST,pou,POU}" }));
    for name in CONFIG_FILES {
        watchers.push(json!({ "globPattern": format!("**/{name}") }));
    }

    let registration = Registration {
        id: "trustlsp-watchers".to_string(),
        method: "workspace/didChangeWatchedFiles".to_string(),
        register_options: Some(json!({ "watchers": watchers })),
    };
    if let Err(err) = client.register_capability(vec![registration]).await {
        client
            .log_message(
                MessageType::WARNING,
                format!("Failed to register file watchers: {err}"),
            )
            .await;
    }
}

pub async fn register_type_hierarchy(client: &Client) {
    let registration = Registration {
        id: "trustlsp-type-hierarchy".to_string(),
        method: "textDocument/prepareTypeHierarchy".to_string(),
        register_options: Some(json!({
            "documentSelector": [{ "scheme": "file", "language": "structured-text" }]
        })),
    };
    if let Err(err) = client.register_capability(vec![registration]).await {
        client
            .log_message(
                MessageType::WARNING,
                format!("Failed to register type hierarchy capability: {err}"),
            )
            .await;
    }
}

pub async fn index_workspace(client: &Client, state: &ServerState) {
    let folders = state.workspace_folders();
    if folders.is_empty() {
        return;
    }

    let mut indexed_total = 0usize;
    let mut skipped_total = 0usize;
    let mut truncated_roots = 0usize;
    let mut seen = FxHashSet::default();

    for folder in folders {
        let Some(root) = uri_to_path(&folder) else {
            continue;
        };
        let config = ProjectConfig::load(&root);
        state.set_workspace_config(folder.clone(), config.clone());
        let summary = index_workspace_root(client, state, &config, &folder, &mut seen).await;
        indexed_total += summary.indexed;
        skipped_total += summary.skipped;
        if summary.truncated {
            truncated_roots += 1;
        }
    }

    info!(
        "Indexed {} workspace ST files (skipped={})",
        indexed_total, skipped_total
    );
    if indexed_total > 0 || truncated_roots > 0 {
        let mut message = format!(
            "Indexed {} workspace ST files (skipped={})",
            indexed_total, skipped_total
        );
        if truncated_roots > 0 {
            message.push_str(&format!(" (budget hit in {truncated_roots} roots)"));
        }
        client.log_message(MessageType::INFO, message).await;
    }
}

pub fn index_workspace_background_with_refresh(client: Client, state: Arc<ServerState>) {
    tokio::spawn(async move {
        state.run_background(index_workspace(&client, &state)).await;
        // Unblock any did_open / pull-diagnostic handlers that were waiting for the
        // first index pass. After this point, cross-file types are visible and any
        // diagnostics published or returned will reflect the full project state.
        state.mark_index_first_pass_done();
        refresh_diagnostics(&client, &state).await;
        refresh_semantic_tokens(&client, &state).await;
    });
}

pub fn did_change_configuration(state: &ServerState, params: DidChangeConfigurationParams) {
    state.set_config(params.settings);
    state.record_activity();
    info!("Updated workspace configuration");
}

pub fn will_rename_files(state: &ServerState, params: RenameFilesParams) -> Option<WorkspaceEdit> {
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    let documents = state.documents();

    for change in params.files {
        let Ok(old_uri) = Url::parse(&change.old_uri) else {
            continue;
        };
        let Ok(new_uri) = Url::parse(&change.new_uri) else {
            continue;
        };

        let Some(old_stem) = lsp_utils::st_file_stem(&old_uri) else {
            continue;
        };
        let Some(new_stem) = lsp_utils::st_file_stem(&new_uri) else {
            continue;
        };
        if old_stem == new_stem {
            continue;
        }
        if !is_valid_identifier(&new_stem) || is_reserved_keyword(&new_stem) {
            continue;
        }

        let Some(doc) = state.ensure_document(&old_uri) else {
            continue;
        };

        let rename_result = state.with_database(|db| {
            let symbols = db.file_symbols(doc.file_id);
            let mut candidate = None;
            for symbol in symbols.iter() {
                if !lsp_utils::is_primary_pou_symbol_kind(&symbol.kind) || symbol.origin.is_some() {
                    continue;
                }
                // Built-in stdlib symbols are registered with empty ranges.
                if symbol.range.is_empty() {
                    continue;
                }
                if candidate.is_some() {
                    return None;
                }
                candidate = Some(symbol);
            }
            let symbol = candidate?;
            if !symbol.name.eq_ignore_ascii_case(old_stem.as_str()) {
                return None;
            }
            if has_conflict(&symbols, symbol.id, &new_stem) {
                return None;
            }
            let references = trust_ide::references::find_references(
                db,
                doc.file_id,
                symbol.range.start(),
                trust_ide::references::FindReferencesOptions {
                    include_declaration: true,
                },
            );
            if references.is_empty() {
                return None;
            }
            let mut result = trust_ide::rename::RenameResult::new();
            for reference in references {
                result.add_edit(
                    reference.file_id,
                    trust_ide::rename::TextEdit {
                        range: reference.range,
                        new_text: new_stem.to_string(),
                    },
                );
            }
            Some(result)
        });

        if let Some(rename_result) = rename_result {
            if let Some(rename_changes) = lsp_utils::rename_result_to_changes(state, rename_result)
            {
                for (uri, edits) in rename_changes {
                    changes.entry(uri).or_default().extend(edits);
                }
            }
        }

        let namespace_rename = state.with_database(|db| {
            let symbols = db.file_symbols(doc.file_id);
            let namespace_id = find_namespace_symbol(&symbols, old_stem.as_str())?;
            if has_conflict(&symbols, namespace_id, &new_stem) {
                return None;
            }
            let full_path = namespace_full_path(&symbols, namespace_id)?;
            let references = trust_ide::references::find_references(
                db,
                doc.file_id,
                symbols.get(namespace_id)?.range.start(),
                trust_ide::references::FindReferencesOptions {
                    include_declaration: true,
                },
            );
            if references.is_empty() {
                return None;
            }
            let mut result = trust_ide::rename::RenameResult::new();
            for reference in references {
                result.add_edit(
                    reference.file_id,
                    trust_ide::rename::TextEdit {
                        range: reference.range,
                        new_text: new_stem.to_string(),
                    },
                );
            }
            Some((result, full_path))
        });

        if let Some((namespace_result, namespace_path)) = namespace_rename {
            if let Some(rename_changes) =
                lsp_utils::rename_result_to_changes(state, namespace_result)
            {
                for (uri, edits) in rename_changes {
                    changes.entry(uri).or_default().extend(edits);
                }
            }

            let using_changes = state.with_database(|db| {
                using_directive_edits(db, &documents, &namespace_path, &new_stem)
            });
            for (uri, edits) in using_changes {
                changes.entry(uri).or_default().extend(edits);
            }
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }
}

struct IndexSummary {
    indexed: usize,
    skipped: usize,
    truncated: bool,
}

struct IndexThrottle {
    idle_ms: u64,
    active_ms: u64,
    max_ms: u64,
    active_window_ms: u64,
    ema_ms: f64,
}

impl IndexThrottle {
    fn new(config: &ProjectConfig) -> Self {
        Self {
            idle_ms: config.indexing.throttle_idle_ms,
            active_ms: config.indexing.throttle_active_ms,
            max_ms: config.indexing.throttle_max_ms,
            active_window_ms: config.indexing.throttle_active_window_ms,
            ema_ms: 0.0,
        }
    }

    async fn pause(&mut self, state: &ServerState, elapsed: Duration) {
        if self.idle_ms == 0 && self.active_ms == 0 && self.max_ms == 0 {
            return;
        }
        let elapsed_ms = elapsed.as_millis() as f64;
        if elapsed_ms > 0.0 {
            self.ema_ms = if self.ema_ms == 0.0 {
                elapsed_ms
            } else {
                self.ema_ms * 0.7 + elapsed_ms * 0.3
            };
        }

        let base = if state.activity_age_ms() <= self.active_window_ms {
            self.active_ms
        } else {
            self.idle_ms
        };
        let mut delay = base;
        if base > 0 {
            delay = delay.saturating_add(self.ema_ms.round() as u64);
        }
        if self.max_ms > 0 {
            delay = delay.min(self.max_ms);
        }
        if delay > 0 {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }
}

async fn index_workspace_root(
    client: &Client,
    state: &ServerState,
    config: &ProjectConfig,
    root_uri: &Url,
    seen: &mut FxHashSet<PathBuf>,
) -> IndexSummary {
    let mut files = Vec::new();
    collect_workspace_files(config, &mut files);
    files.sort();
    files.dedup();

    let total = files.len();
    if total == 0 {
        return IndexSummary {
            indexed: 0,
            skipped: 0,
            truncated: false,
        };
    }

    let cache_dir = config.index_cache_dir();
    let mut cache = cache_dir
        .as_ref()
        .map(|dir| IndexCache::load_or_default(dir));

    let progress = if state.work_done_progress() {
        start_progress(client, root_uri, total).await
    } else {
        None
    };

    let start = Instant::now();
    let mut indexed = 0usize;
    let mut skipped = 0usize;
    let mut truncated = false;
    let mut indexed_since_budget_check = 0usize;
    const BUDGET_CHECK_INTERVAL: usize = 256;
    let max_files = config.indexing.max_files;
    let max_ms = config.indexing.max_ms;
    let mut last_percent = 0u32;
    let mut throttle = IndexThrottle::new(config);

    for (idx, path) in files.iter().enumerate() {
        if let Some(max) = max_files {
            if indexed >= max {
                truncated = true;
                break;
            }
        }
        if let Some(max) = max_ms {
            if start.elapsed() >= Duration::from_millis(max) {
                truncated = true;
                break;
            }
        }

        if !seen.insert(path.clone()) {
            skipped += 1;
            report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
            continue;
        }

        let Some(uri) = path_to_uri(path) else {
            skipped += 1;
            report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
            continue;
        };

        if let Some(doc) = state.get_document(&uri) {
            if doc.is_open {
                skipped += 1;
                report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
                continue;
            }
        }

        let step_start = Instant::now();
        if let Some(cache) = cache.as_ref() {
            if let Some(cached) = cache.content_for_path(path) {
                if state
                    .index_document_deferred_budget(uri, cached.to_string())
                    .is_some()
                {
                    indexed += 1;
                    indexed_since_budget_check += 1;
                } else {
                    skipped += 1;
                }
                if indexed_since_budget_check >= BUDGET_CHECK_INTERVAL {
                    state.apply_memory_budget();
                    indexed_since_budget_check = 0;
                }
                report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
                throttle.pause(state, step_start.elapsed()).await;
                continue;
            }
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            skipped += 1;
            report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
            continue;
        };
        if let Some(cache) = cache.as_mut() {
            cache.update_from_content(path, content.clone());
        }
        if state.index_document_deferred_budget(uri, content).is_some() {
            indexed += 1;
            indexed_since_budget_check += 1;
        } else {
            skipped += 1;
        }
        if indexed_since_budget_check >= BUDGET_CHECK_INTERVAL {
            state.apply_memory_budget();
            indexed_since_budget_check = 0;
        }

        report_progress(client, &progress, idx + 1, total, &mut last_percent).await;
        throttle.pause(state, step_start.elapsed()).await;
    }

    state.apply_memory_budget();

    if let (Some(cache), Some(dir)) = (cache.as_mut(), cache_dir.as_ref()) {
        cache.retain_paths(&files);
        let _ = cache.save(dir);
    }

    end_progress(client, &progress, indexed, truncated).await;

    IndexSummary {
        indexed,
        skipped,
        truncated,
    }
}

include!("workspace/indexing_and_throttle.rs");
include!("workspace/namespace_and_rename.rs");
