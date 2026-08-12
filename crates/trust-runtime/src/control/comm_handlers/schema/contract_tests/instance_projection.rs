use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value as JsonValue};

use super::super::*;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-comm-schema-contract-{name}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create communication schema test project");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn configured_instances_are_protocol_qualified_filtered_and_stably_named() {
    let project = TempProject::new("instances");
    fs::write(
        project.path().join("io.toml"),
        r#"
[io]
safe_state = []

[[io.drivers]]
name = "modbus-tcp"
params = { address = "127.0.0.1:1502", unit_id = 1, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }

[[io.drivers]]
name = "loopback"
params = { input_count = 8, output_count = 8, scan_period_ms = 10, mode = "mirror" }

[[io.drivers]]
name = "modbus-tcp"
params = { address = "127.0.0.1:2502", unit_id = 2, input_start = 0, output_start = 0, timeout_ms = 500, on_error = "fault" }
"#,
    )
    .expect("write io.toml");

    let value = comm_schema_value(None, Some(project.path())).expect("project schema");
    let protocols = value["protocols"].as_array().expect("protocols");
    let modbus = protocols
        .iter()
        .find(|protocol| protocol["id"] == "modbus_tcp")
        .expect("modbus schema");
    let instances = modbus["instances"].as_array().expect("modbus instances");
    assert_eq!(instances.len(), 2);
    assert_eq!(instances[0]["id"], "modbus_tcp:0");
    assert_eq!(instances[0]["display_name"], "Modbus TCP 127.0.0.1:1502");
    assert_eq!(instances[1]["id"], "modbus_tcp:2");
    assert_eq!(instances[1]["display_name"], "Modbus TCP 127.0.0.1:2502");
    assert!(instances
        .iter()
        .all(|instance| instance["driver"] == "modbus-tcp"));
}

#[test]
fn configured_instance_projection_never_serializes_persisted_secrets() {
    let project = TempProject::new("secret-redaction");
    fs::write(
        project.path().join("io.toml"),
        r#"
[io]
safe_state = []

[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883", topic_in = "trust/io/in", topic_out = "trust/io/out", username = "operator", password = "schema-leak-sentinel", tls = false, allow_insecure_remote = false, reconnect_ms = 500, keep_alive_s = 5, on_error = "fault", tls_alpn = [] }
"#,
    )
    .expect("write secret-bearing io.toml");

    let value = comm_schema_value(Some(&json!({ "protocol": "mqtt" })), Some(project.path()))
        .expect("project schema");
    let text = serde_json::to_string(&value).expect("serialize project schema");
    assert!(
        !text.contains("schema-leak-sentinel"),
        "comm.schema exposed a persisted credential: {text}"
    );
    let params = &value["protocols"][0]["instances"][0]["params"];
    assert!(
        params.get("password").is_none() || params["password"].is_null(),
        "secret key must be omitted or redacted: {params}"
    );
}

#[test]
fn malformed_io_configuration_does_not_create_phantom_instances() {
    let project = TempProject::new("malformed-io");
    fs::write(project.path().join("io.toml"), "[io\nnot toml").expect("write malformed io.toml");
    let value = comm_schema_value(None, Some(project.path())).expect("schema remains available");
    for protocol in value["protocols"].as_array().expect("protocols") {
        assert_eq!(protocol["instances"], JsonValue::Array(Vec::new()));
    }
}
