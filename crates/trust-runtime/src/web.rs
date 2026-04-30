//! Embedded browser UI server.

#![allow(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read};
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use qrcode::{render::svg, QrCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::bundle_template::{IoConfigTemplate, IoDriverTemplate};
use crate::config::{
    load_system_io_config, IoConfig, IoDriverConfig, RuntimeCloudProfile, RuntimeCloudWanAllowRule,
    RuntimeConfig, WebAuthMode, WebConfig,
};
use crate::control::ControlState;
use crate::debug::dap::format_value;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::io::{IoAddress, IoDriverRegistry, IoSize};
use crate::memory::IoArea;
use crate::runtime_cloud::config_policy::{
    RuntimeCloudConfigAgentState, RuntimeCloudConfigSnapshot, RuntimeCloudConfigWriteError,
};
use crate::runtime_cloud::contracts::{
    evaluate_compatibility, ContractCompatibility, ReasonCode, RUNTIME_CLOUD_API_VERSION,
};
use crate::runtime_cloud::ha::{
    parse_action_ha_request, RuntimeCloudHaCoordinator, RuntimeCloudHaDecision,
    RuntimeCloudHaDispatchGate, RuntimeCloudHaDispatchRecord, RuntimeCloudHaDispatchTicket,
    RuntimeCloudHaRequest,
};
use crate::runtime_cloud::link_policy::{
    RuntimeCloudLinkTransport, RuntimeCloudLinkTransportPreference, RuntimeCloudLinkTransportState,
};
use crate::runtime_cloud::profile_policy::{
    runtime_cloud_apply_profile_policy, runtime_cloud_profile_precondition,
};
use crate::runtime_cloud::projection::{
    presence_record_from_observation, project_runtime_cloud_state, ChannelState, ChannelType,
    FleetEdge, PresenceThresholds, RuntimeCloudUiState, RuntimePeerObservation,
    RuntimePresenceRecord, UiContext, UiMode,
};
use crate::runtime_cloud::rollout_policy::{
    RuntimeCloudRolloutActionResponse, RuntimeCloudRolloutConfigView,
    RuntimeCloudRolloutManagerState, RuntimeCloudRolloutRecord,
};
use crate::runtime_cloud::routing::{
    map_action_to_control_request, preflight_action, RuntimeCloudActionPreflight,
    RuntimeCloudActionRequest, RuntimeCloudPreflightContext, RuntimeCloudTargetStatus,
};
use crate::security::{constant_time_eq, pairing::PairingStore, AccessRole, TlsMaterials};
use crate::setup::SetupOptions;

mod auth_helpers;
mod aux_routes;
mod config_ui_routes;
mod deploy;
mod hmi_ws;
mod http_post_policy;
pub mod ide;
mod ide_routes;
mod ide_tasks;
mod models;
mod ops_routes;
pub mod pairing {
    pub use crate::security::pairing::*;
}
mod runtime_cloud_dispatch;
mod runtime_cloud_helpers;
mod runtime_cloud_routes;
mod runtime_cloud_state;
mod server;
mod setup_support;
mod ui_routes;
mod web_helpers;

use auth_helpers::*;
use aux_routes::{handle_aux_route, AuxRouteContext, AuxRouteOutcome};
use config_ui_routes::{handle_config_ui_route, ConfigUiRouteContext, ConfigUiRouteOutcome};
use deploy::{apply_deploy, apply_rollback, DeployRequest};
use hmi_ws::*;
use http_post_policy::{api_post_policy_check, json_body_error_response, read_json_body};
use ide::{IdeError, IdeRole, WebIdeFrontendTelemetry, WebIdeState};
use ide_routes::{handle_ide_route, IdeRouteContext, IdeRouteOutcome};
use ide_tasks::*;
use models::*;
use ops_routes::{handle_ops_route, OpsRouteContext, OpsRouteOutcome};
use runtime_cloud_dispatch::{
    runtime_cloud_denied_results, runtime_cloud_peer_appears_live,
    runtime_cloud_preflight_for_action, runtime_cloud_target_control_url,
    runtime_cloud_target_web_base_url, RuntimeCloudPreflightPolicy,
};
use runtime_cloud_helpers::*;
use runtime_cloud_routes::{
    handle_runtime_cloud_route, RuntimeCloudRouteContext, RuntimeCloudRouteOutcome,
};
use runtime_cloud_state::*;
pub use server::{start_web_server, start_web_server_with_mode, WebServer};
use setup_support::*;
use ui_routes::{handle_ui_route, UiRouteContext, UiRouteOutcome};
use web_helpers::*;

const CYTOSCAPE_JS: &str = include_str!("web/ui/modules/cytoscape.min.js");
const HMI_HTML: &str = include_str!("web/ui/hmi.html");
const HMI_JS: &str = concat!(
    include_str!("web/ui/chunks/hmi-js/hmi-01.js"),
    include_str!("web/ui/chunks/hmi-js/hmi-02.js"),
    include_str!("web/ui/chunks/hmi-js/hmi-03.js"),
);
const HMI_CSS: &str = concat!(
    include_str!("web/ui/chunks/hmi-css/hmi-01.css"),
    include_str!("web/ui/chunks/hmi-css/hmi-02.css"),
    include_str!("web/ui/chunks/hmi-css/hmi-03.css"),
    include_str!("web/ui/chunks/hmi-css/hmi-04.css"),
    include_str!("web/ui/chunks/hmi-css/hmi-05.css"),
);
const HMI_MODEL_JS: &str = include_str!("web/ui/modules/hmi-model.js");
const HMI_MODEL_DESCRIPTOR_JS: &str = include_str!("web/ui/modules/hmi-model-descriptor.js");
const HMI_MODEL_LAYOUT_JS: &str = include_str!("web/ui/modules/hmi-model-layout.js");
const HMI_MODEL_NAVIGATION_JS: &str = include_str!("web/ui/modules/hmi-model-navigation.js");
const HMI_RENDERERS_JS: &str = include_str!("web/ui/modules/hmi-renderers.js");
const HMI_WIDGETS_JS: &str = include_str!("web/ui/modules/hmi-widgets.js");
const HMI_TRENDS_ALARMS_JS: &str = include_str!("web/ui/modules/hmi-trends-alarms.js");
const HMI_PROCESS_VIEW_JS: &str = include_str!("web/ui/modules/hmi-process-view.js");
const HMI_TRANSPORT_JS: &str = include_str!("web/ui/modules/hmi-transport.js");
const HMI_PAGES_JS: &str = include_str!("web/ui/modules/hmi-pages.js");
const IDE_HTML: &str = include_str!("web/ui/ide.html");
const BASE_CSS: &str = include_str!("web/ui/chunks/base-css/base-01.css");
const IDE_TABS_JS: &str = include_str!("web/ui/modules/ide-tabs.js");
const IDE_CSS: &str = concat!(
    include_str!("web/ui/chunks/ide-css/ide-01.css"),
    include_str!("web/ui/chunks/ide-css/ide-02.css"),
    include_str!("web/ui/chunks/ide-css/ide-03.css"),
    include_str!("web/ui/chunks/ide-css/ide-04.css"),
    include_str!("web/ui/chunks/ide-css/ide-05.css"),
    include_str!("web/ui/chunks/ide-css/ide-06.css"),
    include_str!("web/ui/chunks/ide-css/ide-07.css"),
    include_str!("web/ui/chunks/ide-css/ide-08.css"),
    include_str!("web/ui/chunks/ide-css/ide-09.css"),
);
const IDE_JS: &str = concat!(
    include_str!("web/ui/chunks/ide-js/ide-01.js"),
    include_str!("web/ui/chunks/ide-js/ide-02.js"),
    include_str!("web/ui/chunks/ide-js/ide-03.js"),
);
const IDE_SHELL_JS: &str = include_str!("web/ui/modules/ide-shell.js");
const IDE_EDITOR_JS: &str = include_str!("web/ui/modules/ide-editor.js");
const IDE_EDITOR_LANGUAGE_JS: &str = concat!(
    include_str!("web/ui/modules/ide-editor-language.js"),
    include_str!("web/ui/modules/ide-editor-language-part-2.js"),
    include_str!("web/ui/modules/ide-editor-language-part-3.js"),
);
const IDE_EDITOR_RUNTIME_JS: &str = include_str!("web/ui/modules/ide-editor-runtime.js");
const IDE_EDITOR_PANE_JS: &str = concat!(
    include_str!("web/ui/modules/ide-editor-pane.js"),
    include_str!("web/ui/modules/ide-editor-pane-part-2.js"),
    include_str!("web/ui/modules/ide-editor-pane-part-3.js"),
);
const IDE_WORKSPACE_JS: &str = include_str!("web/ui/modules/ide-workspace.js");
const IDE_WORKSPACE_TREE_JS: &str = concat!(
    include_str!("web/ui/modules/ide-workspace-tree.js"),
    include_str!("web/ui/modules/ide-workspace-tree-part-2.js"),
    include_str!("web/ui/modules/ide-workspace-tree-part-3.js"),
);
const IDE_WORKSPACE_FILES_JS: &str = concat!(
    include_str!("web/ui/modules/ide-workspace-files.js"),
    include_str!("web/ui/modules/ide-workspace-files-part-2.js"),
    include_str!("web/ui/modules/ide-workspace-files-part-3.js"),
);
const IDE_WORKSPACE_BROWSE_JS: &str = include_str!("web/ui/modules/ide-workspace-browse.js");
const IDE_OBSERVABILITY_JS: &str = concat!(
    include_str!("web/ui/modules/ide-observability.js"),
    include_str!("web/ui/modules/ide-observability-part-2.js"),
    include_str!("web/ui/modules/ide-observability-part-3.js"),
);
const IDE_COMMANDS_JS: &str = concat!(
    include_str!("web/ui/modules/ide-commands.js"),
    include_str!("web/ui/modules/ide-commands-part-2.js"),
    include_str!("web/ui/modules/ide-commands-part-3.js"),
);
const IDE_HARDWARE_JS: &str = include_str!("web/ui/modules/ide-hardware.js");
const IDE_ONLINE_JS: &str = include_str!("web/ui/modules/ide-online.js");
const IDE_DEBUG_JS: &str = include_str!("web/ui/modules/ide-debug.js");
const IDE_SETTINGS_JS: &str = include_str!("web/ui/modules/ide-settings.js");
const IDE_LOGS_JS: &str = include_str!("web/ui/modules/ide-logs.js");
const IDE_MONACO_BUNDLE_JS: &str = include_str!("web/ui/assets/ide-monaco.20260215.js");
const IDE_MONACO_BUNDLE_CSS: &str = include_str!("web/ui/assets/ide-monaco.20260215.css");
const IDE_LOGO_SVG: &str = include_str!("web/ui/assets/logo.svg");
const IDE_WASM_WORKER_JS: &str = include_str!("web/ui/wasm/worker.js");
const IDE_WASM_CLIENT_JS: &str = include_str!("web/ui/wasm/analysis-client.js");
const HMI_WS_ROUTE: &str = "/ws/hmi";
const HMI_WS_VALUES_POLL_INTERVAL: Duration = Duration::from_millis(100);
const HMI_WS_SCHEMA_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HMI_WS_ALARMS_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_JSON_REQUEST_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebServerMode {
    Runtime,
    StandaloneIde,
}

impl WebServerMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::StandaloneIde => "standalone-ide",
        }
    }
}

// Runtime-cloud peer liveliness should not flap under normal mDNS refresh cadence.
const RUNTIME_CLOUD_STALE_TIMEOUT_NS: u64 = 30_000_000_000;
const RUNTIME_CLOUD_PARTITION_TIMEOUT_NS: u64 = 120_000_000_000;
const RUNTIME_CLOUD_DEFAULT_SITE: &str = "default-site";
