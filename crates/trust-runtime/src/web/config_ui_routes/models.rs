use super::*;

#[derive(Debug, Clone)]
pub(super) struct WorkspaceRuntime {
    pub(super) runtime_id: String,
    pub(super) root: PathBuf,
    pub(super) runtime: RuntimeConfig,
}

#[derive(Debug, Clone)]
pub(super) struct WorkspaceModel {
    pub(super) root: PathBuf,
    pub(super) runtimes: Vec<WorkspaceRuntime>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FieldErrorItem {
    pub(super) path: String,
    pub(super) hint: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigTextWriteRequest {
    pub(super) runtime_id: Option<String>,
    pub(super) text: String,
    pub(super) expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigStWriteRequest {
    pub(super) runtime_id: Option<String>,
    pub(super) path: String,
    pub(super) text: String,
    pub(super) expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigStValidateRequest {
    pub(super) runtime_id: Option<String>,
    pub(super) path: Option<String>,
    pub(super) text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigLiveConnectRequest {
    pub(super) target: Option<String>,
    pub(super) token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigLiveTargetUpsertRequest {
    pub(super) target: String,
    pub(super) label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigLiveTargetRemoveRequest {
    pub(super) target: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigRuntimeLifecycleRequest {
    pub(super) runtime_id: String,
    pub(super) action: String,
    pub(super) mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigRuntimeCreateRequest {
    pub(super) runtime_id: String,
    pub(super) host_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfigRuntimeDeleteRequest {
    pub(super) runtime_id: String,
}
