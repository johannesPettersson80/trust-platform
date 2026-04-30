//! Shared data models for web route handlers.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::runtime_cloud::contracts::ReasonCode;
use crate::runtime_cloud::link_policy::{
    RuntimeCloudLinkTransport, RuntimeCloudLinkTransportPreference,
};
use crate::runtime_cloud::routing::RuntimeCloudActionPreflight;

#[derive(Debug, Deserialize)]
pub(super) struct SetupApplyRequest {
    #[serde(alias = "bundle_path")]
    pub(super) project_path: Option<String>,
    pub(super) resource_name: Option<String>,
    pub(super) cycle_ms: Option<u64>,
    pub(super) driver: Option<String>,
    pub(super) write_system_io: Option<bool>,
    pub(super) overwrite_system_io: Option<bool>,
    pub(super) use_system_io: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RollbackRequest {
    pub(super) restart: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IoConfigRequest {
    pub(super) driver: Option<String>,
    pub(super) params: Option<serde_json::Value>,
    pub(super) drivers: Option<Vec<IoDriverConfigRequest>>,
    pub(super) safe_state: Option<Vec<IoSafeStateEntry>>,
    pub(super) use_system_io: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IoDriverConfigRequest {
    pub(super) name: String,
    pub(super) params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct IoConfigResponse {
    pub(super) driver: String,
    pub(super) params: serde_json::Value,
    pub(super) drivers: Vec<IoDriverConfigResponse>,
    pub(super) safe_state: Vec<IoSafeStateEntry>,
    pub(super) supported_drivers: Vec<String>,
    pub(super) source: String,
    pub(super) use_system_io: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct IoDriverConfigResponse {
    pub(super) name: String,
    pub(super) params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeSessionRequest {
    pub(super) role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeProjectOpenRequest {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeProjectCreateRequest {
    pub(super) name: String,
    pub(super) location: String,
    pub(super) template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeWriteRequest {
    pub(super) path: String,
    pub(super) expected_version: u64,
    pub(super) content: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeFsCreateRequest {
    pub(super) path: String,
    pub(super) kind: Option<String>,
    pub(super) content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeFsRenameRequest {
    pub(super) path: String,
    pub(super) new_path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeFsDeleteRequest {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeDiagnosticsRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudDesiredWriteRequest {
    pub(super) api_version: String,
    pub(super) actor: String,
    pub(super) desired: serde_json::Value,
    pub(super) expected_revision: Option<u64>,
    pub(super) expected_etag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudLinkTransportSetRequest {
    pub(super) api_version: String,
    pub(super) actor: String,
    pub(super) source: String,
    pub(super) target: String,
    pub(super) transport: RuntimeCloudLinkTransport,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeCloudLinkTransportSetResponse {
    pub(super) ok: bool,
    pub(super) preference: Option<RuntimeCloudLinkTransportPreference>,
    pub(super) denial_code: Option<ReasonCode>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudRolloutCreateRequest {
    pub(super) api_version: String,
    pub(super) actor: String,
    pub(super) target_runtimes: Vec<String>,
    pub(super) desired_revision: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudControlProxyRequest {
    pub(super) api_version: String,
    pub(super) actor: String,
    pub(super) target_runtime: String,
    pub(super) control_request: RuntimeCloudControlProxyControlRequest,
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudIoConfigProxyRequest {
    pub(super) api_version: String,
    pub(super) actor: String,
    pub(super) target_runtime: String,
    pub(super) driver: Option<String>,
    pub(super) params: Option<serde_json::Value>,
    pub(super) drivers: Option<Vec<IoDriverConfigRequest>>,
    pub(super) safe_state: Option<Vec<IoSafeStateEntry>>,
    pub(super) use_system_io: Option<bool>,
}

impl RuntimeCloudIoConfigProxyRequest {
    pub(super) fn to_io_config_request(&self) -> IoConfigRequest {
        IoConfigRequest {
            driver: self.driver.clone(),
            params: self.params.clone(),
            drivers: self.drivers.clone(),
            safe_state: self.safe_state.clone(),
            use_system_io: self.use_system_io,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RuntimeCloudControlProxyControlRequest {
    #[serde(rename = "type")]
    pub(super) r#type: String,
    pub(super) params: Option<serde_json::Value>,
    pub(super) request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeFormatRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdePositionRequest {
    pub(super) line: u32,
    pub(super) character: u32,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeHoverRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
    pub(super) position: IdePositionRequest,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeCompletionRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
    pub(super) position: IdePositionRequest,
    pub(super) limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeReferencesRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
    pub(super) position: IdePositionRequest,
    pub(super) include_declaration: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeRenameRequest {
    pub(super) path: String,
    pub(super) content: Option<String>,
    pub(super) position: IdePositionRequest,
    pub(super) new_name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct IdeFrontendTelemetryRequest {
    pub(super) bootstrap_failures: Option<u64>,
    pub(super) analysis_timeouts: Option<u64>,
    pub(super) worker_restarts: Option<u64>,
    pub(super) autosave_failures: Option<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct IdeTaskJob {
    pub(super) job_id: u64,
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) success: Option<bool>,
    pub(super) output: String,
    pub(super) started_ms: u64,
    pub(super) finished_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct IdeTaskLocation {
    pub(super) path: String,
    pub(super) line: u32,
    pub(super) column: u32,
    pub(super) message: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct IdeTaskSnapshot {
    pub(super) job_id: u64,
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) success: Option<bool>,
    pub(super) output: String,
    pub(super) locations: Vec<IdeTaskLocation>,
    pub(super) started_ms: u64,
    pub(super) finished_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IoSafeStateEntry {
    pub(super) address: String,
    pub(super) value: String,
}

#[derive(Debug, Serialize)]
pub(super) struct SetupDefaultsResponse {
    pub(super) project_path: String,
    pub(super) resource_name: String,
    pub(super) cycle_ms: u64,
    pub(super) driver: String,
    pub(super) supported_drivers: Vec<String>,
    pub(super) use_system_io: bool,
    pub(super) system_io_exists: bool,
    pub(super) write_system_io: bool,
    pub(super) needs_setup: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeCloudDispatchTargetResult {
    pub(super) runtime_id: String,
    pub(super) ok: bool,
    pub(super) denial_code: Option<ReasonCode>,
    pub(super) denial_reason: Option<String>,
    pub(super) audit_id: Option<String>,
    pub(super) response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeCloudDispatchResponse {
    pub(super) api_version: String,
    pub(super) request_id: String,
    pub(super) connected_via: String,
    pub(super) acting_on: Vec<String>,
    pub(super) dry_run: bool,
    pub(super) ok: bool,
    pub(super) preflight: RuntimeCloudActionPreflight,
    pub(super) results: Vec<RuntimeCloudDispatchTargetResult>,
}
