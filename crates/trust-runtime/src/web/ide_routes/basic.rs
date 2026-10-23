use super::*;

pub(super) fn handle_basic_route(
    mut request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: &IdeRouteContext<'_>,
) -> IdeRouteOutcome {
    let auth = ctx.auth_mode;
    let auth_token = ctx.auth_token;
    let pairing = ctx.pairing;
    let bundle_root = ctx.bundle_root;
    let ide_state = ctx.ide_state;

    if *method == Method::Get && url == "/api/program" {
        let project_root = default_bundle_root(bundle_root);
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
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/source") {
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
            return IdeRouteOutcome::Handled;
        };
        let decoded = urlencoding::decode(&encoded).unwrap_or_else(|_| encoded.as_str().into());
        let project_root = default_bundle_root(bundle_root);
        match read_source_file(&project_root, decoded.as_ref()) {
            Ok(text) => {
                let response = Response::from_string(text).with_header(
                    Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap(),
                );
                let _ = request.respond(response);
            }
            Err(err) => {
                let response = Response::from_string(format!("error: {err}")).with_status_code(404);
                let _ = request.respond(response);
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/ide/capabilities" {
        let (web_role, _request_token) =
            match check_auth_with_role(&request, auth, auth_token, pairing, AccessRole::Viewer) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return IdeRouteOutcome::Handled;
                }
            };
        let caps = ide_state.capabilities(web_role.allows(AccessRole::Engineer));
        let response = Response::from_string(json!({ "ok": true, "result": caps }).to_string())
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
        let _ = request.respond(response);
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/ide/session" {
        let (web_role, _request_token) =
            match check_auth_with_role(&request, auth, auth_token, pairing, AccessRole::Viewer) {
                Ok(token) => token,
                Err(error) => {
                    let _ = request.respond(auth_error_response(error));
                    return IdeRouteOutcome::Handled;
                }
            };
        let mut body = String::new();
        if request.as_reader().read_to_string(&mut body).is_err() {
            let response =
                Response::from_string(json!({ "ok": false, "error": "invalid body" }).to_string())
                    .with_status_code(StatusCode(400));
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        }
        let payload = if body.trim().is_empty() {
            IdeSessionRequest { role: None }
        } else {
            match serde_json::from_str::<IdeSessionRequest>(&body) {
                Ok(value) => value,
                Err(_) => {
                    let response = Response::from_string(
                        json!({ "ok": false, "error": "invalid json" }).to_string(),
                    )
                    .with_status_code(StatusCode(400));
                    let _ = request.respond(response);
                    return IdeRouteOutcome::Handled;
                }
            }
        };
        let role = payload
            .role
            .as_deref()
            .and_then(IdeRole::parse)
            .unwrap_or(IdeRole::Viewer);
        if matches!(role, IdeRole::Editor) && !web_role.allows(AccessRole::Engineer) {
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "error": "editor session requires engineer/admin web role"
                })
                .to_string(),
            )
            .with_status_code(StatusCode(403))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        }
        match ide_state.create_session(role) {
            Ok(session) => {
                let response =
                    Response::from_string(json!({ "ok": true, "result": session }).to_string())
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
            }
            Err(error) => {
                let _ = request.respond(ide_error_response(error));
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/ide/project" {
        let Some(session_token) = ide_session_token(&request) else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "missing X-Trust-Ide-Session" }).to_string(),
            )
            .with_status_code(StatusCode(401));
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        };
        match ide_state.project_selection(session_token.as_str()) {
            Ok(selection) => {
                let response =
                    Response::from_string(json!({ "ok": true, "result": selection }).to_string())
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
            }
            Err(error) => {
                let _ = request.respond(ide_error_response(error));
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/ide/io/config" {
        let Some(session_token) = ide_session_token(&request) else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "missing X-Trust-Ide-Session" }).to_string(),
            )
            .with_status_code(StatusCode(401));
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        };
        if let Err(error) = ide_state.project_selection(session_token.as_str()) {
            let _ = request.respond(ide_error_response(error));
            return IdeRouteOutcome::Handled;
        }
        let active_root = ide_state.active_project_root();
        let target_root = if active_root.is_some() {
            active_root
        } else {
            bundle_root.clone()
        };
        match load_io_config(&target_root) {
            Ok(config) => {
                let response = Response::from_string(
                    json!({ "ok": true, "result": redacted_io_config(&config) }).to_string(),
                )
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
            Err(error) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": error.to_string() }).to_string(),
                )
                .with_status_code(StatusCode(400))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/ide/io/config" {
        let Some(session_token) = ide_session_token(&request) else {
            let response = Response::from_string(
                json!({ "ok": false, "error": "missing X-Trust-Ide-Session" }).to_string(),
            )
            .with_status_code(StatusCode(401));
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        };
        if let Err(error) = ide_state.require_editor_session(session_token.as_str()) {
            let _ = request.respond(ide_error_response(error));
            return IdeRouteOutcome::Handled;
        }
        if !ide_write_enabled(ctx.control_state) {
            let response = Response::from_string(
                json!({
                    "ok": false,
                    "error": "web IDE authoring is disabled in current runtime mode"
                })
                .to_string(),
            )
            .with_status_code(StatusCode(403))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            return IdeRouteOutcome::Handled;
        }
        let payload: IoConfigRequest = match read_json_body(&mut request, MAX_JSON_REQUEST_BYTES) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(json_body_error_response(error));
                return IdeRouteOutcome::Handled;
            }
        };
        let active_root = ide_state.active_project_root();
        let target_root = if active_root.is_some() {
            active_root
        } else {
            bundle_root.clone()
        };
        match save_io_config(&target_root, &payload) {
            Ok(message) => {
                let response =
                    Response::from_string(json!({ "ok": true, "result": message }).to_string())
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
            }
            Err(error) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": error.to_string() }).to_string(),
                )
                .with_status_code(StatusCode(400))
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
                let _ = request.respond(response);
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/ide/project/create" {
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
        let payload: IdeProjectCreateRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "invalid json" }).to_string(),
                )
                .with_status_code(StatusCode(400));
                let _ = request.respond(response);
                return IdeRouteOutcome::Handled;
            }
        };
        let template = payload.template.as_deref().unwrap_or("empty");
        match ide_state.create_project(
            session_token.as_str(),
            payload.name.as_str(),
            payload.location.as_str(),
            template,
        ) {
            Ok(selection) => {
                let response =
                    Response::from_string(json!({ "ok": true, "result": selection }).to_string())
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
            }
            Err(error) => {
                let _ = request.respond(ide_error_response(error));
            }
        }
        return IdeRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/ide/project/open" {
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
        let payload: IdeProjectOpenRequest = match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(_) => {
                let response = Response::from_string(
                    json!({ "ok": false, "error": "invalid json" }).to_string(),
                )
                .with_status_code(StatusCode(400));
                let _ = request.respond(response);
                return IdeRouteOutcome::Handled;
            }
        };
        match ide_state.set_active_project(session_token.as_str(), payload.path.as_str()) {
            Ok(selection) => {
                let response =
                    Response::from_string(json!({ "ok": true, "result": selection }).to_string())
                        .with_header(
                            Header::from_bytes("Content-Type", "application/json").unwrap(),
                        );
                let _ = request.respond(response);
            }
            Err(error) => {
                let _ = request.respond(ide_error_response(error));
            }
        }
        return IdeRouteOutcome::Handled;
    }

    IdeRouteOutcome::NotHandled(request)
}
