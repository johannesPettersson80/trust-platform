fn collect_workspace_files(config: &ProjectConfig, out: &mut Vec<PathBuf>) {
    for root in config.indexing_roots() {
        collect_st_files(&root, out);
    }
}

fn collect_st_files(root: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() && is_st_file(&path) {
                out.push(path);
            }
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(name, ".git" | ".hg" | ".svn" | "node_modules" | "target")
}

async fn start_progress(client: &Client, root_uri: &Url, total: usize) -> Option<ProgressToken> {
    let token = ProgressToken::String(format!("trustlsp-index-{}", root_uri));
    let params = WorkDoneProgressCreateParams {
        token: token.clone(),
    };
    if client
        .send_request::<WorkDoneProgressCreate>(params)
        .await
        .is_err()
    {
        return None;
    }

    let title = uri_to_path(root_uri)
        .and_then(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .map(|name| format!("Indexing {name}"))
        .unwrap_or_else(|| "Indexing workspace".to_string());

    let begin = WorkDoneProgressBegin {
        title,
        cancellable: Some(false),
        message: Some(format!("0/{total} files")),
        percentage: Some(0),
    };
    let _ = client
        .send_notification::<Progress>(ProgressParams {
            token: token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(begin)),
        })
        .await;
    Some(token)
}

async fn report_progress(
    client: &Client,
    token: &Option<ProgressToken>,
    processed: usize,
    total: usize,
    last_percent: &mut u32,
) {
    let Some(token) = token else {
        return;
    };
    if total == 0 {
        return;
    }
    let percent = ((processed * 100) / total) as u32;
    if percent <= *last_percent && processed < total {
        return;
    }
    *last_percent = percent;
    let report = WorkDoneProgressReport {
        cancellable: Some(false),
        message: Some(format!("{processed}/{total} files")),
        percentage: Some(percent),
    };
    let _ = client
        .send_notification::<Progress>(ProgressParams {
            token: token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(report)),
        })
        .await;
}

async fn end_progress(
    client: &Client,
    token: &Option<ProgressToken>,
    indexed: usize,
    truncated: bool,
) {
    let Some(token) = token else {
        return;
    };
    let message = if truncated {
        format!("Indexed {indexed} files (budget limit reached)")
    } else {
        format!("Indexed {indexed} files")
    };
    let end = WorkDoneProgressEnd {
        message: Some(message),
    };
    let _ = client
        .send_notification::<Progress>(ProgressParams {
            token: token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(end)),
        })
        .await;
}

pub async fn did_rename_files(
    client: &Client,
    state: &Arc<ServerState>,
    params: RenameFilesParams,
) {
    let mut indexed = 0usize;
    let mut removed = 0usize;
    let mut config_changed = false;
    let mut cache_by_dir: HashMap<PathBuf, IndexCache> = HashMap::new();
    let mut dirty_cache_dirs: HashSet<PathBuf> = HashSet::new();

    for file in params.files {
        let old_uri = Url::parse(&file.old_uri).ok();
        let new_uri = Url::parse(&file.new_uri).ok();
        let old_path = old_uri.as_ref().and_then(uri_to_path);
        let new_path = new_uri.as_ref().and_then(uri_to_path);

        if old_path.as_ref().is_some_and(|path| is_config_file(path))
            || new_path.as_ref().is_some_and(|path| is_config_file(path))
        {
            config_changed = true;
        }

        let mut renamed_open_doc = false;
        let mut open_content: Option<String> = None;
        if let (Some(old_uri), Some(new_uri)) = (old_uri.as_ref(), new_uri.as_ref()) {
            if let Some(doc) = state.get_document(old_uri) {
                if doc.is_open {
                    renamed_open_doc = true;
                    open_content = Some(doc.content.clone());
                    let _ = state.rename_document(old_uri, new_uri);
                }
            }
        }

        if let (Some(old_uri), Some(path)) = (old_uri.as_ref(), old_path.as_ref()) {
            if is_st_file(path) {
                let cache_dir = state
                    .workspace_config_for_uri(old_uri)
                    .and_then(|config| config.index_cache_dir());
                if let Some(dir) = cache_dir.clone() {
                    let cache = cache_by_dir
                        .entry(dir.clone())
                        .or_insert_with(|| IndexCache::load_or_default(&dir));
                    cache.remove_path(path);
                    dirty_cache_dirs.insert(dir);
                }
                if !renamed_open_doc && state.remove_document(old_uri).is_some() {
                    removed += 1;
                }
            }
        }

        if let (Some(new_uri), Some(path)) = (new_uri.as_ref(), new_path.as_ref()) {
            if is_st_file(path) {
                let cache_dir = state
                    .workspace_config_for_uri(new_uri)
                    .and_then(|config| config.index_cache_dir());
                let content = if let Some(content) = open_content.clone() {
                    content
                } else {
                    let Ok(content) = std::fs::read_to_string(path) else {
                        continue;
                    };
                    content
                };
                if let Some(dir) = cache_dir.clone() {
                    let cache = cache_by_dir
                        .entry(dir.clone())
                        .or_insert_with(|| IndexCache::load_or_default(&dir));
                    cache.update_from_content(path, content.clone());
                    dirty_cache_dirs.insert(dir);
                }
                if !renamed_open_doc && state.index_document(new_uri.clone(), content).is_some() {
                    indexed += 1;
                }
            }
        }

        if renamed_open_doc {
            indexed = indexed.saturating_add(1);
        }
    }

    for dir in dirty_cache_dirs {
        if let Some(cache) = cache_by_dir.get(&dir) {
            let _ = cache.save(&dir);
        }
    }

    let mut diagnostics_refresh = indexed > 0 || removed > 0;
    if diagnostics_refresh {
        client
            .log_message(
                MessageType::INFO,
                format!("Workspace rename: indexed={indexed} removed={removed}"),
            )
            .await;
    }

    if config_changed {
        client
            .log_message(
                MessageType::INFO,
                "Workspace config renamed; reindexing".to_string(),
            )
            .await;
        index_workspace_background_with_refresh(client.clone(), Arc::clone(state));
        diagnostics_refresh = false;
    }

    if diagnostics_refresh {
        refresh_diagnostics(client, state).await;
    }
}

pub async fn did_change_watched_files(
    client: &Client,
    state: &Arc<ServerState>,
    params: DidChangeWatchedFilesParams,
) {
    let mut indexed = 0usize;
    let mut removed = 0usize;
    let mut config_changed = false;
    let mut cache_by_dir: HashMap<PathBuf, IndexCache> = HashMap::new();
    let mut dirty_cache_dirs: HashSet<PathBuf> = HashSet::new();
    for change in params.changes {
        let Some(path) = uri_to_path(&change.uri) else {
            continue;
        };
        if change.typ == FileChangeType::DELETED {
            removed += state.remove_document_tree(&change.uri);
        }
        if is_config_file(&path) {
            config_changed = true;
            continue;
        }
        if !is_st_file(&path) {
            continue;
        }
        let cache_dir = state
            .workspace_config_for_uri(&change.uri)
            .and_then(|config| config.index_cache_dir());

        match change.typ {
            FileChangeType::CREATED | FileChangeType::CHANGED => {
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Some(dir) = cache_dir.clone() {
                    let cache = cache_by_dir
                        .entry(dir.clone())
                        .or_insert_with(|| IndexCache::load_or_default(&dir));
                    cache.update_from_content(&path, content.clone());
                    dirty_cache_dirs.insert(dir);
                }
                if state.index_document(change.uri.clone(), content).is_some() {
                    indexed += 1;
                }
            }
            FileChangeType::DELETED => {
                if let Some(dir) = cache_dir.clone() {
                    let cache = cache_by_dir
                        .entry(dir.clone())
                        .or_insert_with(|| IndexCache::load_or_default(&dir));
                    cache.remove_path(&path);
                    dirty_cache_dirs.insert(dir);
                }
            }
            _ => {}
        }
    }

    for dir in dirty_cache_dirs {
        if let Some(cache) = cache_by_dir.get(&dir) {
            let _ = cache.save(&dir);
        }
    }

    let mut diagnostics_refresh = indexed > 0 || removed > 0;
    if diagnostics_refresh {
        client
            .log_message(
                MessageType::INFO,
                format!("Workspace update: indexed={indexed} removed={removed}"),
            )
            .await;
    }

    if config_changed {
        client
            .log_message(
                MessageType::INFO,
                "Workspace config changed; reindexing".to_string(),
            )
            .await;
        index_workspace_background_with_refresh(client.clone(), Arc::clone(state));
        diagnostics_refresh = false;
    }

    if diagnostics_refresh {
        refresh_diagnostics(client, state).await;
    }
}
