//! Runtime core modules.

#![allow(missing_docs)]

mod ads_subsystem;
mod authoring;
mod builder;
mod bytecode;
#[cfg(feature = "full")]
mod checked_admission_compile_fail;
#[cfg(test)]
mod core;
#[cfg(test)]
mod cycle;
#[cfg(test)]
mod faults;
#[cfg(test)]
mod hosted_observation_cycle_tests;
/// Compiles the embedding migration guide's examples as doc tests.
#[cfg(feature = "full")]
mod hosted_platform_guide_examples;
mod hosted_services;
mod installed;
mod installed_bindings;
mod installed_cycle;
#[cfg(test)]
mod installed_cycle_contract_tests;
mod installed_debug;
mod installed_entry;
mod installed_lifecycle;
mod installed_register;
mod installed_services;
mod io_subsystem;
mod mesh;
mod metadata;
mod metrics_subsystem;
mod opcua_client_subsystem;
mod openot_telemetry;
mod restart;
mod retain_snapshot;
pub(crate) mod types;
#[cfg(test)]
mod vm;
mod watchdog_subsystem;

pub(crate) use authoring::BytecodeAuthoringView;
pub use builder::RuntimeBuilder;
#[cfg(test)]
pub(crate) use core::Runtime as LegacyRuntimeFixture;
pub use installed::Runtime;
pub use metadata::RuntimeMetadata;
pub use trust_runtime_core::retain::{RestartMode, RetainPolicy};
pub use types::RetainSnapshot;

#[cfg(test)]
mod legacy_restart;

#[cfg(test)]
mod legacy_retain_snapshot;

#[cfg(test)]
mod legacy_bytecode;

#[cfg(test)]
mod legacy_retain_store;

#[cfg(test)]
mod legacy_harness;

#[cfg(test)]
pub(crate) use legacy_harness::LegacyTestHarness;
