//! Hosted configuration and tests around the one portable register implementation.

#[cfg(test)]
use super::super::core::Runtime;
#[cfg(test)]
pub(super) use super::frames::FrameStack;
#[cfg(test)]
pub(super) use super::stack::OperandStack;
#[cfg(test)]
pub(super) use super::{invalid_bytecode, VmModule};
pub(crate) use trust_runtime_core::vm::register_test_support::*;

/// Hosted environment parsing stays outside portable execution.
pub(crate) trait HostedRegisterEnvironment {
    fn from_env() -> Self;
}
impl HostedRegisterEnvironment for RegisterLoweringCacheState {
    fn from_env() -> Self {
        Self::from_environment(|name| std::env::var(name).ok())
    }
}
impl HostedRegisterEnvironment for RegisterTier1SpecializedExecutorState {
    fn from_env() -> Self {
        Self::from_environment(|name| std::env::var(name).ok())
    }
}

#[cfg(test)]
fn deadline_exceeded(deadline: Option<std::time::Instant>) -> bool {
    deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline)
}
#[cfg(test)]
fn register_statement_location(
    runtime: &Runtime,
    module: &super::VmModule,
    pou_id: u32,
    pc: usize,
) -> Option<crate::debug::SourceLocation> {
    let source = module.debug_map().source_by_pc.get(&(pou_id, pc as u32))?;
    runtime.resolve_vm_debug_location(source.file.as_str(), source.line, source.column)
}

#[cfg(test)]
mod fixture;
#[cfg(test)]
pub(crate) use fixture::LegacyRegisterFixtureContext;
#[cfg(test)]
mod tests;
