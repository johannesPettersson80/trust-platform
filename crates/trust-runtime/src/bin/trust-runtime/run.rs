//! Runtime launcher helpers.

use std::collections::VecDeque;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::resolve_sources_root;
use trust_runtime::bytecode::BytecodeModule;
use trust_runtime::config::{RuntimeBundle, WebAuthMode, WebConfig};
use trust_runtime::control::{
    spawn_hmi_descriptor_watcher, ControlEndpoint, ControlServer, ControlState,
    HmiRuntimeDescriptor, SourceFile, SourceRegistry,
};
use trust_runtime::discovery::{start_discovery, DiscoveryState};
use trust_runtime::harness::CompileSession;
use trust_runtime::historian::HistorianService;
use trust_runtime::hmi::{HmiScaffoldMode, HmiSourceRef};
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::linux_rt::{make_thread_init_hook, LinuxRtRuntimeStatus};
use trust_runtime::mesh::start_mesh;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::opcua::{start_wire_server, OpcUaWireServer};
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{ResourceCommand, ResourceRunner, StartGate, StdClock};
use trust_runtime::security::{load_tls_materials, pairing::PairingStore};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, OpcUaSettings, RuntimeSettings,
    SimulationSettings, WebSettings,
};
use trust_runtime::value::Duration;
use trust_runtime::web::start_web_server;
use trust_runtime::{RestartMode, Runtime};

use crate::setup;
use crate::style;
use crate::wizard;

include!("run/types.rs");
include!("run/commands.rs");
include!("run/runtime.rs");
include!("run/ui_helpers.rs");
include!("run/util.rs");

#[cfg(test)]
#[path = "run/tests.rs"]
mod tests;

include!("run/logging.rs");
