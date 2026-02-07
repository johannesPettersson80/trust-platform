//! Embedded browser UI server.

#![allow(missing_docs)]

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use qrcode::{render::svg, QrCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::bundle_template::IoConfigTemplate;
use crate::config::{load_system_io_config, IoConfig, RuntimeConfig, WebAuthMode, WebConfig};
use crate::control::{handle_request_value, ControlState};
use crate::debug::dap::format_value;
use crate::discovery::DiscoveryState;
use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSize};
use crate::memory::IoArea;
use crate::setup::SetupOptions;

mod deploy;
pub mod pairing;

use deploy::{apply_deploy, apply_rollback, DeployRequest};
use pairing::PairingStore;

#[derive(Debug, Deserialize)]
struct SetupApplyRequest {
    #[serde(alias = "bundle_path")]
    project_path: Option<String>,
    resource_name: Option<String>,
    cycle_ms: Option<u64>,
    driver: Option<String>,
    write_system_io: Option<bool>,
    overwrite_system_io: Option<bool>,
    use_system_io: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RollbackRequest {
    restart: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IoConfigRequest {
    driver: String,
    params: Option<serde_json::Value>,
    safe_state: Option<Vec<IoSafeStateEntry>>,
    use_system_io: Option<bool>,
}

#[derive(Debug, Serialize)]
struct IoConfigResponse {
    driver: String,
    params: serde_json::Value,
    safe_state: Vec<IoSafeStateEntry>,
    source: String,
    use_system_io: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IoSafeStateEntry {
    address: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct SetupDefaultsResponse {
    project_path: String,
    resource_name: String,
    cycle_ms: u64,
    driver: String,
    use_system_io: bool,
    system_io_exists: bool,
    write_system_io: bool,
    needs_setup: bool,
}

const INDEX_HTML: &str = include_str!("web/ui/index.html");
const APP_JS: &str = include_str!("web/ui/app.js");
const APP_CSS: &str = include_str!("web/ui/styles.css");
const HMI_HTML: &str = include_str!("web/ui/hmi.html");
const HMI_JS: &str = include_str!("web/ui/hmi.js");
const HMI_CSS: &str = include_str!("web/ui/hmi.css");

fn default_bundle_root(bundle_root: &Option<PathBuf>) -> PathBuf {
    bundle_root
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_resource_name(bundle_root: &Path) -> SmolStr {
    let project_name = bundle_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("trust-plc");
    SmolStr::new(project_name.replace(|c: char| !c.is_ascii_alphanumeric(), "_"))
}

fn detect_default_driver() -> String {
    if crate::setup::is_raspberry_pi_hint() {
        "gpio".to_string()
    } else {
        "loopback".to_string()
    }
}

fn setup_defaults(bundle_root: &Option<PathBuf>) -> SetupDefaultsResponse {
    let project_path = default_bundle_root(bundle_root);
    let runtime_path = project_path.join("runtime.toml");
    let io_path = project_path.join("io.toml");

    let runtime_loaded = if runtime_path.exists() {
        RuntimeConfig::load(&runtime_path).ok()
    } else {
        None
    };
    let (resource_name, cycle_ms) = if let Some(runtime) = runtime_loaded.as_ref() {
        (
            runtime.resource_name.to_string(),
            runtime.cycle_interval.as_millis() as u64,
        )
    } else {
        (default_resource_name(&project_path).to_string(), 100)
    };

    let system_io = load_system_io_config().ok().flatten();
    let system_io_exists = system_io.is_some();

    let (driver, use_system_io) = if io_path.exists() {
        match IoConfig::load(&io_path) {
            Ok(io) => (io.driver.to_string(), false),
            Err(_) => (detect_default_driver(), system_io_exists),
        }
    } else if let Some(system_io) = system_io {
        (system_io.driver.to_string(), true)
    } else {
        (detect_default_driver(), false)
    };

    let write_system_io = !system_io_exists;
    let needs_setup = runtime_loaded.is_none() || (!io_path.exists() && !system_io_exists);

    SetupDefaultsResponse {
        project_path: project_path.display().to_string(),
        resource_name,
        cycle_ms,
        driver,
        use_system_io,
        system_io_exists,
        write_system_io,
        needs_setup,
    }
}

fn json_to_toml(value: &serde_json::Value) -> toml::Value {
    match value {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(value) => toml::Value::Boolean(*value),
        serde_json::Value::Number(value) => {
            if let Some(i) = value.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(u) = value.as_u64() {
                toml::Value::Integer(u as i64)
            } else if let Some(f) = value.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(value.to_string())
            }
        }
        serde_json::Value::String(value) => toml::Value::String(value.clone()),
        serde_json::Value::Array(values) => {
            toml::Value::Array(values.iter().map(json_to_toml).collect())
        }
        serde_json::Value::Object(values) => {
            let mut table = toml::map::Map::new();
            for (key, value) in values {
                table.insert(key.clone(), json_to_toml(value));
            }
            toml::Value::Table(table)
        }
    }
}

fn parse_io_toml(text: &str) -> Result<(String, toml::Value, Vec<IoSafeStateEntry>), RuntimeError> {
    let raw: toml::Value = toml::from_str(text)
        .map_err(|err| RuntimeError::InvalidConfig(format!("io.toml: {err}").into()))?;
    let io = raw
        .get("io")
        .and_then(|value| value.as_table())
        .ok_or_else(|| RuntimeError::InvalidConfig("io.toml missing [io]".into()))?;
    let driver = io
        .get("driver")
        .and_then(|value| value.as_str())
        .unwrap_or("loopback")
        .to_string();
    let params = io
        .get("params")
        .cloned()
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let safe_state = io
        .get("safe_state")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_table())
                .filter_map(|entry| {
                    let address = entry.get("address")?.as_str()?.to_string();
                    let value = entry.get("value")?.as_str()?.to_string();
                    Some(IoSafeStateEntry { address, value })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((driver, params, safe_state))
}

fn load_io_config(bundle_root: &Option<PathBuf>) -> Result<IoConfigResponse, RuntimeError> {
    let project_root = default_bundle_root(bundle_root);
    let project_io = project_root.join("io.toml");
    if project_io.is_file() {
        let text = std::fs::read_to_string(&project_io).map_err(|err| {
            RuntimeError::InvalidConfig(format!("failed to read io.toml: {err}").into())
        })?;
        let (driver, params, safe_state) = parse_io_toml(&text)?;
        let params_json = serde_json::to_value(&params).unwrap_or_else(|_| json!({}));
        return Ok(IoConfigResponse {
            driver,
            params: params_json,
            safe_state,
            source: "project".to_string(),
            use_system_io: false,
        });
    }
    if let Some(system) = load_system_io_config().ok().flatten() {
        let params_json = serde_json::to_value(&system.params).unwrap_or_else(|_| json!({}));
        let safe_state = system
            .safe_state
            .outputs
            .iter()
            .map(|(address, value)| IoSafeStateEntry {
                address: format_io_address(address),
                value: format_value(value),
            })
            .collect::<Vec<_>>();
        return Ok(IoConfigResponse {
            driver: system.driver.to_string(),
            params: params_json,
            safe_state,
            source: "system".to_string(),
            use_system_io: true,
        });
    }
    Ok(IoConfigResponse {
        driver: detect_default_driver(),
        params: json!({}),
        safe_state: Vec::new(),
        source: "default".to_string(),
        use_system_io: false,
    })
}

fn render_io_toml(driver: &str, params: toml::Value, safe_state: Vec<IoSafeStateEntry>) -> String {
    let template = IoConfigTemplate {
        driver: driver.to_string(),
        params,
        safe_state: safe_state
            .into_iter()
            .map(|entry| (entry.address, entry.value))
            .collect(),
    };
    crate::bundle_template::render_io_toml(&template)
}

fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{}*", area);
    }
    if matches!(address.size, IoSize::Bit) {
        format!("%{}{}{}.{}", area, size, address.byte, address.bit)
    } else {
        format!("%{}{}{}", area, size, address.byte)
    }
}

fn list_sources(bundle_root: &Path) -> Vec<String> {
    let sources_dir = bundle_root.join("sources");
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(&sources_dir) else {
        return list;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("st") {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
            list.push(name.to_string());
        }
    }
    list.sort();
    list
}

fn read_source_file(bundle_root: &Path, name: &str) -> Result<String, RuntimeError> {
    let sources_dir = bundle_root.join("sources");
    let requested = sources_dir.join(name);
    let sources_dir = sources_dir.canonicalize().map_err(|err| {
        RuntimeError::InvalidConfig(format!("sources dir unavailable: {err}").into())
    })?;
    let requested = requested
        .canonicalize()
        .map_err(|err| RuntimeError::InvalidConfig(format!("source not found: {err}").into()))?;
    if !requested.starts_with(&sources_dir) {
        return Err(RuntimeError::InvalidConfig("invalid source path".into()));
    }
    std::fs::read_to_string(&requested)
        .map_err(|err| RuntimeError::InvalidConfig(format!("failed to read source: {err}").into()))
}

fn apply_setup(
    bundle_root: &Option<PathBuf>,
    payload: SetupApplyRequest,
) -> Result<String, RuntimeError> {
    let defaults = setup_defaults(bundle_root);
    let bundle_path = payload
        .project_path
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.project_path.clone());
    let bundle_root = PathBuf::from(bundle_path);
    std::fs::create_dir_all(&bundle_root).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to create project folder: {err}").into())
    })?;

    let resource_name = payload
        .resource_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.resource_name);
    let cycle_ms = payload.cycle_ms.unwrap_or(defaults.cycle_ms);
    let mut driver = payload
        .driver
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.driver);
    if driver == "auto" {
        driver = detect_default_driver();
    }

    let use_system_io = payload.use_system_io.unwrap_or(defaults.use_system_io);
    let write_system_io = payload.write_system_io.unwrap_or(defaults.write_system_io);
    let overwrite_system_io = payload.overwrite_system_io.unwrap_or(false);

    let runtime_path = bundle_root.join("runtime.toml");
    let runtime_text =
        crate::bundle_template::render_runtime_toml(&SmolStr::new(resource_name), cycle_ms);
    std::fs::write(&runtime_path, runtime_text).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to write runtime.toml: {err}").into())
    })?;

    let io_path = bundle_root.join("io.toml");
    if use_system_io {
        if io_path.exists() {
            std::fs::remove_file(&io_path).map_err(|err| {
                RuntimeError::InvalidConfig(format!("failed to remove io.toml: {err}").into())
            })?;
        }
    } else {
        let template =
            crate::bundle_template::build_io_config_auto(driver.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(format!("io template error: {err}").into())
            })?;
        let io_text = crate::bundle_template::render_io_toml(&template);
        std::fs::write(&io_path, io_text).map_err(|err| {
            RuntimeError::InvalidConfig(format!("failed to write io.toml: {err}").into())
        })?;
    }

    if write_system_io {
        let options = SetupOptions {
            driver: Some(SmolStr::new(driver)),
            backend: None,
            force: overwrite_system_io,
            path: None,
        };
        crate::setup::run_setup(options)?;
    }

    Ok("✓ Setup applied. Restart the runtime to load the new configuration.".to_string())
}

pub struct WebServer {
    // Retained to keep the web thread alive for the lifetime of the server handle.
    #[allow(dead_code)]
    handle: thread::JoinHandle<()>,
    pub listen: String,
}

pub fn start_web_server(
    config: &WebConfig,
    control_state: Arc<ControlState>,
    discovery: Option<Arc<DiscoveryState>>,
    pairing: Option<Arc<PairingStore>>,
    bundle_root: Option<PathBuf>,
) -> Result<WebServer, RuntimeError> {
    if !config.enabled {
        return Err(RuntimeError::ControlError("web disabled".into()));
    }
    let listen = config.listen.to_string();
    let server = Server::http(&listen)
        .map_err(|err| RuntimeError::ControlError(format!("web bind: {err}").into()))?;
    let auth = config.auth;
    let web_url = format_web_url(&listen);
    let auth_token = control_state.auth_token.clone();
    let discovery = discovery.unwrap_or_else(|| Arc::new(DiscoveryState::new()));
    let pairing = pairing.or_else(|| {
        bundle_root
            .as_ref()
            .map(|root| Arc::new(PairingStore::load(root.join("pairings.json"))))
    });
    let bundle_root = bundle_root.clone();
    let handle = thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let method = request.method().clone();
            let url = request.url().to_string();
            if method == Method::Get && (url == "/" || url == "/setup") {
                let response = Response::from_string(INDEX_HTML)
                    .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && (url == "/hmi" || url == "/hmi/") {
                let response = Response::from_string(HMI_HTML)
                    .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/hmi/export.json" {
                let schema_response = handle_request_value(
                    json!({
                        "id": 1_u64,
                        "type": "hmi.schema.get"
                    }),
                    &control_state,
                    None,
                );
                let schema_payload = serde_json::to_value(schema_response)
                    .unwrap_or_else(|_| json!({ "ok": false }));
                let ok = schema_payload
                    .get("ok")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if !ok {
                    let response =
                        Response::from_string(json!({ "error": "schema unavailable" }).to_string())
                            .with_status_code(503)
                            .with_header(
                                Header::from_bytes("Content-Type", "application/json").unwrap(),
                            );
                    let _ = request.respond(response);
                    continue;
                }
                let schema = schema_payload
                    .get("result")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let exported_at_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let payload = json!({
                    "version": 1_u32,
                    "exported_at_ms": exported_at_ms,
                    "entrypoint": "hmi/index.html",
                    "routes": ["/hmi", "/hmi/app.js", "/hmi/styles.css", "/api/control"],
                    "config": {
                        "poll_ms": 500_u32,
                        "schema": schema
                    },
                    "assets": {
                        "hmi/index.html": HMI_HTML,
                        "hmi/styles.css": HMI_CSS,
                        "hmi/app.js": HMI_JS
                    }
                });
                let response = Response::from_string(payload.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                    .with_header(
                        Header::from_bytes(
                            "Content-Disposition",
                            "attachment; filename=\"trust-hmi-export.json\"",
                        )
                        .unwrap(),
                    );
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/styles.css" {
                let response = Response::from_string(APP_CSS)
                    .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/hmi/styles.css" {
                let response = Response::from_string(HMI_CSS)
                    .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/app.js" {
                let response = Response::from_string(APP_JS).with_header(
                    Header::from_bytes("Content-Type", "application/javascript").unwrap(),
                );
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/hmi/app.js" {
                let response = Response::from_string(HMI_JS).with_header(
                    Header::from_bytes("Content-Type", "application/javascript").unwrap(),
                );
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url.starts_with("/api/qr") {
                let text = url.split('?').nth(1).and_then(|query| {
                    query.split('&').find_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        if parts.next()? == "text" {
                            Some(parts.next().unwrap_or("").to_string())
                        } else {
                            None
                        }
                    })
                });
                if let Some(encoded) = text {
                    let decoded =
                        urlencoding::decode(&encoded).unwrap_or_else(|_| encoded.as_str().into());
                    match render_qr_svg(decoded.as_ref()) {
                        Ok(svg) => {
                            let response = Response::from_string(svg).with_header(
                                Header::from_bytes("Content-Type", "image/svg+xml").unwrap(),
                            );
                            let _ = request.respond(response);
                        }
                        Err(err) => {
                            let response = Response::from_string(format!("error: {err}"))
                                .with_status_code(500);
                            let _ = request.respond(response);
                        }
                    }
                } else {
                    let response = Response::from_string("missing text").with_status_code(400);
                    let _ = request.respond(response);
                }
                continue;
            }
            if method == Method::Get && url == "/api/discovery" {
                let items = discovery
                    .snapshot()
                    .into_iter()
                    .map(|entry| {
                        json!({
                            "id": entry.id.as_str(),
                            "name": entry.name.as_str(),
                            "addresses": entry.addresses,
                            "web_port": entry.web_port,
                            "mesh_port": entry.mesh_port,
                            "control": entry.control.as_ref().map(|v| v.as_str()),
                        })
                    })
                    .collect::<Vec<_>>();
                let body = json!({ "items": items }).to_string();
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url.starts_with("/api/probe") {
                let target = url
                    .split_once('?')
                    .map(|(_, query)| query)
                    .and_then(|query| {
                        query
                            .split('&')
                            .find(|part| part.starts_with("url="))
                            .map(|part| part.trim_start_matches("url="))
                    })
                    .map(decode_url_component);
                let target = match target {
                    Some(value)
                        if value.starts_with("http://") || value.starts_with("https://") =>
                    {
                        value.trim_end_matches('/').to_string()
                    }
                    _ => {
                        let response = Response::from_string(
                            json!({ "ok": false, "error": "invalid url" }).to_string(),
                        )
                        .with_status_code(400);
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let control_url = format!("{target}/api/control");
                let agent = ureq::AgentBuilder::new()
                    .timeout_connect(Duration::from_millis(500))
                    .timeout_read(Duration::from_millis(800))
                    .build();
                let body = json!({ "id": 1u64, "type": "status" }).to_string();
                let response_body = agent
                    .post(&control_url)
                    .set("Content-Type", "application/json")
                    .send_string(&body);
                let payload = match response_body {
                    Ok(resp) => {
                        let text = resp.into_string().unwrap_or_default();
                        parse_probe_response(&text)
                    }
                    Err(ureq::Error::Status(401, _)) => {
                        json!({ "ok": false, "error": "auth_required" })
                    }
                    Err(_) => json!({ "ok": false, "error": "unreachable" }),
                };
                let response = Response::from_string(payload.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/api/setup/defaults" {
                let defaults = setup_defaults(&bundle_root);
                let body = serde_json::to_string(&defaults).unwrap_or_else(|_| "{}".to_string());
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/setup/apply" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string("invalid body").with_status_code(400);
                    let _ = request.respond(response);
                    continue;
                }
                let payload: SetupApplyRequest = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string("invalid json").with_status_code(400);
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let response_body = match apply_setup(&bundle_root, payload) {
                    Ok(message) => message,
                    Err(err) => format!("error: {err}"),
                };
                let response = Response::from_string(response_body)
                    .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/api/io/config" {
                let body = match load_io_config(&bundle_root) {
                    Ok(config) => {
                        serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string())
                    }
                    Err(err) => json!({ "error": err.to_string() }).to_string(),
                };
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/io/config" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string("invalid body").with_status_code(400);
                    let _ = request.respond(response);
                    continue;
                }
                let payload: IoConfigRequest = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string("invalid json").with_status_code(400);
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let project_root = default_bundle_root(&bundle_root);
                let io_path = project_root.join("io.toml");
                let use_system = payload.use_system_io.unwrap_or(false);
                let response_body = if use_system {
                    if io_path.exists() {
                        if let Err(err) = std::fs::remove_file(&io_path) {
                            format!("error: failed to remove io.toml: {err}")
                        } else {
                            "✓ Using system I/O config. Restart the runtime to apply.".to_string()
                        }
                    } else {
                        "✓ Using system I/O config. Restart the runtime to apply.".to_string()
                    }
                } else {
                    let params_json = payload.params.unwrap_or_else(|| json!({}));
                    let params_toml = json_to_toml(&params_json);
                    let safe_state = payload.safe_state.unwrap_or_default();
                    let io_text = render_io_toml(payload.driver.as_str(), params_toml, safe_state);
                    match std::fs::write(&io_path, io_text) {
                        Ok(_) => "✓ I/O config saved. Restart the runtime to apply.".to_string(),
                        Err(err) => format!("error: failed to write io.toml: {err}"),
                    }
                };
                let response = Response::from_string(response_body)
                    .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/io/modbus-test" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string("invalid body").with_status_code(400);
                    let _ = request.respond(response);
                    continue;
                }
                let payload: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string("invalid json").with_status_code(400);
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let address = payload
                    .get("address")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let port = payload.get("port").and_then(|v| v.as_u64()).unwrap_or(502);
                let timeout_ms = payload
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(500);
                let target = if address.contains(':') {
                    address.to_string()
                } else {
                    format!("{address}:{port}")
                };
                let result = target
                    .to_socket_addrs()
                    .ok()
                    .and_then(|mut addrs| addrs.next())
                    .ok_or_else(|| RuntimeError::InvalidConfig("invalid address".into()))
                    .and_then(|addr| {
                        std::net::TcpStream::connect_timeout(
                            &addr,
                            std::time::Duration::from_millis(timeout_ms),
                        )
                        .map_err(|err| {
                            RuntimeError::ControlError(format!("connect failed: {err}").into())
                        })
                    });
                let body = match result {
                    Ok(_) => json!({ "ok": true }).to_string(),
                    Err(err) => json!({ "ok": false, "error": err.to_string() }).to_string(),
                };
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/api/program" {
                let project_root = default_bundle_root(&bundle_root);
                let program_path = project_root.join("program.stbc");
                let updated_ms = program_path
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis());
                let sources = list_sources(&project_root);
                let body = json!({
                    "program": "program.stbc",
                    "updated_ms": updated_ms,
                    "sources": sources,
                })
                .to_string();
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url.starts_with("/api/source") {
                let file = url.split('?').nth(1).and_then(|query| {
                    query.split('&').find_map(|pair| {
                        let mut parts = pair.splitn(2, '=');
                        if parts.next()? == "file" {
                            Some(parts.next().unwrap_or("").to_string())
                        } else {
                            None
                        }
                    })
                });
                let Some(encoded) = file else {
                    let response = Response::from_string("missing file").with_status_code(400);
                    let _ = request.respond(response);
                    continue;
                };
                let decoded =
                    urlencoding::decode(&encoded).unwrap_or_else(|_| encoded.as_str().into());
                let project_root = default_bundle_root(&bundle_root);
                match read_source_file(&project_root, decoded.as_ref()) {
                    Ok(text) => {
                        let response = Response::from_string(text).with_header(
                            Header::from_bytes("Content-Type", "text/plain; charset=utf-8")
                                .unwrap(),
                        );
                        let _ = request.respond(response);
                    }
                    Err(err) => {
                        let response =
                            Response::from_string(format!("error: {err}")).with_status_code(404);
                        let _ = request.respond(response);
                    }
                }
                continue;
            }
            if method == Method::Get && url == "/api/pairings" {
                let list = pairing
                    .as_ref()
                    .map(|store| store.list())
                    .unwrap_or_default();
                let body = json!({ "items": list }).to_string();
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/pair/start" {
                let body = if let Some(store) = pairing.as_ref() {
                    let code = store.start_pairing();
                    json!({
                        "code": code.code,
                        "expires_at": code.expires_at,
                    })
                } else {
                    json!({ "error": "pairing unavailable" })
                };
                let response = Response::from_string(body.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/pair/claim" {
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "invalid body" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }
                let payload: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string(
                            json!({ "ok": false, "error": "invalid json" }).to_string(),
                        )
                        .with_status_code(StatusCode(400));
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let code = payload.get("code").and_then(|value| value.as_str());
                let token =
                    code.and_then(|value| pairing.as_ref().and_then(|store| store.claim(value)));
                let body = if let Some(token) = token {
                    json!({ "token": token })
                } else {
                    json!({ "error": "invalid code" })
                };
                let response = Response::from_string(body.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url == "/api/invite" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let token = auth_token
                    .lock()
                    .ok()
                    .and_then(|guard| guard.as_ref().map(|value| value.to_string()));
                let body = json!({
                    "endpoint": web_url,
                    "token": token,
                })
                .to_string();
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url.starts_with("/api/events") {
                let limit = parse_limit(&url).unwrap_or(50);
                let response = handle_request_value(
                    json!({ "id": 1, "type": "events.tail", "params": { "limit": limit } }),
                    &control_state,
                    Some("web"),
                );
                let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Get && url.starts_with("/api/faults") {
                let limit = parse_limit(&url).unwrap_or(50);
                let response = handle_request_value(
                    json!({ "id": 1, "type": "faults", "params": { "limit": limit } }),
                    &control_state,
                    Some("web"),
                );
                let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/deploy" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "invalid body" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }
                let payload: DeployRequest = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string(
                            json!({ "ok": false, "error": "invalid json" }).to_string(),
                        )
                        .with_status_code(StatusCode(400));
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let Some(bundle_root) = bundle_root.as_ref() else {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "project folder unavailable" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                };
                let result = apply_deploy(bundle_root, payload);
                let body = match result {
                    Ok(result) => {
                        if let Some(restart) = result.restart.as_ref() {
                            let _ = handle_request_value(
                                json!({ "id": 1, "type": "restart", "params": { "mode": restart } }),
                                &control_state,
                                Some("web"),
                            );
                        }
                        json!({ "ok": true, "written": result.written, "restart": result.restart })
                    }
                    Err(err) => json!({ "ok": false, "error": err.to_string() }),
                };
                let response = Response::from_string(body.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/rollback" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "invalid body" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }
                let payload: RollbackRequest =
                    serde_json::from_str(&body).unwrap_or(RollbackRequest { restart: None });
                let Some(bundle_root) = bundle_root.as_ref() else {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "project folder unavailable" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                };
                let root = bundle_root
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| bundle_root.clone());
                let result = apply_rollback(&root);
                let body = match result {
                    Ok(result) => {
                        if let Some(restart) = payload.restart.as_ref() {
                            let _ = handle_request_value(
                                json!({ "id": 1, "type": "restart", "params": { "mode": restart } }),
                                &control_state,
                                Some("web"),
                            );
                        }
                        json!({
                            "ok": true,
                            "current": result.current.display().to_string(),
                            "previous": result.previous.display().to_string(),
                        })
                    }
                    Err(err) => json!({ "ok": false, "error": err.to_string() }),
                };
                let response = Response::from_string(body.to_string())
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            if method == Method::Post && url == "/api/control" {
                if !check_auth(&request, auth, &auth_token, pairing.as_deref()) {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "unauthorized" }).to_string(),
                    )
                    .with_status_code(StatusCode(401));
                    let _ = request.respond(response);
                    continue;
                }
                let mut body = String::new();
                if request.as_reader().read_to_string(&mut body).is_err() {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "invalid body" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    continue;
                }
                let payload: serde_json::Value = match serde_json::from_str(&body) {
                    Ok(value) => value,
                    Err(_) => {
                        let response = Response::from_string(
                            json!({ "ok": false, "error": "invalid json" }).to_string(),
                        )
                        .with_status_code(StatusCode(400));
                        let _ = request.respond(response);
                        continue;
                    }
                };
                let response = handle_request_value(payload, &control_state, Some("web"));
                let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
                let response = Response::from_string(body)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                continue;
            }
            let response = Response::from_string("not found").with_status_code(StatusCode(404));
            let _ = request.respond(response);
        }
    });

    Ok(WebServer { handle, listen })
}

fn check_auth(
    request: &tiny_http::Request,
    auth_mode: WebAuthMode,
    token: &Arc<Mutex<Option<smol_str::SmolStr>>>,
    pairing: Option<&PairingStore>,
) -> bool {
    if matches!(auth_mode, WebAuthMode::Local) {
        return true;
    }
    let expected = token.lock().ok().and_then(|guard| guard.as_ref().cloned());
    let header = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Trust-Token"))
        .map(|header| header.value.as_str().to_string());
    if let Some(expected) = expected {
        if header.as_deref() == Some(expected.as_str()) {
            return true;
        }
    }
    let Some(header) = header else {
        return false;
    };
    pairing
        .as_ref()
        .map(|store| store.validate(header.as_str()))
        .unwrap_or(false)
}

fn format_web_url(listen: &str) -> String {
    let host = listen.split(':').next().unwrap_or("localhost");
    let port = listen.rsplit(':').next().unwrap_or("8080");
    let host = if host == "0.0.0.0" { "localhost" } else { host };
    format!("http://{host}:{port}")
}

fn render_qr_svg(text: &str) -> Result<String, RuntimeError> {
    let code = QrCode::new(text.as_bytes())
        .map_err(|err| RuntimeError::ControlError(format!("qr: {err}").into()))?;
    let svg = code.render::<svg::Color>().min_dimensions(120, 120).build();
    Ok(svg)
}

fn parse_limit(url: &str) -> Option<u64> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == "limit" {
            return parts.next().and_then(|value| value.parse().ok());
        }
    }
    None
}

fn decode_url_component(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.as_bytes().iter().copied();
    while let Some(byte) = chars.next() {
        match byte {
            b'%' => {
                let hi = chars.next().unwrap_or(b'0');
                let lo = chars.next().unwrap_or(b'0');
                let hex = [hi, lo];
                if let Ok(text) = std::str::from_utf8(&hex) {
                    if let Ok(value) = u8::from_str_radix(text, 16) {
                        bytes.push(value);
                    }
                }
            }
            b'+' => bytes.push(b' '),
            _ => bytes.push(byte),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn parse_probe_response(text: &str) -> Value {
    let value: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({}));
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if !ok {
        let error = value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unreachable");
        return json!({ "ok": false, "error": error });
    }
    let result = value.get("result").cloned().unwrap_or_else(|| json!({}));
    let name = result
        .get("plc_name")
        .or_else(|| result.get("resource"))
        .and_then(|v| v.as_str())
        .unwrap_or("PLC");
    let state = result
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("online");
    json!({ "ok": true, "name": name, "state": state })
}
