//! Debug runtime facade for the adapter: the session's sources and the project's declarations.

use trust_hir::types::TypeRegistry;
use trust_runtime::control::SourceFile as ControlSourceFile;
use trust_runtime::harness::CompileError;
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::DateTimeProfile;
use trust_runtime::RuntimeMetadata;

use crate::protocol::Source;
use crate::session::{SourceFile, SourceOptionsUpdate};

/// Narrow interface for debug adapters.
pub trait DebugRuntime: Send {
    fn update_source_options(&mut self, update: SourceOptionsUpdate);
    /// The project folder the source options name, against which the file labels of a compiled
    /// program resolve (specification 13, "Stack Trace and Navigation").
    fn source_root(&self) -> Option<&str>;
    fn set_program_path(&mut self, path: String);
    /// The program file of the last launch.
    fn program_path(&self) -> Option<&str>;
    /// The sources of the program at `path` by the source options, registered as the session's
    /// sources, whose declarations the session keeps: the sources a launch builds (specification 44
    /// NX-16).
    fn launch_sources(&mut self, path: &str) -> Result<Vec<(String, String)>, CompileError>;
    /// Reads the declarations of the project the source options name, as an attach knows its
    /// program.
    fn load_project(&mut self) -> Result<(), CompileError>;
    /// The declarations of the session's project: its tasks, whose debugger threads the client sees
    /// (specification 13, "Multi-Task Thread Model").
    fn metadata(&self) -> &RuntimeMetadata;
    fn source_file_for_path(&self, path: &str) -> Option<&SourceFile>;
    fn source_for_file_id(&self, file_id: u32) -> Option<Source>;
    fn source_text_for_file_id(&self, file_id: u32) -> Option<&str>;
    fn control_sources(&self) -> Vec<ControlSourceFile>;
    /// The types and the date and time profile of the project the source options name, from its
    /// declarations, with which the debug expressions of a runtime that runs a compiled program
    /// are compiled (specification 11 §6.13.2).
    fn project_expression_types(&self) -> Result<(TypeRegistry, DateTimeProfile), CompileError>;
    /// The conversion rules of the launched project's `vendor_profile`, so that a debug session
    /// computes what `trust-runtime run` computes (docs/specs/07-standard-functions.md 2.10).
    fn conversion_profile(&self) -> ConversionProfile;
}
