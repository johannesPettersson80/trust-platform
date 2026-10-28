//! Runtime launcher helpers.

use std::collections::VecDeque;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::bundle::{detect_bundle_path, is_bundle_not_found};
use trust_runtime::config::{RuntimeBundle, WebAuthMode, WebConfig};
use trust_runtime::control::endpoint::{
    control_binding, project_control_endpoint_text, ControlBinding, PublishedEndpoint,
};
use trust_runtime::control::{
    ControlEndpoint, ControlServer, ControlState, SourceFile, SourceRegistry,
};
use trust_runtime::discovery::{start_discovery_with_state, DiscoveryState};
use trust_runtime::historian::HistorianService;
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::linux_rt::{make_thread_init_hook, LinuxRtRuntimeStatus};
use trust_runtime::mesh::start_mesh_with_tls;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::opcua::{start_wire_server, OpcUaWireServer};
use trust_runtime::openot_persistence::OpenOtPersistenceService;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{
    ResourceCommand, ResourceRunner, SimulationSpeed, StartGate, StdClock,
};
use trust_runtime::security::{load_tls_materials, pairing::PairingStore};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, EndpointFileState, MeshSettings, OpcUaSettings,
    RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Duration;
use trust_runtime::web::start_web_server;
use trust_runtime::RestartMode;

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

#[cfg(test)]
#[path = "run/conversion_profile_tests.rs"]
mod conversion_profile_tests;

include!("run/logging.rs");
