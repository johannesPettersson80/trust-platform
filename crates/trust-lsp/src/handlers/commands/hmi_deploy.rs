//! Deploy and Roll back from the HMI Builder (specification 39, §16.2 item 8, §16.4): the language
//! server carries out the transaction. Deploy builds and checks the project's bundle as
//! `trust-lsp.hmiBundle` does, asks the selected runtime for its own web server
//! (`fleet.topology`) and sends the bundle there (`POST /api/hmi/deploy`) with the runtime's
//! administrator access, its control token. Roll back asks the same web server to make the
//! previous bundle active (`POST /api/hmi/rollback`). The extension saves the HMI files, passes the
//! selected run target with its token, and shows the outcome.
//!
//! `trust-lsp.hmiDeploy` `{ action: "deploy" | "rollback", project?, target?, authToken? }`:
//! `project` holds the arguments of `trust-lsp.hmiBundle` (Deploy only), and `target` is
//! `{ endpoint, label, simulator }`, absent when no runtime runs. The answer is `{ ok: true,
//! outcome }`, the outcome `{ ok: true, version, target }`, the version now active and the target's
//! name, or `{ ok: false, reason, problems? }`, what stopped it and the problems with file and
//! line; arguments it cannot read answer `ok: false` and an `error` text.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::hmi_builder::deploy::web_server_url;
use super::hmi_checks::{checked_bundle, BundleArgs};
use super::hmi_files::single_argument;
use super::run_targets::request::{send, Outcome as Answer};
use super::HMI_DEPLOY_COMMAND;
use crate::handlers::context::ServerContext;

/// How long the runtime may take to answer `fleet.topology`.
const TOPOLOGY_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the runtime may take to check and activate a bundle.
const DEPLOY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Args {
    action: Action,
    #[serde(default)]
    project: Option<BundleArgs>,
    #[serde(default)]
    target: Option<Target>,
    #[serde(default)]
    auth_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Deploy,
    Rollback,
}

/// The run target the HMI Builder talks to (`trust-lsp.runTargets` `hmiTarget`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    endpoint: String,
    label: String,
    simulator: bool,
}

/// The answer of `trust-lsp.hmiDeploy`.
pub(super) fn deploy_value<C: ServerContext>(context: &C, args: Vec<Value>) -> Value {
    let args: Args = match single_argument(args, HMI_DEPLOY_COMMAND) {
        Ok(args) => args,
        Err(error) => return json!({ "ok": false, "error": error }),
    };
    let (target, token) = (args.target.as_ref(), args.auth_token.as_deref());
    let outcome = match (args.action, args.project) {
        (Action::Deploy, Some(project)) => deploy(context, project, target, token),
        (Action::Deploy, None) => {
            let error = format!("{HMI_DEPLOY_COMMAND} needs the project to deploy.");
            return json!({ "ok": false, "error": error });
        }
        (Action::Rollback, _) => send_to_web_server(target, token, "/api/hmi/rollback", &json!({})),
    };
    json!({ "ok": true, "outcome": outcome })
}

/// Checks the project's bundle and sends it when the check finds no problem (§16.2 item 1).
fn deploy<C: ServerContext>(
    context: &C,
    project: BundleArgs,
    target: Option<&Target>,
    token: Option<&str>,
) -> Value {
    let problems = match checked_bundle(context, project) {
        Ok((bundle, problems)) if problems.is_empty() => {
            return send_to_web_server(target, token, "/api/hmi/deploy", &bundle.to_json());
        }
        Ok((_, problems)) => problems,
        Err(message) => vec![json!({ "message": message })],
    };
    stopped(counted("The editor check found", &problems, "."), problems)
}

/// Sends `body` to the web server of `target` at `path` and reads the runtime's answer: the
/// versions now active and kept (§16.4), or why it refused.
fn send_to_web_server(
    target: Option<&Target>,
    token: Option<&str>,
    path: &str,
    body: &Value,
) -> Value {
    let Some(target) = target else {
        return stopped(
            "No runtime is running: start a runtime to deploy to.".to_owned(),
            Vec::new(),
        );
    };
    if target.simulator {
        return stopped(
            "The simulator has no web server: use Live mode to see the page, or start a runtime to deploy to."
                .to_owned(),
            Vec::new(),
        );
    }
    let label = &target.label;
    let topology = match send(
        &target.endpoint,
        token,
        "fleet.topology",
        Some(json!({})),
        TOPOLOGY_TIMEOUT,
    ) {
        Answer::Result(topology) => topology,
        Answer::Refused { error, .. } | Answer::Transport(error) | Answer::Malformed(error) => {
            return stopped(format!("{label} does not answer: {error}"), Vec::new());
        }
    };
    let Some(url) = web_server_url(&target.endpoint, &topology) else {
        return stopped(
            format!("{label} has no web server: enable [runtime.web] in its runtime.toml."),
            Vec::new(),
        );
    };
    let (status, reply) = match post(&format!("{url}{path}"), token, body) {
        Ok(answer) => answer,
        Err(error) => return stopped(format!("{label} cannot be reached: {error}"), Vec::new()),
    };
    if status == 200 && reply.get("ok").and_then(Value::as_bool) == Some(true) {
        let version = reply
            .get("active")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return json!({ "ok": true, "version": version, "target": label });
    }
    let problems = reply
        .get("problems")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reason = match reply.get("error").and_then(Value::as_str) {
        Some(error) => error.to_owned(),
        None if !problems.is_empty() => {
            counted(&format!("{label} found"), &problems, " in the bundle.")
        }
        None => format!("{label} answered {status}"),
    };
    stopped(reason, problems)
}

/// Posts `body` as JSON to `url` with the token `token` (a blank token is never sent): the
/// answer's status and its body, null when that is not JSON; or why nothing answered. The request
/// goes straight to the runtime, through no proxy, and a redirect is answered as it is, never
/// followed, so the token reaches only the runtime's web server.
fn post(url: &str, token: Option<&str>, body: &Value) -> Result<(u16, Value), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(DEPLOY_TIMEOUT))
        .http_status_as_error(false)
        .proxy(None)
        .max_redirects(0)
        .max_redirects_will_error(false)
        .build()
        .into();
    let mut request = agent
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");
    if let Some(token) = token.map(str::trim).filter(|token| !token.is_empty()) {
        request = request.header("X-Trust-Token", token);
    }
    let mut response = request
        .send(body.to_string())
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// `{before} 1 problem{after}` or `{before} 2 problems{after}`.
fn counted(before: &str, problems: &[Value], after: &str) -> String {
    let plural = if problems.len() == 1 { "" } else { "s" };
    format!("{before} {} problem{plural}{after}", problems.len())
}

/// An outcome that changed nothing, with its reason and the problems behind it.
fn stopped(reason: String, problems: Vec<Value>) -> Value {
    if problems.is_empty() {
        json!({ "ok": false, "reason": reason })
    } else {
        json!({ "ok": false, "reason": reason, "problems": problems })
    }
}
