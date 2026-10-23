use super::*;

impl WebIdeState {
    pub(super) fn ensure_analysis_cache(
        &self,
        guard: &mut IdeStateInner,
        session_token: &str,
        active_path: &str,
        content_override: Option<&str>,
    ) -> Result<(), IdeError> {
        let now = (self.now)();
        let _ = self.ensure_session(guard, session_token, now)?;
        let root = self.workspace_root()?;

        let cache_key = session_token.to_string();
        let mut cache = guard.analysis_cache.remove(&cache_key).unwrap_or_default();
        let result = (|| -> Result<(), IdeError> {
            let refresh_due = !cache.initialized
                || now >= cache.next_refresh_at_secs
                || !cache.docs.contains_key(active_path);
            let mut docs_changed = false;

            if refresh_due {
                let mut files = Vec::new();
                collect_source_files(&root, &PathBuf::new(), &mut files)?;
                files.sort();

                let mut seen = BTreeSet::new();
                for rel_path in files {
                    let normalized = normalize_source_path(&rel_path)?;
                    seen.insert(normalized.clone());
                    let disk_path = root.join(&normalized);
                    let fingerprint = match source_fingerprint(&disk_path) {
                        Ok(value) => value,
                        Err(error) => {
                            if normalized == active_path {
                                return Err(error);
                            }
                            if cache.docs.remove(&normalized).is_some() {
                                docs_changed = true;
                            }
                            cache.fingerprints.remove(&normalized);
                            guard.documents.remove(&normalized);
                            continue;
                        }
                    };
                    let needs_reload = cache.fingerprints.get(&normalized).copied()
                        != Some(fingerprint)
                        || !cache.docs.contains_key(&normalized);

                    if needs_reload {
                        let text =
                            match read_source_with_limit(&disk_path, self.limits.max_file_bytes) {
                                Ok(value) => value,
                                Err(error) => {
                                    if normalized == active_path {
                                        return Err(error);
                                    }
                                    if cache.docs.remove(&normalized).is_some() {
                                        docs_changed = true;
                                    }
                                    cache.fingerprints.remove(&normalized);
                                    guard.documents.remove(&normalized);
                                    continue;
                                }
                            };
                        if cache.docs.get(&normalized).map(String::as_str) != Some(text.as_str()) {
                            docs_changed = true;
                        }
                        cache.docs.insert(normalized.clone(), text.clone());
                        cache.fingerprints.insert(normalized.clone(), fingerprint);
                        Self::upsert_tracked_document(guard, session_token, &normalized, text);
                    } else if let Some(existing) = cache.docs.get(&normalized).cloned() {
                        Self::upsert_tracked_document(guard, session_token, &normalized, existing);
                    }
                }

                let stale_paths = cache
                    .docs
                    .keys()
                    .filter(|path| !seen.contains(*path))
                    .cloned()
                    .collect::<Vec<_>>();
                if !stale_paths.is_empty() {
                    docs_changed = true;
                }
                for stale in stale_paths {
                    cache.docs.remove(&stale);
                    cache.fingerprints.remove(&stale);
                    guard.documents.remove(&stale);
                }

                cache.initialized = true;
                cache.next_refresh_at_secs =
                    now.saturating_add(ANALYSIS_CACHE_REFRESH_INTERVAL_SECS);
            }

            if let Some(override_text) = content_override {
                if override_text.len() > self.limits.max_file_bytes {
                    return Err(IdeError::new(
                        IdeErrorKind::TooLarge,
                        format!(
                            "source file exceeds limit ({} > {} bytes)",
                            override_text.len(),
                            self.limits.max_file_bytes
                        ),
                    ));
                }
                let Some(existing) = cache.docs.get_mut(active_path) else {
                    return Err(IdeError::new(
                        IdeErrorKind::NotFound,
                        "analysis file not found in project context",
                    ));
                };
                if existing != override_text {
                    *existing = override_text.to_string();
                    docs_changed = true;
                }
                Self::upsert_tracked_document(
                    guard,
                    session_token,
                    active_path,
                    override_text.to_string(),
                );
            } else if let Some(existing) = cache.docs.get(active_path).cloned() {
                Self::upsert_tracked_document(guard, session_token, active_path, existing);
            }

            if !cache.docs.contains_key(active_path) {
                return Err(IdeError::new(
                    IdeErrorKind::NotFound,
                    "analysis file not found in project context",
                ));
            }

            if docs_changed || !cache.engine_applied {
                let documents = cache
                    .docs
                    .iter()
                    .map(|(path, text)| DocumentInput {
                        uri: format!("memory:///{path}"),
                        text: text.clone(),
                    })
                    .collect::<Vec<_>>();
                cache
                    .engine
                    .replace_documents(documents)
                    .map_err(map_analysis_error)?;
                cache.engine_applied = true;
            }

            Ok(())
        })();
        guard.analysis_cache.insert(cache_key, cache);
        result
    }

    fn upsert_tracked_document(
        guard: &mut IdeStateInner,
        session_token: &str,
        path: &str,
        content: String,
    ) {
        let entry = guard
            .documents
            .entry(path.to_string())
            .or_insert_with(|| IdeDocumentEntry {
                content: content.clone(),
                version: 1,
                opened_by: BTreeSet::new(),
            });
        if entry.content != content {
            entry.content = content;
            entry.version = entry.version.saturating_add(1);
        }
        entry.opened_by.insert(session_token.to_string());
    }

    pub(super) fn analysis_context(
        &self,
        session_token: &str,
        path: &str,
        content: Option<String>,
    ) -> Result<AnalysisContext, IdeError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.analysis_context_with_guard(&mut guard, session_token, path, content.as_deref())
    }

    pub(super) fn analysis_context_for_all_files(
        &self,
        session_token: &str,
        content_override: Option<(&str, &str)>,
    ) -> Result<AnalysisContext, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let _ = self.ensure_session(&mut guard, session_token, now)?;
        self.build_analysis_context(&mut guard, session_token, content_override)
    }

    fn build_analysis_context(
        &self,
        guard: &mut IdeStateInner,
        session_token: &str,
        content_override: Option<(&str, &str)>,
    ) -> Result<AnalysisContext, IdeError> {
        let root = self.workspace_root()?;
        let mut files = Vec::new();
        collect_source_files(&root, &PathBuf::new(), &mut files)?;
        files.sort();

        let mut db = trust_hir::Database::new();
        let mut file_id_by_path = BTreeMap::new();
        let mut path_by_file_id = HashMap::new();
        let mut text_by_file = HashMap::new();

        for (index, rel_path) in files.iter().enumerate() {
            let normalized = normalize_source_path(rel_path)?;
            let disk_path = self.resolve_source_path(&normalized)?;
            let mut text = std::fs::read_to_string(&disk_path)
                .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;
            if let Some((override_path, override_text)) = content_override
                && normalized == override_path
            {
                text = override_text.to_string();
            }
            if text.len() > self.limits.max_file_bytes {
                return Err(IdeError::new(
                    IdeErrorKind::TooLarge,
                    format!(
                        "source file exceeds limit ({} > {} bytes)",
                        text.len(),
                        self.limits.max_file_bytes
                    ),
                ));
            }
            let file_id = FileId(index as u32);
            db.set_source_text(file_id, text.clone());
            file_id_by_path.insert(normalized.clone(), file_id);
            path_by_file_id.insert(file_id, normalized.clone());
            text_by_file.insert(file_id, text.clone());

            let entry = guard
                .documents
                .entry(normalized.clone())
                .or_insert_with(|| IdeDocumentEntry {
                    content: text.clone(),
                    version: 1,
                    opened_by: BTreeSet::new(),
                });
            if entry.content != text {
                entry.content = text;
                entry.version = entry.version.saturating_add(1);
            }
            entry.opened_by.insert(session_token.to_string());
        }

        Ok(AnalysisContext {
            db,
            file_id_by_path,
            path_by_file_id,
            text_by_file,
        })
    }

    pub(super) fn analysis_context_with_guard(
        &self,
        guard: &mut IdeStateInner,
        session_token: &str,
        path: &str,
        content: Option<&str>,
    ) -> Result<AnalysisContext, IdeError> {
        let now = (self.now)();
        let _ = self.ensure_session(guard, session_token, now)?;
        self.build_analysis_context(guard, session_token, content.map(|text| (path, text)))
    }
}
