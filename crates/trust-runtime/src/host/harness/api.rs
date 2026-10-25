//! Public harness build helpers.

#![allow(missing_docs)]

use super::build;
use super::types::{CompileError, SourceFile};
use crate::memory::VariableStorage;
use crate::RuntimeMetadata;
use indexmap::IndexMap;
use smol_str::SmolStr;

/// A project's declarations in their starting state (specification 44 §6.7): the metadata of its
/// programs, functions, types and tasks, and the storage that holds the starting values of its
/// globals and program instances.
#[derive(Debug, Clone)]
pub struct Declarations {
    metadata: RuntimeMetadata,
    storage: VariableStorage,
    globals: IndexMap<SmolStr, trust_hir::TypeId>,
    access: Vec<super::NamedAccess>,
}

impl Declarations {
    /// The `VAR_ACCESS` declarations, by the names of the program's symbols.
    pub(crate) fn access_paths(&self) -> &[super::NamedAccess] {
        &self.access
    }

    /// The project's globals, each with its declared type, in declaration order.
    pub fn globals(&self) -> impl Iterator<Item = (&SmolStr, trust_hir::TypeId)> {
        self.globals.iter().map(|(name, type_id)| (name, *type_id))
    }

    /// The metadata of the project's declarations.
    #[must_use]
    pub fn metadata(&self) -> &RuntimeMetadata {
        &self.metadata
    }

    /// The starting values of the project's globals and program instances.
    #[must_use]
    pub fn storage(&self) -> &VariableStorage {
        &self.storage
    }
}

/// The sources of a project and how their errors are labelled.
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

    /// The declared program in its starting state (specification 44 §6.7), which the tools read
    /// without running it: its programs, their variables and types, and the starting values the build
    /// computes.
    ///
    /// # Errors
    ///
    /// The [`CompileError`] of sources with errors.
    pub fn declarations(&self) -> Result<Declarations, CompileError> {
        self.ensure_instrumented()?;
        let (builder, templates) = build::lower_application(&self.sources, self.label_errors)?
            .construct(&self.extra_program_instances)?;
        let metadata = builder
            .metadata_for_configuration(
                templates,
                builder.programs().clone(),
                builder.tasks().to_vec(),
            )
            .map_err(|error| CompileError::new(error.to_string()))?;
        Ok(Declarations {
            metadata,
            storage: builder.storage().clone(),
            globals: builder.global_types(),
            access: builder.access_paths().to_vec(),
        })
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

#[cfg(test)]
#[path = "api_contract_tests.rs"]
mod contract_tests;
