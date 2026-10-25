//! The sources of a debug session and the project's declarations: the files a launch builds or an
//! attach names, how a client's paths map to them, and the tasks whose threads the client sees.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use glob::glob;

use trust_hir::{db::FileId, SourceKey, SourceRegistry};
use trust_runtime::control::SourceFile as ControlSourceFile;
use trust_runtime::debug::HitCondition;
use trust_runtime::harness::{CompileError, CompileSession, SourceFile as HarnessSourceFile};
use trust_runtime::RuntimeMetadata;

use crate::protocol::Source;
use crate::runtime::DebugRuntime;

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

/// The sources of a debug session, and the declarations of the project they belong to.
#[derive(Debug, Default)]
pub struct DebugSession {
    /// The declarations of the project's sources, once they compiled: its tasks, whose debugger
    /// threads the client sees (specification 13, "Multi-Task Thread Model").
    metadata: RuntimeMetadata,
    sources: HashMap<SourceKey, SourceFile>,
    source_registry: SourceRegistry,
    program_path: Option<String>,
    source_options: SourceOptions,
}

impl DebugSession {
    /// A session with no sources.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    #[must_use]
    pub fn source_file_for_path(&self, path: &str) -> Option<&SourceFile> {
        let key = SourceKey::from_path(Path::new(path));
        self.sources
            .get(&key)
            .or_else(|| self.sources.get(&SourceKey::from_virtual(path.to_string())))
    }

    #[must_use]
    pub fn metadata(&self) -> &RuntimeMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn source_for_file_id(&self, file_id: u32) -> Option<Source> {
        let key = self.source_registry.key_for_file_id(FileId(file_id))?;
        let path = key.display();
        let name = self.display_source_name(&path);
        Some(Source {
            name: Some(name),
            path: Some(path),
            source_reference: None,
        })
    }

    fn display_source_name(&self, path: &str) -> String {
        let source_path = Path::new(path);
        if let Some(root) = self.source_options.root.as_deref() {
            let root_path = canonicalize_lossy(Path::new(root));
            let candidate_path = canonicalize_lossy(source_path);
            if let Ok(relative) = candidate_path.strip_prefix(&root_path) {
                return display_path(relative);
            }
        }
        display_path(source_path)
    }

    #[must_use]
    pub fn source_text_for_file_id(&self, file_id: u32) -> Option<&str> {
        let key = self.source_registry.key_for_file_id(FileId(file_id))?;
        self.sources.get(key).map(|file| file.text.as_str())
    }

    /// The project's sources as the build takes them.
    fn project_files(&self) -> Result<Vec<HarnessSourceFile>, CompileError> {
        Ok(collect_project_sources(&self.source_options)?
            .into_iter()
            .map(|(path, text)| HarnessSourceFile::with_path(path, text))
            .collect())
    }
}

impl DebugRuntime for DebugSession {
    fn update_source_options(&mut self, update: SourceOptionsUpdate) {
        DebugSession::update_source_options(self, update);
    }

    fn set_program_path(&mut self, path: String) {
        DebugSession::set_program_path(self, path);
    }

    fn program_path(&self) -> Option<&str> {
        self.program_path.as_deref()
    }

    fn launch_sources(&mut self, path: &str) -> Result<Vec<(String, String)>, CompileError> {
        let sources = collect_sources(path, &self.source_options)?;
        self.replace_sources(&sources);
        let files = sources
            .iter()
            .map(|(path, text)| HarnessSourceFile::with_path(path.clone(), text.clone()))
            .collect();
        self.metadata = CompileSession::from_sources(files)
            .declarations()
            .map(|declarations| declarations.metadata().clone())
            .unwrap_or_default();
        Ok(sources)
    }

    fn load_project(&mut self) -> Result<(), CompileError> {
        let files = self.project_files()?;
        self.metadata = CompileSession::from_sources(files)
            .declarations()?
            .metadata()
            .clone();
        Ok(())
    }

    fn metadata(&self) -> &RuntimeMetadata {
        DebugSession::metadata(self)
    }

    fn project_expression_types(
        &self,
    ) -> Result<
        (
            trust_hir::types::TypeRegistry,
            trust_runtime::value::DateTimeProfile,
        ),
        CompileError,
    > {
        trust_runtime::harness::project_expression_types(&self.project_files()?)
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

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    if display.is_empty() {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    } else {
        display
    }
}

include!("session/source_and_parse_helpers.rs");
include!("session/tests.rs");
