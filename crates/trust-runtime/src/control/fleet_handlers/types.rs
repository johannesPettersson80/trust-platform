use super::*;

#[derive(Debug, Serialize)]
pub(super) struct FleetTopologyResponse {
    pub(super) schema_version: u32,
    pub(super) hosts: Vec<FleetHost>,
    pub(super) links: Vec<FleetLink>,
    pub(super) shared: Vec<FleetShared>,
    pub(super) external: Vec<FleetExternal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) discovered: Vec<FleetDiscovered>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetHost {
    pub(super) host_id: String,
    pub(super) hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) board: Option<String>,
    pub(super) arch: String,
    pub(super) os: String,
    pub(super) ips: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temp_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) uptime_s: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) load: Option<f64>,
    pub(super) containers: Vec<FleetContainer>,
    pub(super) runtimes: Vec<FleetRuntime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_seen_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetContainer {
    pub(super) container_id: String,
    pub(super) name: String,
    pub(super) image: String,
    pub(super) status: String,
    pub(super) runtimes: Vec<FleetRuntime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) restart_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cpu_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mem_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetRuntime {
    pub(super) runtime_id: String,
    pub(super) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) control_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) web_listen: Option<String>,
    pub(super) mode: String,
    pub(super) cycle_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) load: Option<f64>,
    pub(super) health: String,
    pub(super) detail: String,
    pub(super) endpoints: Vec<FleetEndpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_seen_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetEndpoint {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) protocol: String,
    pub(super) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) role: Option<String>,
    pub(super) health: String,
    pub(super) detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) live: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) children: Vec<FleetEndpointChild>,
    pub(super) owned: bool,
    pub(super) supports_test: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetEndpointChild {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slot: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) channels: Option<u16>,
    pub(super) health: String,
    pub(super) detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetLink {
    pub(super) id: String,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) protocol: String,
    pub(super) role: String,
    pub(super) direction: String,
    pub(super) same_host: bool,
    pub(super) status: String,
    pub(super) secure: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetShared {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) address: String,
    pub(super) used_by: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetExternal {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) via_protocol: Vec<String>,
    pub(super) direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct FleetDiscovered {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) name: String,
    pub(super) addresses: Vec<String>,
    pub(super) via_protocol: Vec<String>,
    pub(super) direction: String,
    pub(super) adopted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) web_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) web_tls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) mesh_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) host_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_seen_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<String>,
}
