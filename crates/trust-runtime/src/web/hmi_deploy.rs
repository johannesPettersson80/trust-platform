//! Deploying HMI bundles (specification 39, §16.2, §16.4). `POST /api/hmi/deploy` checks a bundle
//! against the running program and the runtime write policy and activates it in one step.
//! `POST /api/hmi/rollback` makes the previous bundle active. Both need the deploy permission
//! (the runtime's administrator access or an Engineer's session) and go to the audit log.

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::auth_helpers::check_auth_with_role;
use super::hmi_operate::session_token;
use super::http_post_policy::hmi_write_route;
use super::pairing::PairingStore;
use crate::config::WebAuthMode;
use crate::control::{ControlAuditEvent, ControlState};
use crate::hmi::access::config::HmiAccessConfig;
use crate::hmi::access::sessions::SessionStore;
use crate::hmi::bundle::checks::{check_bundle_with_remote, Program};
use crate::hmi::bundle::store::{BundleStore, Versions};
use crate::hmi::bundle::{HmiBundle, MAX_BYTES};
use crate::hmi::catalogue::Catalogue;
use crate::scheduler::ResourceCommand;
use crate::security::AccessRole;

/// How long the running program may take to give a snapshot for the checks.
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);

static AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct HmiDeployContext<'a> {
    pub(super) auth_mode: WebAuthMode,
    pub(super) auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub(super) pairing: Option<&'a PairingStore>,
    pub(super) sessions: &'a Arc<SessionStore>,
    pub(super) store: &'a Arc<BundleStore>,
    pub(super) access: &'a Arc<HmiAccessConfig>,
    pub(super) control_state: &'a Arc<ControlState>,
    /// Counts activations, so the value streams tell old pages to reload.
    pub(super) generation: &'a Arc<AtomicU64>,
}

pub(super) enum HmiDeployOutcome {
    Handled,
    NotHandled(Request),
}

pub(super) fn handle_hmi_deploy_route(
    request: Request,
    method: &Method,
    url: &str,
    ctx: HmiDeployContext<'_>,
) -> HmiDeployOutcome {
    let path = url.split('?').next().unwrap_or_default();
    // A cross-site request is answered 403 before the permission check, so it is not even
    // audited (§21.5 item 3).
    match (method, path) {
        (Method::Post, "/api/hmi/deploy") => {
            hmi_write_route(request, |request| deploy(request, &ctx))
        }
        (Method::Post, "/api/hmi/rollback") => {
            hmi_write_route(request, |request| rollback(request, &ctx))
        }
        _ => return HmiDeployOutcome::NotHandled(request),
    }
    HmiDeployOutcome::Handled
}

/// Who may deploy: the administrator, or a user whose session has the Engineer role.
enum Permission {
    Granted(String),
    Refused {
        status: u16,
        who: String,
        reason: &'static str,
    },
}

fn permission(request: &Request, ctx: &HmiDeployContext<'_>) -> Permission {
    let role = match check_auth_with_role(
        request,
        ctx.auth_mode,
        ctx.auth_token,
        ctx.pairing,
        AccessRole::Viewer,
    ) {
        Ok((role, _)) => role,
        Err(error) => {
            return Permission::Refused {
                status: if error == "forbidden" { 403 } else { 401 },
                who: "unknown".to_string(),
                reason: error,
            }
        }
    };
    if role.allows(AccessRole::Admin) {
        return Permission::Granted("administrator".to_string());
    }
    match session_token(request).and_then(|token| ctx.sessions.touch(&token)) {
        Some(session) if session.role.allows(AccessRole::Engineer) => Permission::Granted(session.user.to_string()),
        session => Permission::Refused {
            status: 403,
            who: session.map_or_else(|| "a viewer".to_string(), |session| session.user.to_string()),
            reason: "deploying needs the deploy permission: an Engineer's session or the runtime's administrator access",
        },
    }
}

fn deploy(mut request: Request, ctx: &HmiDeployContext<'_>) {
    let who = match permission(&request, ctx) {
        Permission::Granted(who) => who,
        Permission::Refused {
            status,
            who,
            reason,
        } => {
            audit(ctx, "hmi.deploy", &who, None, "refused", Some(reason));
            return respond(request, status, &json!({ "ok": false, "error": reason }));
        }
    };
    let mut body = String::new();
    if request
        .as_reader()
        .take((MAX_BYTES * 2) as u64)
        .read_to_string(&mut body)
        .is_err()
    {
        return respond(
            request,
            400,
            &json!({ "ok": false, "error": "the bundle cannot be read" }),
        );
    }
    let bundle = match serde_json::from_str::<Value>(&body)
        .map_err(|error| error.to_string())
        .and_then(|value| HmiBundle::from_json(&value))
    {
        Ok(bundle) => bundle,
        Err(error) => {
            audit(ctx, "hmi.deploy", &who, None, "refused", Some(&error));
            return respond(request, 400, &json!({ "ok": false, "error": error }));
        }
    };
    let version = bundle.short_version();
    let Some(storage) = snapshot(ctx) else {
        audit(
            ctx,
            "hmi.deploy",
            &who,
            Some(&version),
            "refused",
            Some("the running program cannot be read"),
        );
        return respond(
            request,
            503,
            &json!({ "ok": false, "error": "the running program cannot be read" }),
        );
    };
    let metadata = ctx
        .control_state
        .metadata
        .lock()
        .map(|metadata| metadata.clone())
        .ok();
    let Some(metadata) = metadata else {
        return respond(
            request,
            503,
            &json!({ "ok": false, "error": "the running program cannot be read" }),
        );
    };
    let problems = check_bundle_with_remote(
        &bundle,
        Catalogue::shipped(),
        &Program {
            metadata: &metadata,
            storage: &storage,
        },
        &ctx.access.write,
        &ctx.control_state
            .resource
            .hmi_exchange()
            .remote_values
            .catalogue(),
    );
    if !problems.is_empty() {
        let listed: Vec<Value> = problems.iter().map(|problem| json!({ "file": problem.file, "line": problem.line, "message": problem.message })).collect();
        audit(
            ctx,
            "hmi.deploy",
            &who,
            Some(&version),
            "refused",
            Some(&format!("{} problems", listed.len())),
        );
        let versions = ctx.store.versions();
        return respond(
            request,
            422,
            &json!({ "ok": false, "problems": listed, "active": short(&versions.active), "previous": short(&versions.previous) }),
        );
    }
    match ctx.store.activate_with_invalidation(&bundle, || {
        ctx.control_state
            .resource
            .hmi_exchange()
            .methods
            .invalidate()
    }) {
        Ok(versions) => {
            ctx.generation.fetch_add(1, Ordering::AcqRel);
            super::hmi_display::trends::share_trended_paths(
                ctx.store,
                &crate::control::hmi_ports::hmi_exchange(ctx.control_state),
            );
            audit_versions(ctx, "hmi.deploy", &who, &versions);
            respond(
                request,
                200,
                &json!({ "ok": true, "active": short(&versions.active), "previous": short(&versions.previous) }),
            );
        }
        Err(error) => {
            let reason = format!("the bundle could not be activated: {error}");
            audit(
                ctx,
                "hmi.deploy",
                &who,
                Some(&version),
                "failed",
                Some(&reason),
            );
            respond(request, 500, &json!({ "ok": false, "error": reason }));
        }
    }
}

fn rollback(request: Request, ctx: &HmiDeployContext<'_>) {
    let who = match permission(&request, ctx) {
        Permission::Granted(who) => who,
        Permission::Refused {
            status,
            who,
            reason,
        } => {
            audit(ctx, "hmi.rollback", &who, None, "refused", Some(reason));
            return respond(request, status, &json!({ "ok": false, "error": reason }));
        }
    };
    match ctx.store.rollback_with_invalidation(|| {
        ctx.control_state
            .resource
            .hmi_exchange()
            .methods
            .invalidate()
    }) {
        Ok(versions) => {
            ctx.generation.fetch_add(1, Ordering::AcqRel);
            super::hmi_display::trends::share_trended_paths(
                ctx.store,
                &crate::control::hmi_ports::hmi_exchange(ctx.control_state),
            );
            audit_versions(ctx, "hmi.rollback", &who, &versions);
            respond(
                request,
                200,
                &json!({ "ok": true, "active": short(&versions.active), "previous": short(&versions.previous) }),
            );
        }
        Err(error) => {
            audit(ctx, "hmi.rollback", &who, None, "refused", Some(&error));
            respond(request, 409, &json!({ "ok": false, "error": error }));
        }
    }
}

/// The running program's variables, for the checks.
fn snapshot(ctx: &HmiDeployContext<'_>) -> Option<crate::memory::VariableStorage> {
    let (respond_to, reply) = mpsc::channel();
    ctx.control_state
        .resource
        .send_command(ResourceCommand::Snapshot { respond_to })
        .ok()?;
    reply
        .recv_timeout(SNAPSHOT_TIMEOUT)
        .ok()
        .map(|snapshot| snapshot.storage)
}

fn short(version: &Option<String>) -> Value {
    version.as_ref().map_or(Value::Null, |version| {
        json!(version[..16.min(version.len())])
    })
}

fn audit_versions(ctx: &HmiDeployContext<'_>, kind: &str, who: &str, versions: &Versions) {
    let details = json!({ "previous": short(&versions.previous) });
    send_audit(
        ctx,
        kind,
        who,
        short(&versions.active),
        "activated",
        None,
        details,
    );
}

fn audit(
    ctx: &HmiDeployContext<'_>,
    kind: &str,
    who: &str,
    version: Option<&str>,
    outcome: &str,
    reason: Option<&str>,
) {
    let versions = ctx.store.versions();
    let details = json!({ "active": short(&versions.active) });
    send_audit(
        ctx,
        kind,
        who,
        version.map_or(Value::Null, |version| json!(version)),
        outcome,
        reason,
        details,
    );
}

/// An audit record (§16.2.7): who, the versions and the outcome.
fn send_audit(
    ctx: &HmiDeployContext<'_>,
    kind: &str,
    who: &str,
    version: Value,
    outcome: &str,
    reason: Option<&str>,
    extra: Value,
) {
    let Some(sender) = &ctx.control_state.audit_tx else {
        return;
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut details = json!({
        "time_ms": u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
        "who": who,
        "version": version,
        "outcome": outcome,
        "reason": reason,
    });
    if let (Some(details), Some(extra)) = (details.as_object_mut(), extra.as_object()) {
        details.extend(extra.clone());
    }
    let _ = sender.send(ControlAuditEvent {
        event_id: SmolStr::new(format!("{kind}-{timestamp_ms}-{sequence}")),
        timestamp_ms,
        request_id: sequence,
        request_type: SmolStr::new(kind),
        correlation_id: None,
        ok: outcome == "activated",
        error: reason.map(SmolStr::new),
        auth_present: true,
        client: None,
        details: Some(details),
    });
}

fn respond(request: Request, status: u16, body: &Value) {
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes("Content-Type", "application/json").expect("a valid header"),
        )
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("a valid header"));
    let _ = request.respond(response);
}
