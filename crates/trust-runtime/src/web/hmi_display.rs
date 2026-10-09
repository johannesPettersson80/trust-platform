//! The runtime display (specification 39, §11.1): pages at `/hmi` and `/hmi/<page>`, their data
//! at `/api/hmi/page`, the live-value stream at `/ws/hmi`, and the drawing module and font.

mod data;
mod faceplates;
pub(super) mod files;
mod history;
mod stream;
pub(super) mod trends;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::auth_helpers::{auth_error_response, check_auth};
use super::pairing::PairingStore;
use super::web_helpers::query_value;
use crate::config::WebAuthMode;
use crate::control::ControlState;
use crate::security::AccessRole;
use data::{faceplate_data, page_data, popup_data};
use files::{bound_paths, errors_json, is_page_name, HmiFiles};
use stream::{accept_key, Stream, StreamSlot, MIN_PERIOD};

const SHELL_HTML: &str = include_str!("ui/hmi.html");
const NO_HMI_HTML: &str = include_str!("ui/hmi-none.html");
const DRAWING_JS: &str = include_str!("ui/modules/hmi-drawing.js");
const DISPLAY_JS: &str = include_str!("ui/modules/hmi-display.js");
const INTER_FONT: &[u8] = include_bytes!("ui/assets/fonts/InterVariable.woff2");
/// The Chinese, Japanese and Korean fonts (§14.5 item 2); a browser loads one only for a page that
/// shows text in its range.
const CJK_FONTS: [(&str, &[u8]); 3] = [
    (
        "/hmi/assets/fonts/NotoSansSC-Subset.woff2",
        include_bytes!("ui/assets/fonts/NotoSansSC-Subset.woff2"),
    ),
    (
        "/hmi/assets/fonts/NotoSansJP-Subset.woff2",
        include_bytes!("ui/assets/fonts/NotoSansJP-Subset.woff2"),
    ),
    (
        "/hmi/assets/fonts/NotoSansKR-Subset.woff2",
        include_bytes!("ui/assets/fonts/NotoSansKR-Subset.woff2"),
    ),
];
const LOGO_SVG: &str = include_str!("ui/assets/logo.svg");

/// The runtime display's drawing module and its font, for drawing pages outside the display, as
/// `trust-runtime hmi test` does (specification 39, §18 phase 5 item 6).
pub fn drawing_assets() -> (&'static str, &'static [u8]) {
    (DRAWING_JS, INTER_FONT)
}

/// The faceplates placed on a project's page, by object id, each bound to its instance as the
/// display binds it (§8.9), for drawing the page outside the display: without a running program, a
/// faceplate's only pair applies. A placement whose faceplate does not load is left out.
pub fn placed_faceplates(project: &std::path::Path, page: &str) -> Value {
    let files = HmiFiles::new(project.to_path_buf());
    let Some(Ok(model)) = files.page(page) else {
        return json!({});
    };
    let placed: serde_json::Map<String, Value> = files
        .placed_faceplates(page, &model, &|_| None)
        .into_iter()
        .filter_map(|(id, faceplate)| Some((id, json!({ "model": faceplate?.ok()? }))))
        .collect();
    Value::Object(placed)
}

pub(super) struct HmiRouteContext<'a> {
    pub(super) auth_mode: WebAuthMode,
    pub(super) auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub(super) pairing: Option<&'a PairingStore>,
    pub(super) control_state: &'a Arc<ControlState>,
    pub(super) sessions: &'a Arc<crate::hmi::access::sessions::SessionStore>,
    pub(super) commands: &'a Arc<crate::hmi::commands::registry::CommandRegistry>,
    /// The bundles; every read of the HMI files takes its lock (§16.4).
    pub(super) store: &'a Arc<crate::hmi::bundle::store::BundleStore>,
    /// Counts activations, so a stream tells its page when the bundle changes.
    pub(super) generation: &'a Arc<std::sync::atomic::AtomicU64>,
    /// The alarm list and records, which the stream sends as they change.
    pub(super) alarms: &'a Arc<super::hmi_alarms::AlarmService>,
    /// Users and the write policy: the page says which role each control needs (§5.9 item 5).
    pub(super) access: &'a Arc<crate::hmi::access::config::HmiAccessConfig>,
}

pub(super) enum HmiRouteOutcome {
    Handled,
    NotHandled(Request),
}

pub(super) fn handle_hmi_route(
    request: Request,
    method: &Method,
    url: &str,
    ctx: HmiRouteContext<'_>,
) -> HmiRouteOutcome {
    if *method != Method::Get {
        return HmiRouteOutcome::NotHandled(request);
    }
    let path = url.split('?').next().unwrap_or_default();
    match path {
        "/hmi/modules/hmi-drawing.js" => respond(
            request,
            Response::from_string(DRAWING_JS),
            "application/javascript",
            NO_STORE,
        ),
        "/hmi/modules/hmi-display.js" => respond(
            request,
            Response::from_string(DISPLAY_JS),
            "application/javascript",
            NO_STORE,
        ),
        "/hmi/assets/logo.svg" => respond(
            request,
            Response::from_string(LOGO_SVG),
            "image/svg+xml",
            "public, max-age=86400",
        ),
        "/hmi/assets/fonts/InterVariable.woff2" => respond(
            request,
            Response::from_data(INTER_FONT),
            "font/woff2",
            "public, max-age=86400",
        ),
        path if CJK_FONTS.iter().any(|(served, _)| *served == path) => {
            let font = CJK_FONTS
                .iter()
                .find_map(|(served, bytes)| (*served == path).then_some(*bytes))
                .unwrap_or_default();
            respond(
                request,
                Response::from_data(font),
                "font/woff2",
                "public, max-age=86400",
            )
        }
        "/api/hmi/page" => match authorise(&request, &ctx) {
            Ok(()) => {
                let name = query_value(url, "name");
                let language = query_value(url, "lang");
                let control_state = ctx.control_state.clone();
                let type_of =
                    move |path: &str| super::hmi_command::instance_type(&control_state, path);
                let (status, data) = ctx.store.read(|root| {
                    page_data(
                        &HmiFiles::new(root.to_path_buf()),
                        name.as_deref(),
                        language.as_deref(),
                        &ctx.access.write,
                        &type_of,
                    )
                });
                respond_json(request, status, &data)
            }
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
            }
        },
        "/api/hmi/popup" => match authorise(&request, &ctx) {
            Ok(()) => {
                // A faceplate, named by the object that opens it (§8.7), or a popup by its name.
                let (status, data) = match query_value(url, "faceplate") {
                    Some(source) => {
                        let control_state = ctx.control_state.clone();
                        let type_of = move |path: &str| {
                            super::hmi_command::instance_type(&control_state, path)
                        };
                        ctx.store.read(|root| {
                            faceplate_data(
                                &HmiFiles::new(root.to_path_buf()),
                                &source,
                                &type_of,
                                &ctx.access.write,
                            )
                        })
                    }
                    None => {
                        let name = query_value(url, "name");
                        ctx.store.read(|root| {
                            popup_data(
                                &HmiFiles::new(root.to_path_buf()),
                                name.as_deref(),
                                &ctx.access.write,
                            )
                        })
                    }
                };
                respond_json(request, status, &data)
            }
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
            }
        },
        "/api/hmi/history" => match authorise(&request, &ctx) {
            Ok(()) => {
                let (status, data) =
                    history::history(url, ctx.control_state, super::web_helpers::now_ms());
                respond_json(request, status, &data)
            }
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
            }
        },
        "/ws/hmi" => open_stream(request, url, &ctx),
        "/hmi" | "/hmi/" => shell(
            request,
            ctx.store
                .read(|root| HmiFiles::new(root.to_path_buf()).exists()),
        ),
        _ if path.starts_with(PROJECT_ASSETS) => match authorise(&request, &ctx) {
            Ok(()) => project_asset(request, &path[PROJECT_ASSETS.len()..], &ctx),
            Err(error) => {
                let _ = request.respond(auth_error_response(error));
            }
        },
        _ => match path.strip_prefix("/hmi/") {
            Some(page) if is_page_name(page) => shell(
                request,
                ctx.store
                    .read(|root| HmiFiles::new(root.to_path_buf()).exists()),
            ),
            _ => return HmiRouteOutcome::NotHandled(request),
        },
    }
    HmiRouteOutcome::Handled
}

const NO_STORE: &str = "no-store";
/// Where the display loads the project's images from: `hmi/assets/<path>` of the active bundle.
const PROJECT_ASSETS: &str = "/hmi/project-assets/";

/// One image of the active bundle's `hmi/assets/` (§5.1, §16.4). The path's segments are
/// letters, digits, `.`, `-` and `_` and never `.` or `..`, so nothing outside the assets is
/// reached; other files and missing ones answer 404.
fn project_asset(request: Request, path: &str, ctx: &HmiRouteContext<'_>) {
    let safe = !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        });
    let kind = match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("svg") => Some("image/svg+xml"),
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        Some("gif") => Some("image/gif"),
        _ => None,
    };
    let bytes = match (safe, kind) {
        (true, Some(_)) => ctx
            .store
            .read(|root| std::fs::read(root.join("hmi").join("assets").join(path)).ok()),
        _ => None,
    };
    match (bytes, kind) {
        (Some(bytes), Some(kind)) => respond(request, Response::from_data(bytes), kind, NO_STORE),
        _ => respond_json(request, 404, &json!({ "error": "no such image" })),
    }
}

fn authorise(request: &Request, ctx: &HmiRouteContext<'_>) -> Result<(), &'static str> {
    check_auth(
        request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    )
    .map(|_| ())
}

/// The page shell; without a screen file, a page saying the project has no HMI.
fn shell(request: Request, exists: bool) {
    let html = if exists { SHELL_HTML } else { NO_HMI_HTML };
    respond(
        request,
        Response::from_string(html),
        "text/html; charset=utf-8",
        NO_STORE,
    );
}

fn open_stream(request: Request, url: &str, ctx: &HmiRouteContext<'_>) {
    if let Err(error) = authorise(&request, ctx) {
        let _ = request.respond(auth_error_response(error));
        return;
    }
    // A page by its name, a popup (§4.9), or a faceplate by the object that opens it (§8.7).
    let faceplate = query_value(url, "faceplate")
        .map(|source| format!("{}{source}", faceplates::FACEPLATE_PREFIX));
    let address = match (
        query_value(url, "page"),
        query_value(url, "popup"),
        faceplate,
    ) {
        (Some(name), _, _) if is_page_name(&name) => name,
        (None, Some(name), _) if is_page_name(&name) => format!("popup:{name}"),
        (None, None, Some(address)) if faceplates::is_faceplate_address(&address) => address,
        _ => {
            return respond_json(
                request,
                400,
                &json!({ "error": "name a page, a popup or a faceplate: /ws/hmi?page=<page>, ?popup=<popup> or ?faceplate=<page>/<object>" }),
            )
        }
    };
    let name = address.clone();
    let control_state = ctx.control_state.clone();
    let type_of = move |path: &str| super::hmi_command::instance_type(&control_state, path);
    // A page's stream also carries the values of the faceplates placed on it (§8.9).
    let page = ctx.store.read(|root| {
        let files = HmiFiles::new(root.to_path_buf());
        let page = files
            .exists()
            .then(|| files.page_or_popup_with(&address, &type_of))
            .flatten()?;
        let placed = match &page {
            Ok(page) if !faceplates::is_faceplate_address(&address) => files
                .placed_faceplates(&address, page, &type_of)
                .into_iter()
                .filter_map(|(_, faceplate)| faceplate?.ok())
                .collect(),
            _ => Vec::new(),
        };
        Some(page.map(|page| (page, placed)))
    });
    let page = match page {
        None => {
            return respond_json(
                request,
                404,
                &json!({ "error": format!("the project has no page `{name}`") }),
            )
        }
        Some(Err(errors)) => {
            return respond_json(request, 422, &json!({ "errors": errors_json(&errors) }))
        }
        Some(Ok(page)) => page,
    };
    let (page, placed) = page;
    let mut paths = bound_paths(&page);
    for faceplate in &placed {
        for path in bound_paths(&faceplate.popup.page) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    let accept = match accept_key(&request) {
        Ok(key) => key,
        Err(error) => return respond_json(request, 400, &json!({ "error": error })),
    };
    let Some(slot) = StreamSlot::take() else {
        return respond_json(
            request,
            503,
            &json!({ "error": "too many value streams are open" }),
        );
    };
    let scan = ctx
        .control_state
        .settings
        .lock()
        .map(|settings| settings.cycle_interval.as_millis())
        .unwrap_or_default();
    let period = MIN_PERIOD.max(std::time::Duration::from_millis(
        u64::try_from(scan).unwrap_or(0),
    ));
    let response = Response::empty(StatusCode(101)).with_header(
        Header::from_bytes("Sec-WebSocket-Accept", accept.as_bytes()).expect("an ASCII header"),
    );
    let token = super::hmi_operate::session_token(&request);
    let socket = request.upgrade("websocket", response);
    Stream {
        resource: ctx.control_state.resource.clone(),
        period,
        paths,
        slot,
        sessions: ctx.sessions.clone(),
        commands: ctx.commands.clone(),
        store: ctx.store.clone(),
        generation: ctx.generation.clone(),
        alarms: ctx.alarms.clone(),
        control: ctx.control_state.clone(),
        token,
    }
    .spawn(socket);
}

fn respond<R: std::io::Read>(request: Request, response: Response<R>, kind: &str, cache: &str) {
    let response = response
        .with_header(Header::from_bytes("Content-Type", kind).expect("an ASCII header"))
        .with_header(Header::from_bytes("Cache-Control", cache).expect("an ASCII header"));
    let _ = request.respond(response);
}

fn respond_json(request: Request, status: u16, data: &Value) {
    let response = Response::from_string(data.to_string()).with_status_code(StatusCode(status));
    respond(request, response, "application/json", NO_STORE);
}
