use super::*;

impl WebIdeState {
    pub fn capabilities(&self, write_enabled: bool) -> WebIdeCapabilities {
        let enabled = self
            .active_project_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .is_some();
        WebIdeCapabilities {
            enabled,
            mode: if write_enabled {
                "authoring".to_string()
            } else {
                "read_only".to_string()
            },
            diagnostics_source: "trust-wasm-analysis in-process diagnostics/hover/completion"
                .to_string(),
            deployment_boundaries: vec![
                "Allowed file scope: <project>/**/* (hidden/system paths filtered)".to_string(),
                "Editor session requires engineer/admin web role".to_string(),
            ],
            security_model: vec![
                "Session bootstrap requires web auth (local or X-Trust-Token)".to_string(),
                "Per-session token required for IDE API calls (X-Trust-Ide-Session)".to_string(),
                "Session TTL uses sliding renewal while requests remain active".to_string(),
                "Optimistic concurrency via expected_version prevents blind overwrite".to_string(),
            ],
            limits: self.limits.clone(),
        }
    }

    pub fn project_selection(&self, session_token: &str) -> Result<IdeProjectSelection, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let _ = self.ensure_session(&mut guard, session_token, now)?;
        drop(guard);
        self.current_project_selection()
    }

    pub fn set_active_project(
        &self,
        session_token: &str,
        path: &str,
    ) -> Result<IdeProjectSelection, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let _ = self.ensure_editor_session(&mut guard, session_token, now)?;
        drop(guard);

        let selected = normalize_project_root(path)?;
        let canonical = selected.canonicalize().map_err(|_| {
            IdeError::new(
                IdeErrorKind::NotFound,
                "project root not found or inaccessible",
            )
        })?;
        if !canonical.is_dir() {
            return Err(IdeError::new(
                IdeErrorKind::InvalidInput,
                "project root must be a directory",
            ));
        }
        let approved_base = self.approved_project_base()?;
        if !canonical.starts_with(&approved_base) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "project root is outside approved project base",
            ));
        }

        {
            let mut project_guard = self
                .active_project_root
                .lock()
                .map_err(|_| IdeError::new(IdeErrorKind::Internal, "project root lock poisoned"))?;
            *project_guard = Some(canonical);
        }

        let mut state_guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        state_guard.documents.clear();
        state_guard.analysis_cache.clear();
        for session in state_guard.sessions.values_mut() {
            session.open_paths.clear();
        }
        drop(state_guard);

        self.current_project_selection()
    }

    pub fn create_project(
        &self,
        session_token: &str,
        name: &str,
        location: &str,
        template: &str,
    ) -> Result<IdeProjectSelection, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let session = self.ensure_session(&mut guard, session_token, now)?;
        if !matches!(session.role, IdeRole::Editor) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "session role does not allow project creation",
            ));
        }
        drop(guard);

        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(IdeError::new(
                IdeErrorKind::InvalidInput,
                "project name is required",
            ));
        }
        if trimmed_name.contains('/') || trimmed_name.contains('\\') || trimmed_name.contains("..")
        {
            return Err(IdeError::new(
                IdeErrorKind::InvalidInput,
                "project name must not contain path separators or '..'",
            ));
        }

        let base = normalize_project_root(location)?;
        let project_dir = base.join(trimmed_name);

        if project_dir.exists() {
            return Err(IdeError::new(
                IdeErrorKind::Conflict,
                "a project directory with this name already exists",
            ));
        }

        std::fs::create_dir_all(&project_dir).map_err(|err| {
            IdeError::new(
                IdeErrorKind::Internal,
                format!("failed to create project directory: {err}"),
            )
        })?;

        let main_st_content = project_template_source(template, trimmed_name);
        let main_path = project_dir.join("main.st");
        std::fs::write(&main_path, main_st_content).map_err(|err| {
            IdeError::new(
                IdeErrorKind::Internal,
                format!("failed to write main.st: {err}"),
            )
        })?;

        for (relative, content) in project_template_extra_sources(template) {
            let file_path = project_dir.join(relative.as_str());
            std::fs::write(&file_path, content).map_err(|err| {
                IdeError::new(
                    IdeErrorKind::Internal,
                    format!("failed to write {relative}: {err}"),
                )
            })?;
        }

        let resource_name = smol_str::SmolStr::new(trimmed_name);
        let runtime_toml = crate::bundle_template::render_runtime_toml(&resource_name, 100);
        std::fs::write(project_dir.join("runtime.toml"), runtime_toml).map_err(|err| {
            IdeError::new(
                IdeErrorKind::Internal,
                format!("failed to write runtime.toml: {err}"),
            )
        })?;

        let io_template = crate::bundle_template::build_io_config_auto("loopback")
            .map_err(|err| IdeError::new(IdeErrorKind::Internal, format!("io template: {err}")))?;
        let io_toml = crate::bundle_template::render_io_toml(&io_template);
        std::fs::write(project_dir.join("io.toml"), io_toml).map_err(|err| {
            IdeError::new(
                IdeErrorKind::Internal,
                format!("failed to write io.toml: {err}"),
            )
        })?;

        let project_path = project_dir.to_string_lossy().to_string();
        self.set_active_project(session_token, &project_path)
    }

    pub fn active_project_root(&self) -> Option<PathBuf> {
        self.active_project_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn approved_project_base(&self) -> Result<PathBuf, IdeError> {
        let base = if let Some(startup_root) = self.startup_project_root.as_ref() {
            let canonical = startup_root.canonicalize().map_err(|_| {
                IdeError::new(
                    IdeErrorKind::NotFound,
                    "approved project base is missing or inaccessible",
                )
            })?;
            canonical
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(canonical)
        } else {
            std::env::current_dir().map_err(|_| {
                IdeError::new(
                    IdeErrorKind::Internal,
                    "current directory unavailable for project-open base",
                )
            })?
        };
        base.canonicalize().map_err(|_| {
            IdeError::new(
                IdeErrorKind::NotFound,
                "approved project base is missing or inaccessible",
            )
        })
    }

    fn current_project_selection(&self) -> Result<IdeProjectSelection, IdeError> {
        let active_project = self
            .active_project_root
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "project root lock poisoned"))?
            .clone()
            .map(pathbuf_to_display);
        let startup_project = self.startup_project_root.clone().map(pathbuf_to_display);
        Ok(IdeProjectSelection {
            active_project,
            startup_project,
        })
    }

    pub fn create_session(&self, role: IdeRole) -> Result<IdeSession, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        prune_expired(&mut guard, now);
        if guard.sessions.len() >= self.limits.max_sessions {
            let oldest_token = guard
                .sessions
                .iter()
                .min_by_key(|(_, session)| session.expires_at)
                .map(|(token, _)| token.clone());
            if let Some(token) = oldest_token {
                remove_session(&mut guard, token.as_str());
            }
        }
        if guard.sessions.len() >= self.limits.max_sessions {
            return Err(IdeError::new(
                IdeErrorKind::LimitExceeded,
                "too many active IDE sessions",
            ));
        }

        let token = generate_token();
        let expires_at = now.saturating_add(self.limits.session_ttl_secs);
        guard.sessions.insert(
            token.clone(),
            IdeSessionEntry {
                role,
                expires_at,
                open_paths: BTreeSet::new(),
            },
        );

        Ok(IdeSession {
            token,
            role: role.as_str().to_string(),
            expires_at,
        })
    }
}
