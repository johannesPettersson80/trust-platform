use serde::Deserialize;

use super::*;

/// The settings panel's request to drop one key from a runtime.toml text.
#[derive(Debug, Deserialize)]
struct RuntimeTomlKeyRemovalRequest {
    text: String,
    section: String,
    key: String,
}

/// `POST /api/ide/runtime-toml/remove-key` returns the runtime.toml text without one key, so the
/// settings panel returns an optional setting, such as an empty Control Endpoint, to its default
/// instead of writing a value (spec 11, section 6.8.0a). The text is edited in Rust and nothing is
/// written; the panel saves it through its usual path.
pub(super) fn handle_settings_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &IdeRouteContext<'_>,
) -> IdeRouteOutcome {
    if !(*method == Method::Post && url == "/api/ide/runtime-toml/remove-key") {
        return IdeRouteOutcome::NotHandled(request);
    }
    let Some(session_token) = ide_session_token(&request) else {
        let response = Response::from_string(
            json!({ "ok": false, "error": "missing X-Trust-Ide-Session" }).to_string(),
        )
        .with_status_code(StatusCode(401));
        let _ = request.respond(response);
        return IdeRouteOutcome::Handled;
    };
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        let response =
            Response::from_string(json!({ "ok": false, "error": "invalid body" }).to_string())
                .with_status_code(StatusCode(400));
        let _ = request.respond(response);
        return IdeRouteOutcome::Handled;
    }
    let payload: RuntimeTomlKeyRemovalRequest = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) => {
            let response =
                Response::from_string(json!({ "ok": false, "error": "invalid json" }).to_string())
                    .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        }
    };
    match ctx.ide_state.remove_runtime_toml_key(
        session_token.as_str(),
        payload.text.as_str(),
        payload.section.as_str(),
        payload.key.as_str(),
    ) {
        Ok(text) => {
            let response = Response::from_string(
                json!({ "ok": true, "result": { "text": text } }).to_string(),
            )
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
        }
        Err(error) => {
            let _ = request.respond(ide_error_response(error));
        }
    }
    IdeRouteOutcome::Handled
}
