//! Debug hook trait.

#![allow(missing_docs)]

use crate::memory::{InstanceId, VariableStorage};
use crate::stdlib::StandardLibrary;
use crate::value::{DateTimeProfile, Duration};
use trust_hir::types::TypeRegistry;

use super::SourceLocation;

/// Storage-backed debugger evaluation context.
pub struct DebugRuntimeContext<'a> {
    pub storage: &'a mut VariableStorage,
    pub registry: &'a TypeRegistry,
    pub stdlib: Option<&'a StandardLibrary>,
    pub profile: DateTimeProfile,
    pub current_instance: Option<InstanceId>,
    pub now: Duration,
}

/// Debug hooks for statement-level instrumentation.
pub trait DebugHook {
    /// Called before a statement executes.
    fn on_statement(&mut self, location: Option<&SourceLocation>, call_depth: u32);

    /// Called before a statement executes with access to runtime storage.
    fn on_statement_with_context(
        &mut self,
        _ctx: &mut DebugRuntimeContext<'_>,
        location: Option<&SourceLocation>,
        call_depth: u32,
    ) {
        self.on_statement(location, call_depth);
    }
}

/// No-op debug hook.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopDebugHook;

impl DebugHook for NoopDebugHook {
    fn on_statement(&mut self, _location: Option<&SourceLocation>, _call_depth: u32) {}
}
