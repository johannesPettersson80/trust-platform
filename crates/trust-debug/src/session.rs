use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use glob::glob;

use trust_hir::{db::FileId, SourceKey, SourceRegistry};
use trust_runtime::control::SourceFile as ControlSourceFile;
use trust_runtime::debug::{DebugBreakpoint, DebugControl, HitCondition, LogFragment};
#[cfg(test)]
use trust_runtime::harness::TestHarness;
use trust_runtime::harness::{
    parse_debug_expression, CompileError, CompileSession, SourceFile as HarnessSourceFile,
};
use trust_runtime::{Runtime, RuntimeMetadata};

use crate::protocol::{
    Breakpoint, SetBreakpointsArguments, SetBreakpointsResponseBody, Source, SourceBreakpoint,
};
use crate::runtime::DebugRuntime;

const MSG_MISSING_SOURCE: &str = "source path not provided";
const MSG_UNKNOWN_SOURCE: &str = "source not registered";
const MSG_PENDING_SOURCE: &str = "source pending (program not loaded yet)";
const MSG_INVALID_POSITION: &str = "line/column are 1-based";
const MSG_INVALID_LOG_MESSAGE: &str = "invalid log message";
const MSG_INVALID_CONDITION: &str = "invalid breakpoint condition";
const MSG_INVALID_HIT_CONDITION: &str = "invalid hit condition";
const MSG_NO_STATEMENT: &str = "no statement at or after requested location";
const DEFAULT_IGNORE_PRAGMAS: &[&str] = &["@trustlsp:runtime-ignore"];
const PRAGMA_SCAN_LINES: usize = 20;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub file_id: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct SourceOptions {
    pub root: Option<String>,
    pub include_globs: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub ignore_pragmas: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceOptionsUpdate {
    pub root: Option<String>,
    pub include_globs: Option<Vec<String>>,
    pub exclude_globs: Option<Vec<String>>,
    pub ignore_pragmas: Option<Vec<String>>,
}

impl SourceOptions {
    pub fn apply_update(&mut self, update: SourceOptionsUpdate) {
        if let Some(root) = update.root {
            let trimmed = root.trim();
            if !trimmed.is_empty() {
                self.root = Some(trimmed.to_string());
            }
        }
        if let Some(include_globs) = update.include_globs {
            self.include_globs = include_globs;
        }
        if let Some(exclude_globs) = update.exclude_globs {
            self.exclude_globs = exclude_globs;
        }
        if let Some(ignore_pragmas) = update.ignore_pragmas {
            self.ignore_pragmas = Some(ignore_pragmas);
        }
    }
}

#[derive(Debug)]
pub struct DebugSession {
    runtime: Arc<Mutex<Runtime>>,
    metadata: RuntimeMetadata,
    control: DebugControl,
    sources: HashMap<SourceKey, SourceFile>,
    source_registry: SourceRegistry,
    breakpoints: BreakpointManager,
    program_path: Option<String>,
    source_options: SourceOptions,
}

impl DebugSession {
    #[must_use]
    pub fn new(runtime: Runtime) -> Self {
        let mut runtime = runtime;
        let control = DebugControl::new();
        runtime.set_debug_control(control.clone());
        let metadata = runtime.metadata_snapshot();
        Self {
            runtime: Arc::new(Mutex::new(runtime)),
            metadata,
            control,
            sources: HashMap::new(),
            source_registry: SourceRegistry::new(),
            breakpoints: BreakpointManager::new(),
            program_path: None,
            source_options: SourceOptions::default(),
        }
    }

    #[must_use]
    pub fn with_control(runtime: Runtime, control: DebugControl) -> Self {
        let mut runtime = runtime;
        runtime.set_debug_control(control.clone());
        let metadata = runtime.metadata_snapshot();
        Self {
            runtime: Arc::new(Mutex::new(runtime)),
            metadata,
            control,
            sources: HashMap::new(),
            source_registry: SourceRegistry::new(),
            breakpoints: BreakpointManager::new(),
            program_path: None,
            source_options: SourceOptions::default(),
        }
    }

    #[must_use]
    pub fn take_breakpoint_report(&mut self) -> Option<String> {
        self.breakpoints.take_report()
    }

    pub fn register_source(
        &mut self,
        path: impl Into<String>,
        file_id: u32,
        text: impl Into<String>,
    ) {
        let path = path.into();
        let key = SourceKey::from_path(Path::new(&path));
        let file_id = self
            .source_registry
            .insert_with_id(key.clone(), FileId(file_id));
        self.sources.insert(
            key,
            SourceFile {
                file_id: file_id.0,
                text: text.into(),
            },
        );
    }

    /// Replace all sources with a single file.
    pub fn replace_single_source(&mut self, path: impl Into<String>, text: impl Into<String>) {
        self.sources.clear();
        self.source_registry.clear();
        self.register_source(path, 0, text);
    }

    /// Replace all sources with multiple files.
    pub fn replace_sources(&mut self, sources: &[(String, String)]) {
        self.sources.clear();
        self.source_registry.clear();
        for (idx, (path, text)) in sources.iter().enumerate() {
            self.register_source(path.clone(), idx as u32, text.clone());
        }
    }

    /// Remember the active program path (for reload).
    pub fn set_program_path(&mut self, path: impl Into<String>) {
        self.program_path = Some(path.into());
    }

    pub fn update_source_options(&mut self, update: SourceOptionsUpdate) {
        self.source_options.apply_update(update);
    }

    /// Reload the current program from disk.
    pub fn reload_program(&mut self, path: Option<&str>) -> Result<Vec<Breakpoint>, CompileError> {
        let path = match path {
            Some(path) => path.to_string(),
            None => self
                .program_path
                .clone()
                .ok_or_else(|| CompileError::new("no program path for reload"))?,
        };
        let sources = collect_sources(&path, &self.source_options)?;
        let (retained, current_time) = {
            let runtime = self
                .runtime
                .lock()
                .map_err(|_| CompileError::new("runtime lock poisoned"))?;
            (runtime.retain_snapshot(), runtime.current_time())
        };

        let source_files = sources
            .iter()
            .map(|(path, text)| HarnessSourceFile::with_path(path.clone(), text.clone()))
            .collect::<Vec<_>>();
        let compile = CompileSession::from_sources(source_files);
        let mut runtime = compile.build_runtime()?;
        runtime.set_debug_control(self.control.clone());
        runtime
            .apply_retain_snapshot(&retained)
            .map_err(|error| CompileError::new(error.to_string()))?;
        runtime.set_current_time(current_time);

        let metadata = runtime.metadata_snapshot();
        {
            let mut guard = self
                .runtime
                .lock()
                .map_err(|_| CompileError::new("runtime lock poisoned"))?;
            *guard = runtime;
        }
        self.metadata = metadata;
        self.replace_sources(&sources);

        // Ensure no stale breakpoints linger across reloads.
        self.control.clear_breakpoints();

        Ok(self.revalidate_breakpoints())
    }

    #[must_use]
    pub fn source_file_for_path(&self, path: &str) -> Option<&SourceFile> {
        let key = SourceKey::from_path(Path::new(path));
        self.sources
            .get(&key)
            .or_else(|| self.sources.get(&SourceKey::from_virtual(path.to_string())))
    }

    #[must_use]
    pub fn debug_control(&self) -> DebugControl {
        self.control.clone()
    }

    #[must_use]
    pub fn runtime_handle(&self) -> Arc<Mutex<Runtime>> {
        Arc::clone(&self.runtime)
    }

    #[must_use]
    pub fn metadata(&self) -> &RuntimeMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn source_for_file_id(&self, file_id: u32) -> Option<Source> {
        let key = self.source_registry.key_for_file_id(FileId(file_id))?;
        let path = key.display();
        Some(Source {
            name: Some(path.clone()),
            path: Some(path),
            source_reference: None,
        })
    }

    #[must_use]
    pub fn source_text_for_file_id(&self, file_id: u32) -> Option<&str> {
        let key = self.source_registry.key_for_file_id(FileId(file_id))?;
        self.sources.get(key).map(|file| file.text.as_str())
    }

    #[must_use]
    pub fn set_breakpoints(
        &mut self,
        args: &SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let context = BreakpointContext::new(&self.sources, &self.metadata, &self.control);
        self.breakpoints.set_breakpoints(context, args)
    }

    /// Revalidate previously requested breakpoints after reload.
    pub fn revalidate_breakpoints(&mut self) -> Vec<Breakpoint> {
        let context = BreakpointContext::new(&self.sources, &self.metadata, &self.control);
        self.breakpoints.revalidate_breakpoints(context)
    }

    #[cfg(test)]
    pub fn clear_requested_breakpoints(&mut self) {
        self.breakpoints.clear_requested();
    }
}

impl DebugRuntime for DebugSession {
    fn update_source_options(&mut self, update: SourceOptionsUpdate) {
        DebugSession::update_source_options(self, update);
    }

    fn set_program_path(&mut self, path: String) {
        DebugSession::set_program_path(self, path);
    }

    fn reload_program(&mut self, path: Option<&str>) -> Result<Vec<Breakpoint>, CompileError> {
        DebugSession::reload_program(self, path)
    }

    fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> SetBreakpointsResponseBody {
        DebugSession::set_breakpoints(self, args)
    }

    fn take_breakpoint_report(&mut self) -> Option<String> {
        DebugSession::take_breakpoint_report(self)
    }

    fn debug_control(&self) -> DebugControl {
        DebugSession::debug_control(self)
    }

    fn runtime_handle(&self) -> Arc<Mutex<Runtime>> {
        DebugSession::runtime_handle(self)
    }

    fn metadata(&self) -> &RuntimeMetadata {
        DebugSession::metadata(self)
    }

    fn source_file_for_path(&self, path: &str) -> Option<&SourceFile> {
        DebugSession::source_file_for_path(self, path)
    }

    fn source_for_file_id(&self, file_id: u32) -> Option<Source> {
        DebugSession::source_for_file_id(self, file_id)
    }

    fn source_text_for_file_id(&self, file_id: u32) -> Option<&str> {
        DebugSession::source_text_for_file_id(self, file_id)
    }

    fn control_sources(&self) -> Vec<ControlSourceFile> {
        self.sources
            .iter()
            .map(|(key, source)| ControlSourceFile {
                id: source.file_id,
                path: PathBuf::from(key.display()),
                text: source.text.clone(),
            })
            .collect()
    }
}

include!("session/breakpoint_manager.rs");
include!("session/source_and_parse_helpers.rs");
include!("session/tests.rs");
