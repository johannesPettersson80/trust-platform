//! UI/web-asset route handlers used by the web server loop.

#![allow(missing_docs)]

use super::*;

pub(super) enum UiRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn handle_ui_route(
    request: tiny_http::Request,
    method: &Method,
    url: &str,
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
            .with_header(Header::from_bytes("Location", "/setup/ads").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && (url == "/setup/ads" || url == "/setup/ads/") {
        let response = Response::from_string(ADS_SETUP_HTML)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/setup/ads.css" {
        let response = Response::from_string(ADS_SETUP_CSS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
        let _ = request.respond(response);
        return UiRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/setup/ads.js" {
        let response = Response::from_string(ADS_SETUP_JS)
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
        let _ = request.respond(response);
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
    UiRouteOutcome::NotHandled(request)
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
