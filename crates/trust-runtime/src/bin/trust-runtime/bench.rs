//! Runtime benchmark command.

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write;
use std::time::Instant;

use anyhow::Context;
use serde::Serialize;
use serde_json::json;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::config::RuntimeConfig;
use trust_runtime::harness::CompileSession;
use trust_runtime::value::Value;

use trust_runtime::realtime::{
    RealtimeRoute, T0ChannelCounters, T0ChannelPolicy, T0ReadOutcome, T0Transport,
};
use trust_runtime::runtime_cloud::contracts::RUNTIME_CLOUD_API_VERSION;
use trust_runtime::runtime_cloud::routing::{
    map_action_to_control_request, preflight_action, RuntimeCloudActionRequest,
    RuntimeCloudPreflightContext, RuntimeCloudTargetStatus,
};
use trust_runtime::security::AccessRole;

use crate::cli::{BenchAction, BenchOutputFormat};

include!("bench/models.rs");
include!("bench/stats.rs");
include!("bench/t0_shm.rs");
include!("bench/mesh_zenoh.rs");
include!("bench/dispatch.rs");
include!("bench/init.rs");
include!("bench/project.rs");
include!("bench/output.rs");
include!("bench/command.rs");

#[cfg(test)]
#[path = "bench/tests.rs"]
mod tests;
