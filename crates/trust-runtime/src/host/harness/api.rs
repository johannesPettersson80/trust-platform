//! Public harness build helpers.

#![allow(missing_docs)]

use super::build;
use super::types::{CompileError, SourceFile};
use crate::Runtime;
use smol_str::SmolStr;

/// Compile helper for runtime + bytecode builds.
#[derive(Debug, Clone)]
pub struct CompileSession {
    sources: Vec<SourceFile>,
    label_errors: bool,
    extra_program_instances: Vec<SmolStr>,
    instrumentation_errors: Vec<String>,
}

impl CompileSession {
    /// Build a compile session from a single source.
    pub fn from_source(source: impl Into<String>) -> Self {
        let sources = vec![SourceFile::new(source)];
        let (sources, instrumentation_errors) = instrument_sources_for_compile(sources);
        Self {
            sources,
            label_errors: false,
            extra_program_instances: Vec::new(),
            instrumentation_errors,
        }
    }

    /// Build a compile session from multiple sources.
    pub fn from_sources(sources: Vec<SourceFile>) -> Self {
        let label_errors = sources.len() > 1;
        let (sources, instrumentation_errors) = instrument_sources_for_compile(sources);
        Self {
            sources,
            label_errors,
            extra_program_instances: Vec::new(),
            instrumentation_errors,
        }
    }

    /// Enable/disable labeled errors (file path or index prefix).
    pub fn label_errors(mut self, label_errors: bool) -> Self {
        self.label_errors = label_errors;
        self
    }

    /// Register additional program instances at build time.
    ///
    /// This is used by `trust-dev test` so discovered `TEST_PROGRAM`s can be
    /// executed even when a `CONFIGURATION` is present, without changing normal
    /// configured runtime behavior.
    pub fn with_extra_program_instances<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SmolStr>,
    {
        self.extra_program_instances = names.into_iter().map(Into::into).collect();
        self
    }

    /// Access the registered sources.
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    /// Compile sources into a runtime.
    pub fn build_runtime(&self) -> Result<Runtime, CompileError> {
        self.ensure_instrumented()?;
        build::build_runtime_from_source_files(
            &self.sources,
            self.label_errors,
            &self.extra_program_instances,
        )
    }

    /// Compile sources into a bytecode module.
    pub fn build_bytecode_module(&self) -> Result<crate::bytecode::BytecodeModule, CompileError> {
        self.ensure_instrumented()?;
        build::build_bytecode_module_from_source_files(
            &self.sources,
            self.label_errors,
            &self.extra_program_instances,
        )
    }

    /// Compile sources into bytecode bytes.
    pub fn build_bytecode_bytes(&self) -> Result<Vec<u8>, CompileError> {
        let module = self.build_bytecode_module()?;
        module
            .encode()
            .map_err(|err| CompileError::new(err.to_string()))
    }

    fn ensure_instrumented(&self) -> Result<(), CompileError> {
        if self.instrumentation_errors.is_empty() {
            Ok(())
        } else {
            Err(CompileError::new(self.instrumentation_errors.join("\n")))
        }
    }
}

fn instrument_sources_for_compile(sources: Vec<SourceFile>) -> (Vec<SourceFile>, Vec<String>) {
    match crate::openot_authoring::try_instrument_source_files(&sources) {
        Ok(instrumented) => (instrumented, Vec::new()),
        Err(error) => (sources, vec![error]),
    }
}

/// Build a bytecode module from a single source file.
pub fn bytecode_module_from_source(
    source: &str,
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    CompileSession::from_source(source).build_bytecode_module()
}

/// Build a bytecode module from a single source file with an explicit path.
pub fn bytecode_module_from_source_with_path(
    source: &str,
    path: &str,
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    CompileSession::from_sources(vec![SourceFile::with_path(path, source)]).build_bytecode_module()
}

/// Build a bytecode module from multiple source files.
pub fn bytecode_module_from_sources(
    sources: &[&str],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    let source_files = sources
        .iter()
        .copied()
        .map(SourceFile::new)
        .collect::<Vec<_>>();
    CompileSession::from_sources(source_files).build_bytecode_module()
}

/// Build a bytecode module from multiple source files with explicit paths.
pub fn bytecode_module_from_sources_with_paths(
    sources: &[&str],
    paths: &[&str],
) -> Result<crate::bytecode::BytecodeModule, CompileError> {
    if sources.len() != paths.len() {
        return Err(CompileError::new("sources/paths length mismatch"));
    }
    let source_files = sources
        .iter()
        .zip(paths.iter())
        .map(|(source, path)| SourceFile::with_path(*path, *source))
        .collect::<Vec<_>>();
    CompileSession::from_sources(source_files).build_bytecode_module()
}

/// Build bytecode bytes from a single source file.
pub fn bytecode_bytes_from_source(source: &str) -> Result<Vec<u8>, CompileError> {
    CompileSession::from_source(source).build_bytecode_bytes()
}

/// Build bytecode bytes from a single source file with an explicit path.
pub fn bytecode_bytes_from_source_with_path(
    source: &str,
    path: &str,
) -> Result<Vec<u8>, CompileError> {
    CompileSession::from_sources(vec![SourceFile::with_path(path, source)]).build_bytecode_bytes()
}

/// Build bytecode bytes from multiple source files.
pub fn bytecode_bytes_from_sources(sources: &[&str]) -> Result<Vec<u8>, CompileError> {
    let source_files = sources
        .iter()
        .copied()
        .map(SourceFile::new)
        .collect::<Vec<_>>();
    CompileSession::from_sources(source_files).build_bytecode_bytes()
}

/// Build bytecode bytes from multiple source files with explicit paths.
pub fn bytecode_bytes_from_sources_with_paths(
    sources: &[&str],
    paths: &[&str],
) -> Result<Vec<u8>, CompileError> {
    if sources.len() != paths.len() {
        return Err(CompileError::new("sources/paths length mismatch"));
    }
    let source_files = sources
        .iter()
        .zip(paths.iter())
        .map(|(source, path)| SourceFile::with_path(*path, *source))
        .collect::<Vec<_>>();
    CompileSession::from_sources(source_files).build_bytecode_bytes()
}
