//! `trust-runtime` - IEC 61131-3 Structured Text runtime.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![recursion_limit = "512"]

extern crate self as trust_runtime;

/// Portable runtime-core compatibility surface.
pub use trust_runtime_core as runtime_core;

/// Fail-closed external boundary value resolution.
#[path = "host/boundary/mod.rs"]
pub mod boundary;
/// Bundle discovery helpers.
#[path = "host/bundle.rs"]
pub mod bundle;
/// Bundle build helpers.
#[path = "host/bundle_builder/mod.rs"]
pub mod bundle_builder;
/// Bundle template rendering helpers.
#[path = "host/bundle_template.rs"]
pub mod bundle_template;
/// Bytecode metadata configuration helpers.
pub mod bytecode;
/// Runtime bundle configuration.
pub mod config;
/// Control server and protocol.
pub mod control;
pub(crate) mod datetime {
    pub(crate) use trust_runtime_core::datetime::*;
}
/// Debugging and tracing support.
#[path = "host/debug/mod.rs"]
pub mod debug;
/// Local discovery (mDNS) for runtimes.
#[path = "host/discovery.rs"]
pub mod discovery;
/// Runtime errors and configuration.
pub mod error {
    pub use trust_runtime_core::error::*;
}
/// Expression and statement evaluation.
#[path = "host/eval/mod.rs"]
pub mod eval;
/// Runtime execution backend selection and provenance.
#[path = "host/execution_backend.rs"]
pub mod execution_backend;
/// Test harness for runtime execution.
#[path = "host/harness/mod.rs"]
pub mod harness;
#[path = "host/helper_eval/mod.rs"]
pub(crate) mod helper_eval;
/// Historian, alerts, and Prometheus observability helpers.
#[path = "host/historian/mod.rs"]
pub mod historian;
/// HMI schema and value contract helpers.
pub mod hmi;
/// FB/Class instance management.
#[path = "host/instance.rs"]
pub mod instance;
/// Direct I/O mapping.
pub mod io;
/// Linux `PREEMPT_RT` posture config and verification helpers.
#[path = "host/linux_rt.rs"]
pub mod linux_rt;
/// Variable storage and instances.
pub mod memory;
/// Runtime-to-runtime mesh data sharing.
#[path = "host/mesh/mod.rs"]
pub mod mesh;
/// Runtime metrics collection.
#[path = "host/metrics.rs"]
pub mod metrics;
mod numeric {
    pub use trust_runtime_core::numeric::*;
}
/// OPC UA profile and IEC-to-OPC UA mapping helpers.
#[path = "host/opcua/mod.rs"]
pub mod opcua;
/// PLCopen XML import/export helpers (strict subset profile).
#[path = "host/plcopen.rs"]
pub mod plcopen;
/// Backend-agnostic runtime/program model and shared semantics.
pub mod program_model;
/// Deterministic same-host realtime (T0/HardRT) communication contracts.
#[path = "host/realtime/mod.rs"]
pub mod realtime;
/// Local package registry contracts and workflows.
#[path = "host/registry/mod.rs"]
pub mod registry;
/// Retain storage support.
pub mod retain;
/// Runtime cloud contract and UI projection helpers.
pub mod runtime_cloud;
/// Resource scheduling helpers and clocks.
pub mod scheduler;
/// Security roles and authorization helpers.
#[path = "host/security/mod.rs"]
pub mod security;
/// Runtime settings snapshot.
#[path = "host/settings.rs"]
pub mod settings;
/// System setup helpers (writes system IO config).
#[path = "host/setup.rs"]
pub mod setup;
/// Simulation-first runtime mode, configuration, and coupling hooks.
#[path = "host/simulation.rs"]
pub mod simulation;
/// Standard library functions and FBs.
pub mod stdlib;
/// Task scheduling and cycle execution.
pub mod task;
/// Terminal UI for runtime monitoring.
#[path = "host/ui/mod.rs"]
pub mod ui;
/// Value types and date/time profile.
pub mod value;
/// Watchdog and fault policies.
pub mod watchdog;
/// Embedded browser UI server.
pub mod web;

mod runtime;

pub(crate) use runtime::types::GlobalInitValue;
pub use runtime::{RestartMode, RetainPolicy, RetainSnapshot, Runtime, RuntimeMetadata};
