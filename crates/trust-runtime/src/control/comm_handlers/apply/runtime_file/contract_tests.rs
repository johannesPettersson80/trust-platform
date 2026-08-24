use super::*;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(0);

struct ApplyProject {
    root: PathBuf,
}

impl ApplyProject {
    fn new(label: &str) -> Self {
        let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-runtime-file-apply-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create runtime file apply project");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create runtime file fixture parent");
        }
        fs::write(&path, contents).expect("write runtime file fixture");
        path
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("read runtime file fixture")
    }
}

impl Drop for ApplyProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn request(
    protocol: &str,
    action: CommApplyAction,
    params: serde_json::Value,
    dry_run: bool,
) -> CommApplyRequest {
    CommApplyRequest {
        protocol: protocol.to_string(),
        action,
        instance_id: None,
        instance_name: None,
        params,
        dry_run,
        credential_channel: Some("trusted_same_host".to_string()),
    }
}

fn runtime_cloud_params() -> serde_json::Value {
    json!({
        "profile": "plant",
        "wan_allow_write": [{
            "action": "cfg_apply",
            "target": "site-b/runtime-1"
        }],
        "link_transports": [{
            "source": "site-a/runtime-1",
            "target": "site-b/runtime-1",
            "transport": "mesh"
        }]
    })
}

fn ads_params(config_path: serde_json::Value) -> serde_json::Value {
    json!({
        "enabled": true,
        "config_path": config_path,
        "worker_tick_interval_ms": 20,
        "connections": [{
            "name": "line1",
            "target_net_id": "5.23.91.12.1.1",
            "host": "192.168.10.5",
            "ams_port": 851,
            "transport": "plain",
            "insecure_transport": true,
            "points": [{
                "symbol": "MAIN.Temperature",
                "var": "line1_temp",
                "type": "REAL"
            }]
        }]
    })
}

fn opcua_client_params(config_path: &str) -> serde_json::Value {
    json!({
        "enabled": true,
        "config_path": config_path,
        "poll_interval_ms": 100,
        "connections": [{
            "name": "line1",
            "endpoint_url": "opc.tcp://127.0.0.1:4840/trust",
            "security_policy": "none",
            "security_mode": "none",
            "auth": "anonymous",
            "trust_server_certificate": false,
            "points": [{
                "var": "conveyor_speed",
                "node_id": "ns=2;s=MAIN.conveyor_speed",
                "type": "REAL",
                "access": "read"
            }]
        }]
    })
}

fn parse_doc(text: &str) -> DocumentMut {
    text.parse().expect("parse runtime test document")
}

#[test]
fn runtime_protocol_sections_are_explicit_and_complete() {
    assert_eq!(runtime_section("opcua"), Some("opcua"));
    assert_eq!(runtime_section("openot"), Some("openot"));
    assert_eq!(runtime_section("discovery"), Some("discovery"));
    assert_eq!(runtime_section("mesh"), Some("mesh"));
    assert_eq!(runtime_section("realtime_t0"), Some("realtime"));
    assert_eq!(runtime_section("runtime_cloud"), Some("cloud"));
    assert_eq!(runtime_section("ads_server"), Some("ads_server"));
    assert_eq!(runtime_section("ads"), None);
    assert_eq!(runtime_section("opcua_client"), None);
    assert_eq!(runtime_section("unknown"), None);
}

#[test]
fn null_params_normalize_to_empty_table() {
    let request = request(
        "runtime_cloud",
        CommApplyAction::Validate,
        serde_json::Value::Null,
        true,
    );
    assert!(normalized_params(&request).unwrap().is_empty());
}

#[test]
fn object_params_preserve_nested_json_types() {
    let request = request(
        "runtime_cloud",
        CommApplyAction::Validate,
        json!({
            "string": "value",
            "integer": 7,
            "float": 1.5,
            "boolean": true,
            "array": ["a", 2],
            "object": { "nested": false }
        }),
        true,
    );
    let params = normalized_params(&request).unwrap();
    assert_eq!(params["string"].as_str(), Some("value"));
    assert_eq!(params["integer"].as_integer(), Some(7));
    assert_eq!(params["float"].as_float(), Some(1.5));
    assert_eq!(params["boolean"].as_bool(), Some(true));
    assert_eq!(params["array"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        params["object"]
            .as_table()
            .and_then(|table| table.get("nested"))
            .and_then(toml::Value::as_bool),
        Some(false)
    );
}

#[test]
fn scalar_and_array_params_are_rejected() {
    for params in [json!("text"), json!(7), json!(true), json!([])] {
        let request = request("runtime_cloud", CommApplyAction::Validate, params, true);
        let errors = normalized_params(&request).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "params");
    }
}

#[test]
fn empty_optional_values_are_removed_before_projection() {
    let request = request(
        "runtime_cloud",
        CommApplyAction::Validate,
        json!({
            "client_id": " ",
            "note": "",
            "auth_token": null,
            "kept": "value"
        }),
        true,
    );
    let params = normalized_params(&request).unwrap();
    assert!(!params.contains_key("client_id"));
    assert!(!params.contains_key("note"));
    assert!(!params.contains_key("auth_token"));
    assert_eq!(params["kept"].as_str(), Some("value"));
}

#[test]
fn secret_fields_require_exact_trusted_channel() {
    let mut untrusted = request(
        "opcua",
        CommApplyAction::Validate,
        json!({ "username": "operator", "password": "secret" }),
        true,
    );
    untrusted.credential_channel = Some("trusted_https_admin".to_string());
    assert_eq!(guard_secret_channel(&untrusted).len(), 1);
    untrusted.credential_channel = None;
    assert_eq!(guard_secret_channel(&untrusted).len(), 1);
    untrusted.credential_channel = Some("trusted_same_host".to_string());
    assert!(guard_secret_channel(&untrusted).is_empty());
}

#[test]
fn secret_guard_never_echoes_secret_value() {
    let mut untrusted = request(
        "opcua",
        CommApplyAction::Validate,
        json!({ "password": "never-echo-this" }),
        true,
    );
    untrusted.credential_channel = Some("untrusted_plain_http_network".to_string());
    let serialized = serde_json::to_string(&guard_secret_channel(&untrusted)).unwrap();
    assert!(!serialized.contains("never-echo-this"));
}

#[test]
fn ensure_table_creates_missing_table() {
    let mut doc = parse_doc("");
    ensure_table(doc.as_table_mut(), "runtime").unwrap();
    assert!(doc["runtime"].is_table());
}

#[test]
fn ensure_table_rejects_existing_scalar_without_replacing_it() {
    let mut doc = parse_doc("runtime = \"preserve\"\n");
    let error = ensure_table(doc.as_table_mut(), "runtime").unwrap_err();
    assert_eq!(error.field, "runtime");
    assert_eq!(doc["runtime"].as_str(), Some("preserve"));
}

#[test]
fn toml_edit_projection_preserves_all_supported_value_shapes() {
    let input: toml::Value = toml::from_str(
        r#"
string = "x"
integer = 7
float = 1.5
boolean = true
datetime = 1979-05-27T07:32:00Z
array = ["x", 2]
inline = { nested = false }
"#,
    )
    .unwrap();
    let projected = item_from_toml(input.as_table().unwrap().get("inline").unwrap());
    assert!(projected.is_table() || projected.is_value());
    let table = edit_table_from_toml(input.as_table().unwrap());
    let text = table.to_string();
    assert!(text.contains("string = \"x\""));
    assert!(text.contains("integer = 7"));
    assert!(text.contains("float = 1.5"));
    assert!(text.contains("boolean = true"));
    assert!(text.contains("datetime"));
    assert!(text.contains("array"));
    assert_eq!(table["inline"]["nested"].as_bool(), Some(false));
}

#[test]
fn generic_runtime_upsert_replaces_only_owned_section() {
    let mut doc = parse_doc(
        r#"
[runtime]
cycle_time_ms = 10

[runtime.discovery]
enabled = true

[runtime.mesh]
role = "peer"
"#,
    );
    let params = toml::toml! {
        role = "router"
        listen = "tcp/0.0.0.0:7447"
    };
    patch_runtime_doc(&mut doc, "mesh", &params, CommApplyAction::Upsert).unwrap();
    let text = doc.to_string();
    assert!(text.contains("cycle_time_ms = 10"));
    assert!(text.contains("[runtime.discovery]"));
    assert!(text.contains("role = \"router\""));
    assert!(!text.contains("role = \"peer\""));
}

#[test]
fn generic_runtime_disable_changes_only_enabled_state() {
    let mut doc = parse_doc(
        r#"
[runtime.mesh]
enabled = true
role = "peer"
listen = "tcp/0.0.0.0:7447"
"#,
    );
    patch_runtime_doc(
        &mut doc,
        "mesh",
        &toml::map::Map::new(),
        CommApplyAction::Disable,
    )
    .unwrap();
    let text = doc.to_string();
    assert!(text.contains("enabled = false"));
    assert!(text.contains("role = \"peer\""));
    assert!(text.contains("listen = \"tcp/0.0.0.0:7447\""));
}

#[test]
fn generic_runtime_remove_preserves_unrelated_sections() {
    let mut doc = parse_doc(
        r#"
[runtime.mesh]
enabled = true

[runtime.discovery]
enabled = true
"#,
    );
    patch_runtime_doc(
        &mut doc,
        "mesh",
        &toml::map::Map::new(),
        CommApplyAction::Remove,
    )
    .unwrap();
    let text = doc.to_string();
    assert!(!text.contains("[runtime.mesh]"));
    assert!(text.contains("[runtime.discovery]"));
}

#[test]
fn unsupported_runtime_protocol_does_not_mutate_document() {
    let original = "[runtime]\ncycle_time_ms = 10\n";
    let mut doc = parse_doc(original);
    let error = patch_runtime_doc(
        &mut doc,
        "unknown",
        &toml::map::Map::new(),
        CommApplyAction::Upsert,
    )
    .unwrap_err();
    assert_eq!(error.field, "protocol");
    assert_eq!(doc.to_string(), original);
}

#[test]
fn ads_server_update_preserves_existing_clients_when_omitted() {
    let mut doc = parse_doc(
        r#"
[[runtime.ads_server.clients]]
ams_net_id = "5.23.91.12.1.1"
source_ip = "192.168.10.50"
"#,
    );
    let params = toml::toml! {
        enabled = true
        listen = "127.0.0.1"
        expose = ["global.*"]
    };
    patch_runtime_doc(&mut doc, "ads_server", &params, CommApplyAction::Upsert).unwrap();
    let text = doc.to_string();
    assert!(text.contains("[[runtime.ads_server.clients]]"));
    assert!(text.contains("ams_net_id = \"5.23.91.12.1.1\""));
    assert!(text.contains("expose = [\"global.*\"]"));
}

#[test]
fn ads_server_update_preserves_clients_when_empty_array_is_supplied() {
    let mut doc = parse_doc(
        r#"
[[runtime.ads_server.clients]]
ams_net_id = "5.23.91.12.1.1"
"#,
    );
    let params = toml::toml! {
        enabled = true
        clients = []
    };
    patch_runtime_doc(&mut doc, "ads_server", &params, CommApplyAction::Edit).unwrap();
    assert!(doc.to_string().contains("ams_net_id = \"5.23.91.12.1.1\""));
}

#[test]
fn runtime_cloud_projection_uses_nested_owned_keys() {
    let mut doc = parse_doc("[runtime]\ncycle_time_ms = 10\n");
    let params = normalized_params(&request(
        "runtime_cloud",
        CommApplyAction::Upsert,
        runtime_cloud_params(),
        false,
    ))
    .unwrap();
    patch_runtime_doc(&mut doc, "runtime_cloud", &params, CommApplyAction::Upsert).unwrap();
    let text = doc.to_string();
    assert!(text.contains("[runtime.cloud]"));
    assert!(text.contains("profile = \"plant\""));
    assert!(text.contains("[runtime.cloud.wan]"));
    assert!(
        text.contains("allow_write = [{ action = \"cfg_apply\", target = \"site-b/runtime-1\" }]")
    );
    assert!(text.contains("[runtime.cloud.links]"));
    assert!(text.contains(
        "transports = [{ source = \"site-a/runtime-1\", target = \"site-b/runtime-1\", transport = \"mesh\" }]"
    ));
}

#[test]
fn runtime_cloud_disable_removes_owned_section() {
    let mut doc = parse_doc(
        r#"
[runtime.cloud]
profile = "plant"

[runtime.discovery]
enabled = true
"#,
    );
    patch_runtime_doc(
        &mut doc,
        "runtime_cloud",
        &toml::map::Map::new(),
        CommApplyAction::Disable,
    )
    .unwrap();
    let text = doc.to_string();
    assert!(!text.contains("[runtime.cloud]"));
    assert!(text.contains("[runtime.discovery]"));
}

#[test]
fn ads_runtime_projection_uses_documented_defaults() {
    let mut doc = parse_doc("[runtime]\ncycle_time_ms = 10\n");
    patch_ads_runtime_doc(&mut doc, &toml::map::Map::new(), CommApplyAction::Upsert).unwrap();
    let text = doc.to_string();
    assert!(text.contains("[runtime.ads]"));
    assert!(text.contains("enabled = true"));
    assert!(text.contains("config_path = \"ads.toml\""));
    assert!(text.contains("worker_tick_interval_ms = 20"));
}

#[test]
fn ads_disable_retains_existing_configuration() {
    let mut doc = parse_doc(
        r#"
[runtime.ads]
enabled = true
config_path = "custom/ads.toml"
worker_tick_interval_ms = 75
"#,
    );
    patch_ads_runtime_doc(&mut doc, &toml::map::Map::new(), CommApplyAction::Disable).unwrap();
    let text = doc.to_string();
    assert!(text.contains("enabled = false"));
    assert!(text.contains("config_path = \"custom/ads.toml\""));
    assert!(text.contains("worker_tick_interval_ms = 75"));
}

#[test]
fn opcua_client_projection_uses_documented_defaults() {
    let mut doc = parse_doc("[runtime]\ncycle_time_ms = 10\n");
    patch_opcua_client_runtime_doc(&mut doc, &toml::map::Map::new(), CommApplyAction::Upsert)
        .unwrap();
    let text = doc.to_string();
    assert!(text.contains("[runtime.opcua_client]"));
    assert!(text.contains("enabled = true"));
    assert!(text.contains("config_path = \"opcua_client.toml\""));
    assert!(text.contains("poll_interval_ms = 250"));
}

#[test]
fn runtime_subsection_string_reads_only_string_leaf() {
    let doc = parse_doc(
        r#"
[runtime.ads]
config_path = "custom/ads.toml"
worker_tick_interval_ms = 20
"#,
    );
    assert_eq!(
        runtime_subsection_string(&doc, "ads", "config_path").as_deref(),
        Some("custom/ads.toml")
    );
    assert!(runtime_subsection_string(&doc, "ads", "worker_tick_interval_ms").is_none());
    assert!(runtime_subsection_string(&doc, "missing", "config_path").is_none());
}

#[test]
fn ads_sidecar_renderer_emits_parseable_connections_table() {
    let params = normalized_params(&request(
        "ads",
        CommApplyAction::Validate,
        ads_params(json!("ads.toml")),
        true,
    ))
    .unwrap();
    let text = render_ads_toml(connections_array(params.get("connections")).unwrap()).unwrap();
    crate::ads::parse_ads_toml(&text).expect("rendered ADS sidecar");
    assert!(text.contains("[[connections]]"));
    assert!(text.contains("name = \"line1\""));
}

#[test]
fn opcua_sidecar_renderer_inherits_default_poll_interval() {
    let params = normalized_params(&request(
        "opcua_client",
        CommApplyAction::Validate,
        opcua_client_params("opcua_client.toml"),
        true,
    ))
    .unwrap();
    let text = render_opcua_client_toml(
        connections_array(params.get("connections")).unwrap(),
        Some(333),
    )
    .unwrap();
    crate::opcua::parse_opcua_client_toml(&text).expect("rendered OPC UA sidecar");
    assert!(text.contains("poll_interval_ms = 333"));
}

#[test]
fn opcua_sidecar_renderer_preserves_connection_specific_poll_interval() {
    let connections = vec![toml::toml! {
        name = "line1"
        endpoint_url = "opc.tcp://127.0.0.1:4840/trust"
        security_policy = "none"
        security_mode = "none"
        auth = "anonymous"
        trust_server_certificate = false
        poll_interval_ms = 777
        points = []
    }
    .into()];
    let text = render_opcua_client_toml(&connections, Some(333)).unwrap();
    assert!(text.contains("poll_interval_ms = 777"));
    assert!(!text.contains("poll_interval_ms = 333"));
}

#[test]
fn missing_runtime_file_produces_valid_default_document_without_writing() {
    let project = ApplyProject::new("default-document");
    let path = project.root.join("runtime.toml");
    let doc = load_runtime_doc(&path, "runtime_cloud", CommApplyAction::Validate).unwrap();
    crate::config::validate_runtime_toml_text(&doc.to_string()).expect("default runtime document");
    assert!(!path.exists());
}

#[test]
fn malformed_runtime_file_fails_without_replacing_input() {
    let project = ApplyProject::new("malformed-input");
    let original = "[runtime\ninvalid = true";
    project.write("runtime.toml", original);
    let response = apply_runtime_protocol(
        Some(&project.root),
        "runtime_cloud".to_string(),
        request(
            "runtime_cloud",
            CommApplyAction::Upsert,
            runtime_cloud_params(),
            false,
        ),
    );
    assert!(!response.applied);
    assert_eq!(project.read("runtime.toml"), original);
}

#[test]
fn dry_run_validates_without_creating_project_or_files() {
    let project = ApplyProject::new("dry-run");
    fs::remove_dir_all(&project.root).expect("remove project before dry run");
    let response = apply_runtime_protocol(
        Some(&project.root),
        "runtime_cloud".to_string(),
        request(
            "runtime_cloud",
            CommApplyAction::Upsert,
            runtime_cloud_params(),
            true,
        ),
    );
    assert!(!response.applied);
    assert_eq!(response.lifecycle_effect, "validate_only");
    assert!(!project.root.exists());
}

#[test]
fn identical_runtime_apply_is_byte_idempotent() {
    let project = ApplyProject::new("idempotent");
    for _ in 0..2 {
        let response = apply_runtime_protocol(
            Some(&project.root),
            "runtime_cloud".to_string(),
            request(
                "runtime_cloud",
                CommApplyAction::Upsert,
                runtime_cloud_params(),
                false,
            ),
        );
        assert!(response.applied, "{:?}", response.field_errors);
        let bytes = fs::read(project.root.join("runtime.toml")).unwrap();
        if project.root.join("first.bin").exists() {
            assert_eq!(bytes, fs::read(project.root.join("first.bin")).unwrap());
        } else {
            fs::write(project.root.join("first.bin"), bytes).unwrap();
        }
    }
}

#[test]
fn absolute_ads_sidecar_path_is_rejected_before_any_write() {
    let project = ApplyProject::new("absolute-path");
    let outside = project.root.join("outside/ads.toml");
    let selected_root = project.root.join("selected");
    let response = apply_runtime_protocol(
        Some(&selected_root),
        "ads".to_string(),
        request(
            "ads",
            CommApplyAction::Upsert,
            ads_params(json!(outside.to_string_lossy())),
            false,
        ),
    );
    assert!(!response.applied);
    assert!(!selected_root.join("runtime.toml").exists());
    assert!(!outside.exists());
}

#[test]
fn parent_traversing_sidecar_path_is_rejected_before_any_write() {
    let project = ApplyProject::new("parent-traversal");
    let selected_root = project.root.join("selected");
    let outside = project.root.join("escape.toml");
    let response = apply_runtime_protocol(
        Some(&selected_root),
        "ads".to_string(),
        request(
            "ads",
            CommApplyAction::Upsert,
            ads_params(json!("../escape.toml")),
            false,
        ),
    );
    assert!(!response.applied);
    assert!(!selected_root.join("runtime.toml").exists());
    assert!(!outside.exists());
}

#[cfg(unix)]
#[test]
fn symlink_escaping_sidecar_path_is_rejected_before_any_write() {
    use std::os::unix::fs::symlink;

    let project = ApplyProject::new("symlink-escape");
    let selected_root = project.root.join("selected");
    let outside = project.root.join("outside");
    fs::create_dir_all(&selected_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, selected_root.join("link")).unwrap();
    let response = apply_runtime_protocol(
        Some(&selected_root),
        "ads".to_string(),
        request(
            "ads",
            CommApplyAction::Upsert,
            ads_params(json!("link/ads.toml")),
            false,
        ),
    );
    assert!(!response.applied);
    assert!(!selected_root.join("runtime.toml").exists());
    assert!(!outside.join("ads.toml").exists());
}

#[test]
fn sidecar_write_failure_preserves_original_runtime_document() {
    let project = ApplyProject::new("sidecar-failure");
    let original = "[runtime]\ncycle_time_ms = 10\n";
    project.write("runtime.toml", original);
    project.write("blocker", "not a directory");
    let response = apply_runtime_protocol(
        Some(&project.root),
        "opcua_client".to_string(),
        request(
            "opcua_client",
            CommApplyAction::Upsert,
            opcua_client_params("blocker/client.toml"),
            false,
        ),
    );
    assert!(!response.applied);
    assert_eq!(project.read("runtime.toml"), original);
}

#[test]
fn sidecar_remove_failure_preserves_original_runtime_document() {
    let project = ApplyProject::new("remove-failure");
    let original = r#"
[runtime]
cycle_time_ms = 10

[runtime.opcua_client]
enabled = true
config_path = "sidecar"
poll_interval_ms = 100
"#;
    project.write("runtime.toml", original);
    fs::create_dir_all(project.root.join("sidecar")).unwrap();
    let response = apply_runtime_protocol(
        Some(&project.root),
        "opcua_client".to_string(),
        request("opcua_client", CommApplyAction::Remove, json!({}), false),
    );
    assert!(!response.applied);
    assert_eq!(project.read("runtime.toml"), original);
    assert!(project.root.join("sidecar").is_dir());
}

#[test]
fn blocked_secret_apply_changes_no_files_and_echoes_no_secret() {
    let project = ApplyProject::new("secret-block");
    let mut request = request(
        "opcua",
        CommApplyAction::Upsert,
        json!({
            "listen": "127.0.0.1:4840",
            "endpoint_path": "/trust",
            "namespace_uri": "urn:trust",
            "allow_anonymous": false,
            "username": "operator",
            "password": "never-persist-this"
        }),
        false,
    );
    request.credential_channel = Some("untrusted_plain_http_network".to_string());
    let response = apply_runtime_protocol(Some(&project.root), "opcua".to_string(), request);
    assert!(!response.applied);
    assert!(!project.root.join("runtime.toml").exists());
    assert!(!serde_json::to_string(&response)
        .unwrap()
        .contains("never-persist-this"));
}
