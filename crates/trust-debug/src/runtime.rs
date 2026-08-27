//! Debug runtime facade for the adapter.

use std::sync::{Arc, Mutex};

use trust_runtime::control::SourceFile as ControlSourceFile;
use trust_runtime::debug::DebugControl;
use trust_runtime::harness::CompileError;
use trust_runtime::simulation::SimulationController;
use trust_runtime::{Runtime, RuntimeMetadata};

use crate::protocol::{Breakpoint, SetBreakpointsArguments, SetBreakpointsResponseBody, Source};
use crate::session::{SourceFile, SourceOptionsUpdate};

/// Narrow interface for debug adapters.
pub trait DebugRuntime: Send {
    fn update_source_options(&mut self, update: SourceOptionsUpdate);
    fn set_program_path(&mut self, path: String);
    fn reload_program(&mut self, path: Option<&str>) -> Result<Vec<Breakpoint>, CompileError>;
    fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> SetBreakpointsResponseBody;
    fn take_breakpoint_report(&mut self) -> Option<String>;
    fn debug_control(&self) -> DebugControl;
    fn runtime_handle(&self) -> Arc<Mutex<Runtime>>;
    fn simulation_handle(&self) -> Arc<Mutex<Option<SimulationController>>>;
    fn metadata(&self) -> &RuntimeMetadata;
    fn source_file_for_path(&self, path: &str) -> Option<&SourceFile>;
    fn source_for_file_id(&self, file_id: u32) -> Option<Source>;
    fn source_text_for_file_id(&self, file_id: u32) -> Option<&str>;
    fn control_sources(&self) -> Vec<ControlSourceFile>;
}
