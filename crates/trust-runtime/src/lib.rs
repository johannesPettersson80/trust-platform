//! `trust-runtime` - IEC 61131-3 Structured Text runtime.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![recursion_limit = "512"]

extern crate self as trust_runtime;

#[cfg(all(test, feature = "full"))]
include!("../../../tests/support/generated_output_oracle.rs");

/// Portable runtime-core compatibility surface.
pub use trust_runtime_core as runtime_core;

/// Beckhoff ADS client integration contracts.
#[cfg(feature = "full")]
#[path = "host/ads/mod.rs"]
pub mod ads;
/// Fail-closed external boundary value resolution.
#[cfg(feature = "full")]
#[path = "host/boundary/mod.rs"]
pub mod boundary;
/// Bundle discovery helpers.
#[cfg(feature = "full")]
#[path = "host/bundle.rs"]
pub mod bundle;
/// Bundle build helpers.
#[cfg(feature = "full")]
#[path = "host/bundle_builder/mod.rs"]
pub mod bundle_builder;
/// Bundle template rendering helpers.
#[cfg(feature = "full")]
#[path = "host/bundle_template.rs"]
pub mod bundle_template;
/// Bytecode metadata configuration helpers.
#[cfg(feature = "full")]
pub mod bytecode;
/// Runtime bundle configuration.
#[cfg(feature = "full")]
pub mod config;
/// Shared runtime connector status contracts.
#[cfg(feature = "full")]
pub mod connectors;
/// Control server and protocol.
#[cfg(feature = "full")]
pub mod control;
#[cfg(feature = "full")]
pub(crate) mod datetime {
    pub(crate) use trust_runtime_core::datetime::*;
}
/// Debugging and tracing support.
#[cfg(feature = "full")]
#[path = "host/debug/mod.rs"]
pub mod debug;
/// Local discovery (mDNS) for runtimes.
#[cfg(feature = "full")]
#[path = "host/discovery.rs"]
pub mod discovery;
/// Runtime errors and configuration.
pub mod error {
    pub use trust_runtime_core::error::*;
}
/// Expression and statement evaluation.
#[cfg(feature = "full")]
#[path = "host/eval/mod.rs"]
pub mod eval;
/// Runtime execution backend selection and provenance.
#[cfg(feature = "full")]
#[path = "host/execution_backend.rs"]
pub mod execution_backend;
/// Test harness for runtime execution.
#[cfg(feature = "full")]
#[path = "host/harness/mod.rs"]
pub mod harness;
#[cfg(feature = "full")]
#[path = "host/helper_eval/mod.rs"]
pub(crate) mod helper_eval;
/// Historian, alerts, and Prometheus observability helpers.
#[cfg(feature = "full")]
#[path = "host/historian/mod.rs"]
pub mod historian;
/// HMI schema and value contract helpers.
#[cfg(feature = "full")]
pub mod hmi;
/// FB/Class instance management.
#[cfg(feature = "full")]
#[path = "host/instance.rs"]
pub mod instance;
/// Direct I/O mapping.
#[cfg(feature = "full")]
pub mod io;
/// Linux `PREEMPT_RT` posture config and verification helpers.
#[cfg(feature = "full")]
#[path = "host/linux_rt.rs"]
pub mod linux_rt;
/// Variable storage and instances.
#[cfg(feature = "full")]
pub mod memory;
/// Runtime-to-runtime mesh data sharing.
#[cfg(feature = "full")]
#[path = "host/mesh/mod.rs"]
pub mod mesh;
/// Runtime metrics collection.
#[cfg(feature = "full")]
#[path = "host/metrics.rs"]
pub mod metrics;
/// OPC UA profile and IEC-to-OPC UA mapping helpers.
#[cfg(feature = "full")]
#[path = "host/opcua/mod.rs"]
pub mod opcua;
/// OpenOT declaration-attribute authoring support.
#[cfg(feature = "full")]
pub mod openot_authoring;
/// Host-side durable persistence contracts for resolved OpenOT documents.
#[cfg(feature = "full")]
#[path = "host/openot_persistence/mod.rs"]
pub mod openot_persistence;
/// PLCopen XML import/export helpers (strict subset profile).
#[cfg(feature = "full")]
#[path = "host/plcopen.rs"]
pub mod plcopen;
/// Backend-agnostic runtime/program model and shared semantics.
#[cfg(feature = "full")]
pub mod program_model;
/// Deterministic same-host realtime (T0/HardRT) communication contracts.
#[cfg(feature = "full")]
#[path = "host/realtime/mod.rs"]
pub mod realtime;
/// Local package registry contracts and workflows.
#[cfg(feature = "full")]
#[path = "host/registry/mod.rs"]
pub mod registry;
/// Retain storage support.
#[cfg(feature = "full")]
pub mod retain;
/// Runtime cloud contract and UI projection helpers.
#[cfg(feature = "full")]
pub mod runtime_cloud;
/// Resource scheduling helpers and clocks.
#[cfg(feature = "full")]
pub mod scheduler;
/// Security roles and authorization helpers.
#[cfg(feature = "full")]
#[path = "host/security/mod.rs"]
pub mod security;
/// Runtime settings snapshot.
#[cfg(feature = "full")]
#[path = "host/settings.rs"]
pub mod settings;
/// System setup helpers (writes system IO config).
#[cfg(feature = "full")]
#[path = "host/setup.rs"]
pub mod setup;
/// Simulation-first runtime mode, configuration, and coupling hooks.
#[cfg(feature = "full")]
#[path = "host/simulation.rs"]
pub mod simulation;
/// Standard library functions and FBs.
#[cfg(feature = "full")]
pub mod stdlib;
/// Task scheduling and cycle execution.
#[cfg(feature = "full")]
pub mod task;
/// Terminal UI for runtime monitoring.
#[cfg(feature = "full")]
#[path = "host/ui/mod.rs"]
pub mod ui;
/// Value types and date/time profile.
#[cfg(feature = "full")]
pub mod value;
/// Watchdog and fault policies.
#[cfg(feature = "full")]
pub mod watchdog;
/// Embedded browser UI server.
#[cfg(feature = "full")]
pub mod web;

#[cfg(feature = "full")]
mod runtime;

#[cfg(feature = "full")]
pub(crate) use runtime::types::GlobalInitValue;
#[cfg(feature = "full")]
pub use runtime::{
    RestartMode, RetainPolicy, RetainSnapshot, Runtime, RuntimeBuilder, RuntimeMetadata,
};

#[cfg(not(feature = "full"))]
#[path = "host/runtime_only.rs"]
mod runtime_only;
#[cfg(not(feature = "full"))]
pub use runtime_only::Runtime;
#[cfg(not(feature = "full"))]
pub use trust_runtime_core::retain::{RestartMode, RetainPolicy, RetainSnapshot};

// These are the canonical core records, not alternative hosted semantics.
#[cfg(not(feature = "full"))]
pub use trust_runtime_core::{
    bytecode, memory, program_model, retain, scheduler, stdlib, task, value, watchdog,
};
/// Direct I/O address records for the source-free hosted composition.
#[cfg(not(feature = "full"))]
pub mod io {
    pub use trust_runtime_core::io_address::{IoAddress, IoSize};
    pub use trust_runtime_core::memory::IoArea;
}
#[path = "host/host_clock.rs"]
mod host_clock;
#[cfg(feature = "register-ir")]
#[path = "runtime/register_environment.rs"]
mod register_environment;
