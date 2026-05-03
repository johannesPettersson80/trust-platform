//! UI/web-asset route handlers used by the web server loop.

#![allow(missing_docs)]

use super::*;

pub(super) struct UiRouteContext<'a> {
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub control_state: &'a Arc<ControlState>,
    pub hmi_asset_root: Option<PathBuf>,
}

pub(super) enum UiRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_ui_route(
    request: tiny_http::Request,
    method: &Method,
    url: &str,
    url_path: &str,
    ctx: UiRouteContext<'_>,
) -> UiRouteOutcome {
    if *method == Method::Get && url == "/" {
        let response = Response::from_string("")
            .with_status_code(StatusCode(302))
            .with_header(Header::from_bytes("Location", "/ide").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && (url == "/setup" || url == "/setup/") {
        let response = Response::from_string("")
            .with_status_code(StatusCode(302))
            .with_header(Header::from_bytes("Location", "/ide").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && (url == "/hmi" || url == "/hmi/") {
        let response = Response::from_string(HMI_HTML)
            .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url_path.starts_with("/hmi/assets/") {
        let Some(project_root) = ctx.hmi_asset_root else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "project root unavailable" }).to_string(),
            )
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return UiRouteOutcome::Handled;
        };
        let encoded = url_path.trim_start_matches("/hmi/assets/");
        if encoded.is_empty() {
            let response = Response::from_string(
                json!({ "ok": false, "error": "missing asset path" }).to_string(),
            )
            .with_status_code(StatusCode(400))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return UiRouteOutcome::Handled;
        }
        let asset = decode_url_component(encoded);
        match read_hmi_asset_file(&project_root, asset.as_str()) {
            Ok(svg) => {
                let response = Response::from_string(svg)
                    .with_header(Header::from_bytes("Content-Type", "image/svg+xml").unwrap());
                let _ = request.respond(response);
            }
            Err(err) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": err.to_string() }).to_string(),
                )
                .with_status_code(StatusCode(404))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
        }
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url_path == HMI_WS_ROUTE {
        let request_token = match check_auth(
            &request,
            ctx.auth_mode,
            ctx.auth_token,
            ctx.pairing,
            AccessRole::Viewer,
        ) {
            Ok(token) => token,
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
                return UiRouteOutcome::Handled;
            }
        };
        let accept_key = match websocket_accept_key(&request) {
            Ok(key) => key,
            Err(error) => {
                let response =
                    Response::from_string(json!({ "ok": false, "error": error }).to_string())
                        .with_status_code(StatusCode(400))
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
                return UiRouteOutcome::Handled;
            }
        };
        let response = Response::empty(StatusCode(101)).with_header(
            Header::from_bytes("Sec-WebSocket-Accept", accept_key.as_bytes()).unwrap(),
        );
        let stream = request.upgrade("websocket", response);
        spawn_hmi_websocket_session(stream, ctx.control_state.clone(), request_token);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get
        && (url == "/ide"
            || url == "/ide/"
            || url == "/ide/code"
            || url == "/ide/code/"
            || url == "/ide/hardware"
            || url == "/ide/hardware/"
            || url == "/ide/settings"
            || url == "/ide/settings/"
            || url == "/ide/logs"
            || url == "/ide/logs/")
    {
        let response = Response::from_string(IDE_HTML)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/base.css" {
        let response = Response::from_string(BASE_CSS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/ide.css" {
        let response = Response::from_string(IDE_CSS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/ide.js" {
        let response = Response::from_string(IDE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-tabs.js" {
        let response = Response::from_string(IDE_TABS_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-shell.js" {
        let response = Response::from_string(IDE_SHELL_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-editor.js" {
        let response = Response::from_string(IDE_EDITOR_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-editor-language.js" {
        let response = Response::from_string(IDE_EDITOR_LANGUAGE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-editor-runtime.js" {
        let response = Response::from_string(IDE_EDITOR_RUNTIME_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-editor-pane.js" {
        let response = Response::from_string(IDE_EDITOR_PANE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-workspace.js" {
        let response = Response::from_string(IDE_WORKSPACE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-workspace-tree.js" {
        let response = Response::from_string(IDE_WORKSPACE_TREE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-workspace-files.js" {
        let response = Response::from_string(IDE_WORKSPACE_FILES_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-workspace-browse.js" {
        let response = Response::from_string(IDE_WORKSPACE_BROWSE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-observability.js" {
        let response = Response::from_string(IDE_OBSERVABILITY_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-commands.js" {
        let response = Response::from_string(IDE_COMMANDS_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-hardware.js" {
        let response = Response::from_string(IDE_HARDWARE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/cytoscape.min.js" {
        let response = Response::from_string(CYTOSCAPE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-online.js" {
        let response = Response::from_string(IDE_ONLINE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-debug.js" {
        let response = Response::from_string(IDE_DEBUG_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-settings.js" {
        let response = Response::from_string(IDE_SETTINGS_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/modules/ide-logs.js" {
        let response = Response::from_string(IDE_LOGS_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/assets/ide-monaco.20260215.js" {
        let response = Response::from_string(IDE_MONACO_BUNDLE_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/assets/ide-monaco.20260215.css" {
        let response = Response::from_string(IDE_MONACO_BUNDLE_CSS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/assets/logo.svg" {
        let response = Response::from_string(IDE_LOGO_SVG)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "image/svg+xml").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/wasm/worker.js" {
        let response = Response::from_string(IDE_WASM_WORKER_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/wasm/analysis-client.js" {
        let response = Response::from_string(IDE_WASM_CLIENT_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/wasm/trust_wasm_analysis.js" {
        let js_path = wasm_pkg_dir().join("trust_wasm_analysis.js");
        match std::fs::read_to_string(&js_path) {
            Ok(js_content) => {
                let response = Response::from_string(js_content)
                    .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
                    .with_header(
                        Header::from_bytes("Content-Type", "application/javascript").unwrap(),
                    );
                let _ = request.respond(response);
            }
            Err(_) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "WASM JS glue not found. Run scripts/build_browser_analysis_wasm_spike.sh" }).to_string(),
                )
                .with_status_code(StatusCode(404))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
        }
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/ide/wasm/trust_wasm_analysis_bg.wasm" {
        let wasm_path = wasm_pkg_dir().join("trust_wasm_analysis_bg.wasm");
        match std::fs::read(&wasm_path) {
            Ok(wasm_bytes) => {
                let cursor = std::io::Cursor::new(wasm_bytes);
                let response = Response::new(
                    StatusCode(200),
                    vec![
                        Header::from_bytes("Cache-Control", "no-store").unwrap(),
                        Header::from_bytes("Content-Type", "application/wasm").unwrap(),
                    ],
                    cursor,
                    None,
                    None,
                );
                let _ = request.respond(response);
            }
            Err(_) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "WASM binary not found. Run scripts/build_browser_analysis_wasm_spike.sh" }).to_string(),
                )
                .with_status_code(StatusCode(404))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
        }
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/hmi/export.json" {
        let schema_response = dispatch_control_request(
            json!({
                "id": 1_u64,
                "type": "hmi.schema.get"
            }),
            ctx.control_state,
            None,
            None,
        );
        let schema_payload = match serde_json::to_value(schema_response) {
            Ok(value) => value,
            Err(err) => {
                let response = Response::from_string(
                    json!({
                        "error": format!("schema response serialization failed: {err}")
                    })
                    .to_string(),
                )
                .with_status_code(503)
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
                return UiRouteOutcome::Handled;
            }
        };
        let ok = schema_payload
            .get("ok")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !ok {
            let response =
                Response::from_string(json!({ "error": "schema unavailable" }).to_string())
                    .with_status_code(503)
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return UiRouteOutcome::Handled;
        }
        let schema = schema_payload
            .get("result")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let exported_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let descriptor = ctx
            .control_state
            .hmi_descriptor
            .lock()
            .ok()
            .and_then(|state| state.customization.dir_descriptor().cloned());
        let payload = json!({
            "version": 2_u32,
            "exported_at_ms": exported_at_ms,
            "entrypoint": "hmi/index.html",
            "routes": [
                "/hmi",
                "/hmi/app.js",
                "/hmi/styles.css",
                "/hmi/modules/hmi-model-descriptor.js",
                "/hmi/modules/hmi-model-layout.js",
                "/hmi/modules/hmi-model-navigation.js",
                "/hmi/modules/hmi-model.js",
                "/hmi/modules/hmi-renderers.js",
                "/hmi/modules/hmi-widgets.js",
                "/hmi/modules/hmi-trends-alarms.js",
                "/hmi/modules/hmi-process-view.js",
                "/hmi/modules/hmi-transport.js",
                "/hmi/modules/hmi-pages.js",
                "/api/control",
                HMI_WS_ROUTE
            ],
            "config": {
                "poll_ms": 500_u32,
                "ws_route": HMI_WS_ROUTE,
                "schema": schema,
                "descriptor": descriptor
            },
            "assets": hmi_export_assets()
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
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/hmi/styles.css" {
        let response = Response::from_string(HMI_CSS)
            .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/hmi/app.js" {
        let response = Response::from_string(HMI_JS)
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url_path.starts_with("/hmi/modules/") {
        if let Some(module_name) = url_path.strip_prefix("/hmi/modules/") {
            if let Some(source) = resolve_ui_module_source(module_name) {
                let response = Response::from_string(source).with_header(
                    Header::from_bytes("Content-Type", "application/javascript").unwrap(),
                );
                let _ = request.respond(response);
                return UiRouteOutcome::Handled;
            }
        }
    }

    UiRouteOutcome::NotHandled(request)
}

const UI_MODULES_FS_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/web/ui/modules");
const HMI_MODULE_FILES: [&str; 10] = [
    "hmi-model-descriptor.js",
    "hmi-model-layout.js",
    "hmi-model-navigation.js",
    "hmi-model.js",
    "hmi-renderers.js",
    "hmi-widgets.js",
    "hmi-trends-alarms.js",
    "hmi-process-view.js",
    "hmi-transport.js",
    "hmi-pages.js",
];

fn hmi_export_assets() -> serde_json::Value {
    let mut assets = serde_json::Map::new();
    assets.insert("hmi/index.html".to_string(), json!(HMI_HTML));
    assets.insert("hmi/styles.css".to_string(), json!(HMI_CSS));
    assets.insert("hmi/app.js".to_string(), json!(HMI_JS));
    for module_name in HMI_MODULE_FILES {
        let source = resolve_hmi_module_source(module_name);
        assets.insert(format!("hmi/modules/{module_name}"), json!(source));
    }
    serde_json::Value::Object(assets)
}

fn resolve_hmi_module_source(module_name: &str) -> String {
    resolve_ui_module_source(module_name)
        .or_else(|| fallback_hmi_module_source(module_name).map(str::to_string))
        .unwrap_or_default()
}

fn fallback_hmi_module_source(module_name: &str) -> Option<&'static str> {
    match module_name {
        "hmi-model-descriptor.js" => Some(HMI_MODEL_DESCRIPTOR_JS),
        "hmi-model-layout.js" => Some(HMI_MODEL_LAYOUT_JS),
        "hmi-model-navigation.js" => Some(HMI_MODEL_NAVIGATION_JS),
        "hmi-model.js" => Some(HMI_MODEL_JS),
        "hmi-renderers.js" => Some(HMI_RENDERERS_JS),
        "hmi-widgets.js" => Some(HMI_WIDGETS_JS),
        "hmi-trends-alarms.js" => Some(HMI_TRENDS_ALARMS_JS),
        "hmi-process-view.js" => Some(HMI_PROCESS_VIEW_JS),
        "hmi-transport.js" => Some(HMI_TRANSPORT_JS),
        "hmi-pages.js" => Some(HMI_PAGES_JS),
        _ => None,
    }
}

fn resolve_ui_module_source(module_name: &str) -> Option<String> {
    if module_name.is_empty() || !module_name.ends_with(".js") {
        return None;
    }
    if module_name.contains('/') || module_name.contains('\\') || module_name.contains("..") {
        return None;
    }

    let modules_root = Path::new(UI_MODULES_FS_ROOT);
    let module_path = modules_root.join(module_name);
    let mut source = std::fs::read_to_string(module_path).ok()?;
    if module_name.contains("-part-") {
        return Some(source);
    }

    let stem = module_name.strip_suffix(".js")?;
    let prefix = format!("{stem}-part-");
    let Ok(entries) = std::fs::read_dir(modules_root) else {
        return Some(source);
    };

    let mut parts: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with(&prefix) || !file_name.ends_with(".js") {
                return None;
            }
            std::fs::read_to_string(entry.path())
                .ok()
                .map(|content| (file_name, content))
        })
        .collect();

    parts.sort_by(|(left_name, _), (right_name, _)| {
        module_part_sort_key(stem, left_name)
            .cmp(&module_part_sort_key(stem, right_name))
            .then_with(|| left_name.cmp(right_name))
    });

    for (_, part_source) in parts {
        source.push('\n');
        source.push_str(&part_source);
    }
    Some(source)
}

fn module_part_sort_key(stem: &str, file_name: &str) -> Vec<u32> {
    let trimmed = file_name.strip_suffix(".js").unwrap_or(file_name);
    let suffix = trimmed.strip_prefix(stem).unwrap_or(trimmed);
    let mut key = Vec::new();
    for segment in suffix.split("-part-").skip(1) {
        let digits: String = segment
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect();
        key.push(digits.parse::<u32>().unwrap_or(u32::MAX));
    }
    if key.is_empty() {
        key.push(u32::MAX);
    }
    key
}

fn wasm_pkg_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|p| p.join("../../target/browser-analysis-wasm/pkg"))
        .or_else(|| {
            std::env::var("CARGO_MANIFEST_DIR")
                .ok()
                .map(|d| PathBuf::from(d).join("../../target/browser-analysis-wasm/pkg"))
        })
        .unwrap_or_else(|| PathBuf::from("target/browser-analysis-wasm/pkg"))
}
