use serde::Serialize;

pub(super) const COMM_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CommId {
    Ads,
    AdsServer,
    Opcua,
    OpcuaClient,
    ModbusTcp,
    Mqtt,
    Openot,
    Discovery,
    Mesh,
    RealtimeT0,
    RuntimeCloud,
    Ethercat,
    Gpio,
    Simulated,
    Loopback,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum CommHealth {
    NotInBuild,
    NotConfigured,
    Simulate,
    RuntimeUnreachable,
    Connected,
    Degraded,
    Error,
    ConfiguredPolicy,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CommPlatform {
    Unix,
    Linux,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum CommNextActionKind {
    None,
    Setup,
    OpenRuntimePane,
    TestConnection,
    DiagnoseAds,
    OpenDocs,
    ApplyConfig,
    SwitchToOnline,
    GetBuildWithFeature,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CommNextAction {
    pub(super) kind: CommNextActionKind,
    pub(super) label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RuntimeCapabilityStatus {
    pub(super) id: CommId,
    pub(super) built: bool,
    pub(super) configured: bool,
    pub(super) operational: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) platform: Option<CommPlatform>,
    pub(super) health: CommHealth,
    pub(super) detail: String,
    pub(super) next_action: CommNextAction,
    /// Live discovery health, carried only by the `discovery` entry and flattened into it, so the
    /// other entries keep their version-4 shape (specification 11, "Communication schema,
    /// capabilities, and Test").
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(super) discovery: Option<DiscoveryHealth>,
    /// Live mesh health, carried only by the `mesh` entry and flattened into it, so the other
    /// entries keep their version-4 shape (specification 11, "Communication schema, capabilities,
    /// and Test").
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(super) mesh: Option<MeshHealth>,
}

/// Live discovery health of the `discovery` capability entry (specification 11, "Communication
/// schema, capabilities, and Test"): whether runtime discovery is enabled, whether it announces
/// itself, and how many other runtimes it currently sees.
#[derive(Debug, Clone, Serialize)]
pub(super) struct DiscoveryHealth {
    pub(super) enabled: bool,
    pub(super) announcing: bool,
    pub(super) visible_runtimes: usize,
}

/// Live mesh health of the `mesh` capability entry (specification 11, "Communication schema,
/// capabilities, and Test"): one object for each configured peer, in configuration order.
#[derive(Debug, Clone, Serialize)]
pub(super) struct MeshHealth {
    pub(super) peers: Vec<MeshPeerHealth>,
}

/// One configured mesh peer and what the mesh's liveliness reports about it (specification 11,
/// "Communication schema, capabilities, and Test").
#[derive(Debug, Clone, Serialize)]
pub(super) struct MeshPeerHealth {
    pub(super) peer: String,
    pub(super) connected: bool,
    pub(super) last_seen_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct CommCapabilitiesResponse {
    pub(super) schema_version: u32,
    pub(super) capabilities: Vec<RuntimeCapabilityStatus>,
}

pub(super) fn action(kind: CommNextActionKind, label: &'static str) -> CommNextAction {
    CommNextAction { kind, label }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn communication_contract_serializes_stable_status_and_action_ids() {
        let statuses = [
            CommHealth::NotInBuild,
            CommHealth::NotConfigured,
            CommHealth::Simulate,
            CommHealth::RuntimeUnreachable,
            CommHealth::Connected,
            CommHealth::Degraded,
            CommHealth::Error,
            CommHealth::ConfiguredPolicy,
        ];
        let status_json: Vec<String> = statuses
            .into_iter()
            .map(|status| {
                serde_json::to_value(status)
                    .expect("status json")
                    .as_str()
                    .expect("status string")
                    .to_string()
            })
            .collect();
        assert_eq!(
            status_json,
            [
                "not_in_build",
                "not_configured",
                "simulate",
                "runtime_unreachable",
                "connected",
                "degraded",
                "error",
                "configured_policy",
            ]
        );

        let actions = [
            CommNextActionKind::Setup,
            CommNextActionKind::OpenRuntimePane,
            CommNextActionKind::TestConnection,
            CommNextActionKind::DiagnoseAds,
            CommNextActionKind::OpenDocs,
            CommNextActionKind::ApplyConfig,
            CommNextActionKind::SwitchToOnline,
            CommNextActionKind::GetBuildWithFeature,
        ];
        let action_json: Vec<String> = actions
            .into_iter()
            .map(|kind| {
                serde_json::to_value(kind)
                    .expect("action json")
                    .as_str()
                    .expect("action string")
                    .to_string()
            })
            .collect();
        assert_eq!(
            action_json,
            [
                "setup",
                "open_runtime_pane",
                "test_connection",
                "diagnose_ads",
                "open_docs",
                "apply_config",
                "switch_to_online",
                "get_build_with_feature",
            ]
        );
    }
}
