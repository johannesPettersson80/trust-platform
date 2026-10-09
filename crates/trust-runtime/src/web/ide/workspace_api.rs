use super::*;

impl WebIdeState {
    pub fn list_sources(&self, session_token: &str) -> Result<Vec<String>, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_session(&mut guard, session_token, now)?;
        drop(guard);

        let root = self.workspace_root()?;
        let mut list = Vec::new();
        collect_workspace_files(&root, &PathBuf::new(), &mut list)?;
        list.sort();
        Ok(list)
    }

    pub fn list_tree(&self, session_token: &str) -> Result<Vec<IdeTreeNode>, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_session(&mut guard, session_token, now)?;
        drop(guard);

        let root = self.workspace_root()?;
        let nodes = collect_workspace_tree(&root, &PathBuf::new())?;
        Ok(nodes)
    }

    pub fn require_editor_session(&self, session_token: &str) -> Result<(), IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let _ = self.ensure_editor_session(&mut guard, session_token, now)?;
        Ok(())
    }

    pub fn create_entry(
        &self,
        session_token: &str,
        path: &str,
        is_directory: bool,
        content: Option<String>,
        write_enabled: bool,
    ) -> Result<IdeFsResult, IdeError> {
        if !write_enabled {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "web IDE authoring is disabled in current runtime mode",
            ));
        }

        let normalized = if is_directory {
            normalize_workspace_path(path, false)?
        } else {
            normalize_workspace_file_path(path)?
        };
        let resolved = self.resolve_workspace_path(&normalized)?;
        if resolved.exists() {
            return Err(IdeError::new(
                IdeErrorKind::Conflict,
                "target path already exists",
            ));
        }

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_editor_session(&mut guard, session_token, now)?;

        if is_directory {
            std::fs::create_dir_all(&resolved).map_err(|err| {
                IdeError::new(IdeErrorKind::Internal, format!("mkdir failed: {err}"))
            })?;
            guard.analysis_cache.clear();
            self.record_fs_audit_event(
                &mut guard,
                session_token,
                "create_directory",
                normalized.as_str(),
                now,
            );
            return Ok(IdeFsResult {
                path: normalized,
                kind: "directory".to_string(),
                version: None,
            });
        }

        let payload = content.unwrap_or_default();
        if payload.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "payload exceeds limit ({} > {} bytes)",
                    payload.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }

        let payload = if super::super::io_secrets::is_io_config(&resolved) {
            super::super::io_secrets::restore(&payload, "").map_err(|_| {
                IdeError::new(
                    IdeErrorKind::InvalidInput,
                    "invalid I/O configuration or missing stored secret",
                )
            })?
        } else {
            payload
        };
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                IdeError::new(
                    IdeErrorKind::Internal,
                    format!("create parent directory failed: {err}"),
                )
            })?;
        }
        std::fs::write(&resolved, &payload).map_err(|err| {
            IdeError::new(IdeErrorKind::Internal, format!("create file failed: {err}"))
        })?;

        let version = {
            let entry = guard
                .documents
                .entry(normalized.clone())
                .or_insert_with(|| IdeDocumentEntry {
                    content: payload.clone(),
                    version: 1,
                    opened_by: BTreeSet::new(),
                });
            entry.content = payload;
            entry.version = entry.version.max(1);
            entry.opened_by.insert(session_token.to_string());
            entry.version
        };
        self.record_fs_audit_event(
            &mut guard,
            session_token,
            "create_file",
            normalized.as_str(),
            now,
        );
        guard.analysis_cache.clear();

        Ok(IdeFsResult {
            path: normalized,
            kind: "file".to_string(),
            version: Some(version),
        })
    }

    pub fn rename_entry(
        &self,
        session_token: &str,
        path: &str,
        new_path: &str,
        write_enabled: bool,
    ) -> Result<IdeFsResult, IdeError> {
        if !write_enabled {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "web IDE authoring is disabled in current runtime mode",
            ));
        }

        let old_norm = normalize_workspace_path(path, false)?;
        let new_norm = normalize_workspace_path(new_path, false)?;
        let old_resolved = self.resolve_workspace_path(&old_norm)?;
        let old_is_dir = old_resolved.is_dir();
        let new_resolved = self.resolve_workspace_path(&new_norm)?;
        if super::super::io_secrets::is_io_config(&old_resolved)
            && !super::super::io_secrets::is_io_config(&new_resolved)
        {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "I/O configuration must retain its io.toml filename",
            ));
        }
        if !old_resolved.exists() {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "source path not found",
            ));
        }
        if new_resolved.exists() {
            return Err(IdeError::new(
                IdeErrorKind::Conflict,
                "target path already exists",
            ));
        }
        if let Some(parent) = new_resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                IdeError::new(
                    IdeErrorKind::Internal,
                    format!("create parent directory failed: {err}"),
                )
            })?;
        }

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_editor_session(&mut guard, session_token, now)?;

        std::fs::rename(&old_resolved, &new_resolved).map_err(|err| {
            IdeError::new(IdeErrorKind::Internal, format!("rename failed: {err}"))
        })?;

        if old_is_dir {
            let mut remapped_docs = Vec::new();
            for key in guard.documents.keys().cloned().collect::<Vec<_>>() {
                if key == old_norm || key.starts_with(&format!("{old_norm}/")) {
                    let suffix = key.strip_prefix(&old_norm).unwrap_or_default();
                    let mapped = format!("{new_norm}{suffix}");
                    remapped_docs.push((key, mapped));
                }
            }
            for (old_key, mapped) in remapped_docs {
                if let Some(mut entry) = guard.documents.remove(&old_key) {
                    entry.version = entry.version.saturating_add(1);
                    guard.documents.insert(mapped, entry);
                }
            }
            for session in guard.sessions.values_mut() {
                let mut remapped = BTreeSet::new();
                for path in &session.open_paths {
                    if path == &old_norm || path.starts_with(&format!("{old_norm}/")) {
                        let suffix = path.strip_prefix(&old_norm).unwrap_or_default();
                        remapped.insert(format!("{new_norm}{suffix}"));
                    } else {
                        remapped.insert(path.clone());
                    }
                }
                session.open_paths = remapped;
            }
        } else if let Some(mut entry) = guard.documents.remove(&old_norm) {
            entry.version = entry.version.saturating_add(1);
            guard.documents.insert(new_norm.clone(), entry);
            for session in guard.sessions.values_mut() {
                if session.open_paths.remove(&old_norm) {
                    session.open_paths.insert(new_norm.clone());
                }
            }
        }

        guard.analysis_cache.clear();
        self.record_fs_audit_event(
            &mut guard,
            session_token,
            "rename_path",
            format!("{old_norm} -> {new_norm}").as_str(),
            now,
        );

        Ok(IdeFsResult {
            path: new_norm,
            kind: if old_is_dir {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            version: None,
        })
    }

    pub fn delete_entry(
        &self,
        session_token: &str,
        path: &str,
        write_enabled: bool,
    ) -> Result<IdeFsResult, IdeError> {
        if !write_enabled {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "web IDE authoring is disabled in current runtime mode",
            ));
        }
        let normalized = normalize_workspace_path(path, false)?;
        let resolved = self.resolve_workspace_path(&normalized)?;
        if !resolved.exists() {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "source path not found",
            ));
        }
        let is_dir = resolved.is_dir();

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_editor_session(&mut guard, session_token, now)?;

        if is_dir {
            std::fs::remove_dir_all(&resolved).map_err(|err| {
                IdeError::new(
                    IdeErrorKind::Internal,
                    format!("delete directory failed: {err}"),
                )
            })?;
        } else {
            std::fs::remove_file(&resolved).map_err(|err| {
                IdeError::new(IdeErrorKind::Internal, format!("delete file failed: {err}"))
            })?;
        }

        if is_dir {
            for key in guard.documents.keys().cloned().collect::<Vec<_>>() {
                if key == normalized || key.starts_with(&format!("{normalized}/")) {
                    guard.documents.remove(&key);
                }
            }
            for session in guard.sessions.values_mut() {
                session.open_paths.retain(|open| {
                    !(open == &normalized || open.starts_with(&format!("{normalized}/")))
                });
            }
        } else {
            guard.documents.remove(&normalized);
            for session in guard.sessions.values_mut() {
                session.open_paths.remove(&normalized);
            }
        }
        guard.analysis_cache.clear();
        self.record_fs_audit_event(
            &mut guard,
            session_token,
            "delete_path",
            normalized.as_str(),
            now,
        );

        Ok(IdeFsResult {
            path: normalized,
            kind: if is_dir {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            version: None,
        })
    }
}
