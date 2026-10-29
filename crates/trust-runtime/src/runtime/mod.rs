//! Runtime core modules: the compiled program a computer runs (specification 44 §8.8) and the
//! subsystems around its scan.

#![allow(missing_docs)]

mod ads_subsystem;
mod bound_global;
mod builder;
mod compiled;
#[cfg(test)]
mod critical_section_tests;
mod hmi_methods;
pub(crate) mod hmi_stale;
mod hmi_subsystem;
#[cfg(test)]
mod identify_cycle_tests;
mod io_subsystem;
mod link_subsystem;
pub(crate) mod metadata;
mod metrics_subsystem;
mod opcua_client_subsystem;
#[cfg(unix)]
mod openot_alarm_records;
mod openot_telemetry;
pub mod program_access;
#[cfg(test)]
mod test_fixtures;
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
pub(crate) use hmi_methods::InvocationError;
pub use metadata::RuntimeMetadata;
pub use program_access::{Access, Located, PathError, ProgramRead, ProgramWrite, View, Walk};
#[cfg(test)]
pub(crate) use test_fixtures::{bound_runtime, empty_runtime};
pub use trust_runtime_core::retain::{RestartMode, RetainPolicy};
pub use types::RetainSnapshot;
