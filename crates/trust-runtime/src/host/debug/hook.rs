//! Debug hook trait.

#![allow(missing_docs)]

use crate::memory::{InstanceId, VariableStorage};
use crate::stdlib::StandardLibrary;
use crate::value::{DateTimeProfile, Duration};
use trust_hir::types::TypeRegistry;

use super::SourceLocation;

/// Storage-backed debugger evaluation context.
///
/// The PLC storage is private to this crate: debugger evaluation contexts hold it
/// privately and offer no path around checked mutation (spec 34 §4.3.1).
///
/// ```compile_fail,E0616
/// use trust_runtime::debug::DebugRuntimeContext;
/// fn read_storage(ctx: &DebugRuntimeContext<'_>) {
///     let _ = &ctx.storage;
/// }
/// ```
///
/// ```
/// use trust_runtime::debug::DebugRuntimeContext;
/// fn public_fields(ctx: &DebugRuntimeContext<'_>) -> bool {
///     ctx.current_instance.is_some()
/// }
/// ```
pub struct DebugRuntimeContext<'a> {
    pub storage_abi: crate::value::StorageAbi,
    pub(crate) storage: &'a mut VariableStorage,
    pub registry: &'a TypeRegistry,
    pub stdlib: Option<&'a StandardLibrary>,
    pub profile: DateTimeProfile,
    pub current_instance: Option<InstanceId>,
    pub now: Duration,
}

impl<'a> DebugRuntimeContext<'a> {
    /// A context over storage and types the caller owns, with the default storage ABI and
    /// date-time profile, no standard library, no current instance and time zero. The public
    /// fields can be set afterwards; the storage stays private to the context, so a hook that
    /// receives the context cannot write PLC storage around checked mutation (spec 34 §4.3.1).
    pub fn new(storage: &'a mut VariableStorage, registry: &'a TypeRegistry) -> Self {
        Self {
            storage_abi: Default::default(),
            storage,
            registry,
            stdlib: None,
            profile: DateTimeProfile::default(),
            current_instance: None,
            now: Duration::ZERO,
        }
    }
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

#[cfg(test)]
#[path = "hook/contract_tests.rs"]
mod contract_tests;
