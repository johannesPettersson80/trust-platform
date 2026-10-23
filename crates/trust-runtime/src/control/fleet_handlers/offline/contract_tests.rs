use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::*;

static PROJECT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let sequence = PROJECT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-fleet-offline-{label}-{}-{sequence}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create test project");
        Self { root }
    }

    fn path(&self) -> &Path {
        self.root.as_path()
    }

    fn write(&self, relative: &str, content: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, content).expect("write fixture");
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn runtime_toml(extra: &str) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "offline-runtime"
cycle_interval_ms = 100

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "tcp://127.0.0.1:9900"
mode = "production"
debug_enabled = false
auth_token = "offline-contract-test-token"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"

[runtime.web]
enabled = true
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.discovery]
enabled = true
service_name = "truST"
advertise = true
interfaces = ["eth0"]

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
connect = []
tls = false
publish = []
subscribe = {{}}

[runtime.cloud]
profile = "dev"

[runtime.cloud.wan]
allow_write = []

{extra}
"#
    )
}

fn ads_toml() -> &'static str {
    r#"
[[connections]]
name = "line"
target_net_id = "5.23.91.12.1.1"
host = "192.0.2.10"
ams_port = 851
local_net_id = "192.0.2.100.1.1"
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Ready"
var = "line_ready"
type = "BOOL"
"#
}

fn opcua_client_toml(password: &str) -> String {
    format!(
        r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://192.0.2.10:4840/line"
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
auth = "username"
username = "operator"
password = "{password}"

[[connections.points]]
var = "line_ready"
node_id = "ns=2;s=Ready"
type = "bool"
access = "read"
"#
    )
}

fn build(project: &TestProject) -> FleetTopologyResponse {
    build_project_fleet_topology(project.path()).expect("offline topology")
}

fn serialized(project: &TestProject) -> Value {
    serde_json::to_value(build(project)).expect("serialize offline topology")
}

#[test]
fn fleet_offline_contract_missing_runtime_file_is_an_error() {
    let project = TestProject::new("missing-runtime");
    let error =
        build_project_fleet_topology(project.path()).expect_err("missing runtime must fail");
    assert!(error.contains("failed to load runtime.toml"), "{error}");
}

#[test]
fn fleet_offline_contract_malformed_runtime_file_is_an_error() {
    let project = TestProject::new("malformed-runtime");
    project.write("runtime.toml", "[resource\nname = 1");
    let error =
        build_project_fleet_topology(project.path()).expect_err("malformed runtime must fail");
    assert!(error.contains("failed to load runtime.toml"), "{error}");
}

#[test]
fn fleet_offline_contract_missing_optional_io_file_is_empty_driver_set() {
    let project = TestProject::new("missing-io");
    project.write("runtime.toml", runtime_toml(""));
    let topology = build(&project);
    let runtime = &topology.hosts[0].runtimes[0];
    assert!(runtime
        .endpoints
        .iter()
        .all(|endpoint| endpoint.kind != "field"));
}

#[test]
fn fleet_offline_contract_malformed_present_io_file_is_an_error() {
    let project = TestProject::new("malformed-io");
    project.write("runtime.toml", runtime_toml(""));
    project.write("io.toml", "[io]\ndrivers = 42");
    let error =
        build_project_fleet_topology(project.path()).expect_err("malformed io.toml must fail");
    assert!(error.contains("failed to load io.toml"), "{error}");
}

#[test]
fn fleet_offline_contract_runtime_and_host_are_configuration_only() {
    let project = TestProject::new("configuration-only");
    project.write("runtime.toml", runtime_toml(""));
    let topology = build(&project);
    assert_eq!(topology.schema_version, 4);
    assert_eq!(topology.hosts.len(), 1);
    let host = &topology.hosts[0];
    assert_eq!(host.source.as_deref(), Some("config"));
    assert_eq!(host.last_seen_ms, None);
    assert_eq!(host.temp_c, None);
    assert_eq!(host.uptime_s, None);
    assert_eq!(host.load, None);
    assert_eq!(host.runtimes.len(), 1);
    let runtime = &host.runtimes[0];
    assert_eq!(runtime.runtime_id, "offline-runtime");
    assert_eq!(runtime.mode, "stopped");
    assert_eq!(runtime.health, "configured_policy");
    assert_eq!(runtime.source.as_deref(), Some("config"));
    assert_eq!(runtime.last_seen_ms, None);
    assert_eq!(runtime.load, None);
    assert_eq!(
        runtime.control_endpoint.as_deref(),
        Some("tcp://127.0.0.1:9900")
    );
}

#[test]
fn fleet_offline_contract_service_endpoints_have_no_live_payload() {
    let project = TestProject::new("services");
    project.write("runtime.toml", runtime_toml(""));
    let topology = build(&project);
    let endpoints = &topology.hosts[0].runtimes[0].endpoints;
    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint.protocol.as_str())
            .collect::<Vec<_>>(),
        ["web", "discovery"]
    );
    for endpoint in endpoints {
        assert_eq!(endpoint.health, "configured_policy");
        assert!(endpoint.live.is_none());
        assert_eq!(endpoint.source.as_deref(), Some("config"));
        assert!(endpoint.owned);
        assert!(!endpoint.supports_test);
    }
}

#[test]
fn fleet_offline_contract_configured_io_order_and_policy_are_preserved() {
    let project = TestProject::new("io-order");
    project.write("runtime.toml", runtime_toml(""));
    project.write(
        "io.toml",
        r#"
[io]
drivers = [
  { name = "mqtt", params = { broker = "broker:1883" } },
  { name = "modbus-tcp", params = { address = "plc:502" } },
  { name = "gpio", enabled = false, params = {} },
]
"#,
    );
    let topology = build(&project);
    let field = topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .filter(|endpoint| endpoint.kind == "field")
        .collect::<Vec<_>>();
    assert_eq!(
        field
            .iter()
            .map(|endpoint| endpoint.protocol.as_str())
            .collect::<Vec<_>>(),
        ["mqtt", "modbus_tcp", "gpio"]
    );
    assert_eq!(field[0].health, "configured_policy");
    assert_eq!(field[1].health, "configured_policy");
    assert_eq!(field[2].health, "disabled");
    assert!(field.iter().all(|endpoint| endpoint.live.is_none()));
}

#[test]
fn fleet_offline_contract_disabled_driver_is_not_testable() {
    let project = TestProject::new("disabled-driver");
    project.write("runtime.toml", runtime_toml(""));
    project.write(
        "io.toml",
        r#"
[io]
drivers = [
  { name = "mqtt", enabled = false, params = { broker = "broker:1883" } },
]
"#,
    );
    let topology = build(&project);
    let mqtt = topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == "mqtt")
        .expect("MQTT endpoint");
    assert_eq!(mqtt.health, "disabled");
    assert!(!mqtt.supports_test);
    assert!(mqtt.live.is_none());
}

#[test]
fn fleet_offline_contract_io_secrets_are_redacted() {
    let secret = "offline-broker-password";
    let project = TestProject::new("io-secret");
    project.write("runtime.toml", runtime_toml(""));
    project.write(
        "io.toml",
        format!(
            r#"
[io]
drivers = [
  {{ name = "mqtt", params = {{ broker = "broker:1883", password = "{secret}" }} }},
]
"#
        ),
    );
    let payload = serialized(&project).to_string();
    assert!(!payload.contains(secret), "{payload}");
    assert!(payload.contains("<redacted>"), "{payload}");
}

#[test]
fn fleet_offline_contract_openot_is_policy_without_live_evidence() {
    let project = TestProject::new("openot");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.openot]
enabled = true
path = "openot.shm"
capacity = 4096
fence_mode = "fenced"
source = "st-fb"
producer_instances = ["Main.Telemetry"]
"#,
        ),
    );
    let topology = build(&project);
    let endpoint = topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == "openot")
        .expect("OpenOT endpoint");
    assert_eq!(endpoint.health, "configured_policy");
    assert!(endpoint.live.is_none());
    assert_eq!(endpoint.source.as_deref(), Some("config"));
}

#[test]
fn fleet_offline_contract_enabled_ads_requires_sidecar() {
    let project = TestProject::new("ads-missing");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.ads]
enabled = true
config_path = "ads.toml"
"#,
        ),
    );
    let error =
        build_project_fleet_topology(project.path()).expect_err("missing ADS sidecar must fail");
    assert!(error.contains("runtime.ads.enabled=true"), "{error}");
    assert!(error.contains("ads.toml"), "{error}");
}

#[test]
fn fleet_offline_contract_disabled_ads_does_not_read_sidecar() {
    let project = TestProject::new("ads-disabled");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.ads]
enabled = false
config_path = "ads.toml"
"#,
        ),
    );
    project.write("ads.toml", "this is not valid ADS TOML");
    let topology = build(&project);
    assert!(topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .all(|endpoint| endpoint.protocol != "ads"));
}

#[test]
fn fleet_offline_contract_valid_ads_projects_endpoint_link_and_external() {
    let project = TestProject::new("ads-valid");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.ads]
enabled = true
config_path = "config/ads.toml"
"#,
        ),
    );
    project.write("config/ads.toml", ads_toml());
    let topology = build(&project);
    let endpoint = topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .find(|endpoint| endpoint.protocol == "ads")
        .expect("ADS endpoint");
    assert_eq!(endpoint.health, "configured_policy");
    assert!(endpoint.supports_test);
    assert!(endpoint.live.is_none());
    assert!(topology
        .links
        .iter()
        .any(|link| link.protocol == "ads" && link.status == "configured_policy"));
    assert!(topology
        .external
        .iter()
        .any(|item| item.id == "external:ads:5.23.91.12.1.1"));
}

#[test]
fn fleet_offline_contract_enabled_opcua_client_requires_sidecar() {
    let project = TestProject::new("opcua-missing");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.opcua_client]
enabled = true
config_path = "opcua_client.toml"
"#,
        ),
    );
    let error =
        build_project_fleet_topology(project.path()).expect_err("missing OPC UA sidecar must fail");
    assert!(
        error.contains("runtime.opcua_client.enabled=true"),
        "{error}"
    );
    assert!(error.contains("opcua_client.toml"), "{error}");
}

#[test]
fn fleet_offline_contract_disabled_opcua_client_does_not_read_sidecar() {
    let project = TestProject::new("opcua-disabled");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.opcua_client]
enabled = false
config_path = "opcua_client.toml"
"#,
        ),
    );
    project.write("opcua_client.toml", "this is not valid OPC UA TOML");
    let topology = build(&project);
    assert!(topology.hosts[0].runtimes[0]
        .endpoints
        .iter()
        .all(|endpoint| endpoint.protocol != "opcua_client"));
}

#[test]
fn fleet_offline_contract_valid_opcua_client_never_serializes_credentials() {
    let secret = "offline-opcua-password";
    let project = TestProject::new("opcua-valid");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.opcua_client]
enabled = true
config_path = "config/opcua_client.toml"
"#,
        ),
    );
    project.write("config/opcua_client.toml", opcua_client_toml(secret));
    let payload = serialized(&project);
    let text = payload.to_string();
    assert!(!text.contains(secret), "{text}");
    assert!(!text.contains("operator"), "{text}");
    let endpoint = payload["hosts"][0]["runtimes"][0]["endpoints"]
        .as_array()
        .and_then(|endpoints| {
            endpoints
                .iter()
                .find(|endpoint| endpoint["protocol"] == "opcua_client")
        })
        .expect("OPC UA client endpoint");
    assert_eq!(endpoint["health"], "configured_policy");
    assert_eq!(endpoint["supports_test"], true);
    assert!(endpoint.get("live").is_none());
}

#[test]
fn fleet_offline_contract_malformed_sidecar_error_does_not_echo_secret() {
    let secret = "malformed-sidecar-secret";
    let project = TestProject::new("sidecar-secret-error");
    project.write(
        "runtime.toml",
        runtime_toml(
            r#"
[runtime.opcua_client]
enabled = true
config_path = "opcua_client.toml"
"#,
        ),
    );
    project.write(
        "opcua_client.toml",
        format!(
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://192.0.2.10:4840"
auth = "username"
username = "operator"
password = "{secret}"
unknown_field = true
"#
        ),
    );
    let error =
        build_project_fleet_topology(project.path()).expect_err("malformed sidecar must fail");
    assert!(error.contains("failed to parse"), "{error}");
    assert!(!error.contains(secret), "{error}");
}

#[test]
fn fleet_offline_contract_absolute_sidecar_path_is_rejected_before_read() {
    let project = TestProject::new("absolute-sidecar-project");
    let external = TestProject::new("absolute-sidecar-external");
    external.write("ads.toml", ads_toml());
    let escaped_path = toml::Value::String(
        external
            .path()
            .join("ads.toml")
            .to_string_lossy()
            .into_owned(),
    )
    .to_string();
    project.write(
        "runtime.toml",
        runtime_toml(
            format!(
                r#"
[runtime.ads]
enabled = true
config_path = {escaped_path}
"#,
            )
            .as_str(),
        ),
    );
    let error = build_project_fleet_topology(project.path())
        .expect_err("absolute sidecar must fail closed");
    assert!(
        error.contains("project") || error.contains("relative") || error.contains("confined"),
        "{error}"
    );
}

#[test]
fn fleet_offline_contract_top_level_discovered_population_is_absent() {
    let project = TestProject::new("no-discovered");
    project.write("runtime.toml", runtime_toml(""));
    let payload = serialized(&project);
    assert!(payload.get("discovered").is_none());
}

#[test]
fn fleet_offline_contract_links_are_sorted_by_stable_id() {
    let project = TestProject::new("sorted-links");
    project.write("runtime.toml", runtime_toml(""));
    project.write(
        "io.toml",
        r#"
[io]
drivers = [
  { name = "modbus-tcp", params = { address = "plc:502" } },
  { name = "mqtt", params = { broker = "broker:1883" } },
  { name = "ethercat", params = { adapter = "eth0" } },
]
"#,
    );
    let topology = build(&project);
    let ids = topology
        .links
        .iter()
        .map(|link| link.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "{ids:?}");
}

#[test]
fn fleet_offline_contract_error_does_not_publish_partial_topology() {
    let project = TestProject::new("no-partial");
    project.write("runtime.toml", runtime_toml(""));
    project.write("io.toml", "[io]\ndriver = 42");
    assert!(build_project_fleet_topology(project.path()).is_err());
}
