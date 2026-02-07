//! `trust-runtime` - IEC 61131-3 Structured Text runtime interpreter.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

/// Bundle discovery helpers.
pub mod bundle;
/// Bundle build helpers.
pub mod bundle_builder;
/// Bundle template rendering helpers.
pub mod bundle_template;
/// Bytecode metadata configuration helpers.
pub mod bytecode;
/// Runtime bundle configuration.
pub mod config;
/// Control server and protocol.
pub mod control;
mod datetime;
/// Debugging and tracing support.
pub mod debug;
/// Local discovery (mDNS) for runtimes.
pub mod discovery;
/// Runtime errors and configuration.
pub mod error;
/// Expression and statement evaluation.
pub mod eval;
/// Test harness for runtime execution.
pub mod harness;
/// HMI schema and value contract helpers.
pub mod hmi;
/// FB/Class instance management.
pub mod instance;
/// Direct I/O mapping.
pub mod io;
/// Variable storage and instances.
pub mod memory;
/// Runtime-to-runtime mesh data sharing.
pub mod mesh;
/// Runtime metrics collection.
pub mod metrics;
mod numeric;
/// PLCopen XML import/export helpers (strict subset profile).
pub mod plcopen;
/// Retain storage support.
pub mod retain;
/// Resource scheduling helpers and clocks.
pub mod scheduler;
/// Runtime settings snapshot.
pub mod settings;
/// System setup helpers (writes system IO config).
pub mod setup;
/// Simulation-first runtime mode, configuration, and coupling hooks.
pub mod simulation;
/// Standard library functions and FBs.
pub mod stdlib;
/// Task scheduling and cycle execution.
pub mod task;
/// Terminal UI for runtime monitoring.
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
