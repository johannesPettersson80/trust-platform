//! Public harness build helpers.

#![allow(missing_docs)]

use std::sync::Arc;

use super::build;
use super::types::{CompileError, SourceFile};
use crate::io::IoInterface;
use crate::memory::VariableStorage;
use crate::RuntimeMetadata;
use indexmap::IndexMap;
use smol_str::SmolStr;

/// A project's declarations in their starting state (specification 44 §6.7): the metadata of its
/// programs, functions, types and tasks, the storage that holds the starting values of its
/// globals and program instances, and the process image its located variables bind.
#[derive(Debug, Clone)]
pub struct Declarations {
    metadata: RuntimeMetadata,
    storage: VariableStorage,
    globals: IndexMap<SmolStr, trust_hir::TypeId>,
    access: Vec<super::NamedAccess>,
    io: Arc<IoInterface>,
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

    /// The process image of the declarations before any scan: the addresses the located
    /// variables bind, with their names and types.
    #[must_use]
    pub fn io(&self) -> &IoInterface {
        &self.io
    }
}

/// The sources of a project, how their errors are labelled, and the conversion rules its program
/// is built with.
#[derive(Debug, Clone)]
pub struct CompileSession {
    /// The sources with their OpenOT authoring instrumented (specification 27 §10.3).
    sources: Vec<SourceFile>,
    /// The sources as written, before the OpenOT authoring is compiled into them, which a build of
    /// the program image instruments itself.
    written: Vec<SourceFile>,
    label_errors: bool,
    extra_program_instances: Vec<SmolStr>,
    instrumentation_errors: Vec<String>,
    conversion_profile: crate::stdlib::conversions::ConversionProfile,
}

impl CompileSession {
    /// Build a compile session from a single source.
    pub fn from_source(source: impl Into<String>) -> Self {
        let written = vec![SourceFile::new(source)];
        let (sources, instrumentation_errors) = instrument_sources_for_compile(written.clone());
        Self {
            sources,
            written,
            label_errors: false,
            extra_program_instances: Vec::new(),
            instrumentation_errors,
            conversion_profile: Default::default(),
        }
    }

    /// Build a compile session from multiple sources.
    pub fn from_sources(sources: Vec<SourceFile>) -> Self {
        let _trace = trust_hir::timing::CompilerStage::new("compiler.preprocess", sources.len());
        let label_errors = sources.len() > 1;
        let written = sources.clone();
        let (sources, instrumentation_errors) = instrument_sources_for_compile(sources);
        Self {
            sources,
            written,
            label_errors,
            extra_program_instances: Vec::new(),
            instrumentation_errors,
            conversion_profile: Default::default(),
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

    /// Use the given conversion rules (docs/specs/07-standard-functions.md 2.10).
    #[must_use]
    pub fn with_conversion_profile(
        mut self,
        profile: crate::stdlib::conversions::ConversionProfile,
    ) -> Self {
        self.conversion_profile = profile;
        self
    }

    /// The conversion rules of the session.
    #[must_use]
    pub fn conversion_profile(&self) -> crate::stdlib::conversions::ConversionProfile {
        self.conversion_profile
    }

    /// Access the registered sources.
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    /// The sources as written, which a build of the program image compiles (specification 44
    /// §3), the program instances registered at build time and the conversion rules: what
    /// [`TestHarness::from_session`](super::TestHarness::from_session) builds.
    ///
    /// # Errors
    ///
    /// The [`CompileError`] of sources whose OpenOT authoring does not validate.
    pub(super) fn build_inputs(
        &self,
    ) -> Result<
        (
            &[SourceFile],
            &[SmolStr],
            crate::stdlib::conversions::ConversionProfile,
        ),
        CompileError,
    > {
        self.ensure_instrumented()?;
        Ok((
            &self.written,
            &self.extra_program_instances,
            self.conversion_profile,
        ))
    }

    /// The declared program in its starting state (specification 44 §6.7), which the tools read
    /// without running it: its programs, their variables and types, and the starting values the build
    /// computes.
    ///
    /// # Errors
    ///
    /// The [`CompileError`] of sources with errors.
    pub fn declarations(&self) -> Result<Declarations, CompileError> {
        let _trace =
            trust_hir::timing::CompilerStage::new("compiler.declarations", self.sources.len());
        self.ensure_instrumented()?;
        let mut lowered = build::lower_application(&self.sources, self.label_errors)?;
        // The starting values follow the project's conversion rules
        // (docs/specs/07-standard-functions.md 2.10).
        lowered
            .builder
            .set_conversion_profile(self.conversion_profile);
        let (mut builder, templates) = lowered.construct(&self.extra_program_instances)?;
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
            io: Arc::new(std::mem::take(builder.io_mut())),
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
