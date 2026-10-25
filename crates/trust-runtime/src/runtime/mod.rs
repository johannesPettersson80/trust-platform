//! Runtime core modules: the compiled program a computer runs (specification 44 §8.8) and the
//! subsystems around its scan.

#![allow(missing_docs)]

mod ads_subsystem;
mod bound_global;
mod builder;
mod compiled;
mod io_subsystem;
mod metadata;
mod metrics_subsystem;
mod opcua_client_subsystem;
mod openot_telemetry;
pub(crate) mod types;
mod watchdog_subsystem;

#[cfg(test)]
pub(crate) use bound_global::test_support::NamedGlobals;
pub use bound_global::BoundGlobal;
pub(crate) use bound_global::GlobalPort;
pub use builder::RuntimeBuilder;
pub use compiled::{
    CompiledDebugger, CompiledRuntime, CompiledState, CompiledVariable, NamedVariable,
};
pub use metadata::RuntimeMetadata;
pub use trust_runtime_core::retain::{RestartMode, RetainPolicy};
pub use types::RetainSnapshot;
