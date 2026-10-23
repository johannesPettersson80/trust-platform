use super::*;

impl WebIdeState {
    pub fn open_source(
        &self,
        session_token: &str,
        path: &str,
    ) -> Result<IdeFileSnapshot, IdeError> {
        let normalized = normalize_workspace_file_path(path)?;
        let source_path = self.resolve_source_path(&normalized)?;
        let disk_content = std::fs::read_to_string(&source_path)
            .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;
        if disk_content.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "source file exceeds limit ({} > {} bytes)",
                    disk_content.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let role = {
            let session = self.ensure_session(&mut guard, session_token, now)?;
            session.open_paths.insert(normalized.clone());
            session.role
        };

        let entry = guard
            .documents
            .entry(normalized.clone())
            .or_insert_with(|| IdeDocumentEntry {
                content: disk_content.clone(),
                version: 1,
                opened_by: BTreeSet::new(),
            });
        if entry.content != disk_content {
            entry.content = disk_content.clone();
            entry.version = entry.version.saturating_add(1);
        }
        entry.opened_by.insert(session_token.to_string());

        let content = if super::super::io_secrets::is_io_config(&source_path) {
            super::super::io_secrets::redact(&disk_content).map_err(|_| {
                IdeError::new(IdeErrorKind::InvalidInput, "invalid I/O configuration")
            })?
        } else {
            disk_content
        };
        Ok(IdeFileSnapshot {
            path: normalized,
            content,
            version: entry.version,
            read_only: !matches!(role, IdeRole::Editor),
        })
    }

    pub fn apply_source(
        &self,
        session_token: &str,
        path: &str,
        expected_version: u64,
        content: String,
        write_enabled: bool,
    ) -> Result<IdeWriteResult, IdeError> {
        if !write_enabled {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "web IDE authoring is disabled in current runtime mode",
            ));
        }
        if content.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "payload exceeds limit ({} > {} bytes)",
                    content.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }

        let normalized = normalize_workspace_file_path(path)?;
        let source_path = self.resolve_source_path(&normalized)?;
        let disk_content = std::fs::read_to_string(&source_path)
            .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let role = {
            let session = self.ensure_session(&mut guard, session_token, now)?;
            session.open_paths.insert(normalized.clone());
            session.role
        };
        if !matches!(role, IdeRole::Editor) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "session role does not allow edits",
            ));
        }

        let next_version = {
            let entry = guard
                .documents
                .entry(normalized.clone())
                .or_insert_with(|| IdeDocumentEntry {
                    content: disk_content.clone(),
                    version: 1,
                    opened_by: BTreeSet::new(),
                });
            if entry.content != disk_content {
                entry.content = disk_content;
                entry.version = entry.version.saturating_add(1);
            }
            if entry.version != expected_version {
                return Err(IdeError::conflict(entry.version));
            }

            let content = if super::super::io_secrets::is_io_config(&source_path) {
                super::super::io_secrets::restore(&content, &entry.content).map_err(|_| {
                    IdeError::new(
                        IdeErrorKind::InvalidInput,
                        "invalid I/O configuration or missing stored secret",
                    )
                })?
            } else {
                content
            };
            std::fs::write(&source_path, &content).map_err(|err| {
                IdeError::new(IdeErrorKind::Internal, format!("write failed: {err}"))
            })?;

            entry.content = content;
            entry.version = entry.version.saturating_add(1);
            entry.opened_by.insert(session_token.to_string());
            entry.version
        };
        self.record_fs_audit_event(
            &mut guard,
            session_token,
            "write_file",
            normalized.as_str(),
            now,
        );
        guard.analysis_cache.clear();

        Ok(IdeWriteResult {
            path: normalized,
            version: next_version,
        })
    }

    pub fn format_source(
        &self,
        session_token: &str,
        path: &str,
        content: Option<String>,
    ) -> Result<IdeFormatResult, IdeError> {
        let normalized = normalize_source_path(path)?;
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let _ = self.ensure_session(&mut guard, session_token, now)?;
        drop(guard);

        let current = if let Some(content) = content {
            content
        } else {
            let source_path = self.resolve_source_path(&normalized)?;
            std::fs::read_to_string(&source_path)
                .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?
        };
        if current.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "source file exceeds limit ({} > {} bytes)",
                    current.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }
        let formatted = format_structured_text_document(current.as_str());
        let changed = formatted != current;
        Ok(IdeFormatResult {
            path: normalized,
            content: formatted,
            changed,
        })
    }
}
