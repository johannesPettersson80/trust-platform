//! What Live Values and the simulation speed show of the runtime they act on (specification 25 §4;
//! specification 35 A2.4 and B9.2): its state, its endpoint and whether it answers, whether it is
//! simulated, the operations its session may use and whether a Force must be armed first, and the
//! speed it reports. The runtime's own `status` decides; the editor settings and the debug
//! configuration never make a target simulated.
//!
//! - `liveValues` `{ runtimeMode, endpoint?, authToken?, endpointEnabled?, inlineValuesEnabled?,
//!   session?, projectEndpoint? }`, `session` the debug session's configuration (`request`,
//!   `endpoint`, `authToken`, `controlEndpoint`, `controlAuthToken`, `targetLabel`) or `null`, and
//!   `projectEndpoint` the workspace project's own control endpoint: `{ ok: true, status,
//!   adsReport }`, `status` the panel's runtime status with what each section's rows may do and
//!   whether the project's own runtime answers, and `adsReport` the runtime's `ads.status` answer
//!   while an online target answers without a session.
//! - `speed` `{ endpoint, authToken? }`: `{ ok: true, read }`, the simulation facts and speeds that
//!   the runtime's `status` reports, `null` when it cannot be read.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::request::{reachable, send, Outcome};
use super::{decode, non_blank};

/// Why an output or a memory row is not written: the program drives it (specification 25 §4.2).
const PROGRAM_DRIVEN: &str = "Program-driven — use Force to override";
/// Why a forced row is not written.
const RELEASE_FIRST: &str = "Release force before writing this value.";
/// Why an ADS tag added without writes is neither written nor forced.
const READ_ONLY_TAG: &str =
    "Read-only ADS tag. Enable writes when adding the tag, then restart the runtime.";
/// Why a row is not written when nothing more particular says why.
const NO_WRITE: &str = "Write is not available for this value.";
/// Why a row is not forced when nothing more particular says why.
const NO_FORCE: &str = "Force is not available for this value.";
/// The wait of the status requests (the tools' budget, specification 25 §5.1).
const STATUS_TIMEOUT: Duration = Duration::from_millis(750);
/// The wait of a reachability probe.
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
/// How long a probe's answer stands for its endpoint, so that the frequent refreshes of the
/// lifecycle do not probe each time.
const PROBE_REUSE: Duration = Duration::from_millis(2000);

/// The probes that stand for their endpoints, the newest last: each endpoint, its answer and when
/// it was made. A refresh probes the target and the project's own runtime, so each keeps its own.
static PROBES: Mutex<Vec<(String, bool, Instant)>> = Mutex::new(Vec::new());
/// How many endpoints' probes are kept.
const PROBES_KEPT: usize = 8;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    #[serde(default)]
    request: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    control_endpoint: Option<String>,
    #[serde(default)]
    control_auth_token: Option<String>,
    #[serde(default)]
    target_label: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveArgs {
    #[serde(default)]
    runtime_mode: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    endpoint_enabled: Option<bool>,
    #[serde(default)]
    inline_values_enabled: Option<bool>,
    #[serde(default)]
    session: Option<Session>,
    /// The workspace project's own control endpoint, as `trust-runtime ctl --project <dir> endpoint
    /// --json` reports it (specification 11 §6.8.0a).
    #[serde(default)]
    project_endpoint: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpeedArgs {
    endpoint: String,
    #[serde(default)]
    auth_token: Option<String>,
}

/// The answer of the action `action`.
pub(super) fn answer(action: &str, argument: Value) -> Result<Value, String> {
    if action == "speed" {
        let args: SpeedArgs = decode(argument)?;
        let report = status_report(&args.endpoint, args.auth_token.as_deref());
        return Ok(json!({ "ok": true, "read": report.as_ref().map(speed_read) }));
    }
    let args: LiveArgs = decode(argument)?;
    let (status, ads) = live_values(&args, &Network);
    Ok(json!({ "ok": true, "status": status, "adsReport": ads }))
}

/// The runtime that answers the requests: the network, or a test's stand-in.
pub(super) trait Runtimes {
    /// The `status` answer at `endpoint`, `None` when there is none.
    fn status(&self, endpoint: &str, token: Option<&str>) -> Option<Value>;
    /// The `ads.status` answer at `endpoint`, `None` when there is none.
    fn ads_status(&self, endpoint: &str, token: Option<&str>) -> Option<Value>;
    /// Whether `endpoint` accepts a connection.
    fn reachable(&self, endpoint: &str) -> bool;
}

/// The runtimes on the network.
struct Network;

impl Runtimes for Network {
    fn status(&self, endpoint: &str, token: Option<&str>) -> Option<Value> {
        status_report(endpoint, token)
    }

    fn ads_status(&self, endpoint: &str, token: Option<&str>) -> Option<Value> {
        match send(endpoint, token, "ads.status", None, STATUS_TIMEOUT) {
            Outcome::Result(result) => Some(result),
            _ => None,
        }
    }

    fn reachable(&self, endpoint: &str) -> bool {
        reachable_reused(endpoint)
    }
}

/// The `status` answer at `endpoint`, `None` when the runtime does not answer it.
fn status_report(endpoint: &str, token: Option<&str>) -> Option<Value> {
    match send(endpoint, token, "status", None, STATUS_TIMEOUT) {
        Outcome::Result(result) => Some(result),
        _ => None,
    }
}

/// Whether `endpoint` accepts a connection, the last probe of it standing for [`PROBE_REUSE`].
fn reachable_reused(endpoint: &str) -> bool {
    if let Ok(probes) = PROBES.lock()
        && let Some((_, answer, _)) = probes
            .iter()
            .find(|(probed, _, at)| probed == endpoint && at.elapsed() < PROBE_REUSE)
    {
        return *answer;
    }
    let answer = reachable(endpoint, PROBE_TIMEOUT);
    remember_probe(endpoint, answer);
    answer
}

/// Records a probe's answer for `endpoint`, as a fresh probe leaves it.
pub(super) fn remember_probe(endpoint: &str, answer: bool) {
    if let Ok(mut probes) = PROBES.lock() {
        probes.retain(|(probed, _, _)| probed != endpoint);
        if probes.len() == PROBES_KEPT {
            probes.remove(0);
        }
        probes.push((endpoint.to_owned(), answer, Instant::now()));
    }
}

/// The runtime status of Live Values and the runtime's `ads.status` answer, from `args`, asking
/// `runtimes`.
fn live_values(args: &LiveArgs, runtimes: &dyn Runtimes) -> (Value, Option<Value>) {
    let mode = if args.runtime_mode.as_deref() == Some("online") {
        "online"
    } else {
        "simulate"
    };
    let configured = args
        .endpoint
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_owned();
    let token = non_blank(args.auth_token.as_deref());
    let enabled = args.endpoint_enabled.unwrap_or(true);
    let session = args.session.as_ref();
    let attach = session.is_some_and(|session| session.request.as_deref() == Some("attach"));
    let state = match session {
        Some(_) if attach => "connected",
        Some(_) => "running",
        None => "stopped",
    };
    // A running session's own endpoint and token are the ones its status is asked at.
    let (endpoint, status_endpoint, status_token) = match session {
        Some(session) => {
            let (own_endpoint, own_token) = if attach {
                (&session.endpoint, &session.auth_token)
            } else {
                (&session.control_endpoint, &session.control_auth_token)
            };
            let own_endpoint = non_blank(own_endpoint.as_deref());
            let endpoint = own_endpoint.clone().unwrap_or_else(|| configured.clone());
            let status_token = non_blank(own_token.as_deref()).or_else(|| token.clone());
            (endpoint.clone(), endpoint, status_token)
        }
        None => (configured.clone(), configured.clone(), token.clone()),
    };
    let mut report = None;
    let mut ads = None;
    let mut reachable = false;
    if session.is_some() && !status_endpoint.is_empty() {
        report = runtimes.status(&status_endpoint, status_token.as_deref());
    }
    // The configured endpoint is probed only online, with no session and the endpoint in use.
    let probed = session.is_none() && mode == "online" && !configured.is_empty() && enabled;
    if probed {
        reachable = runtimes.reachable(&configured);
        if reachable {
            report = runtimes.status(&configured, token.as_deref());
            ads = runtimes.ads_status(&configured, token.as_deref());
        }
    }
    let facts = report.as_ref().map(target_facts).unwrap_or_default();
    let access = report
        .as_ref()
        .and_then(|report| report.get("access"))
        .and_then(reported_access)
        .unwrap_or_else(|| default_access(mode, state));
    let mut status = Map::new();
    status.insert("running".into(), json!(session.is_some()));
    status.insert(
        "inlineValuesEnabled".into(),
        json!(args.inline_values_enabled.unwrap_or(true)),
    );
    status.insert("runtimeMode".into(), json!(mode));
    // Only a running session's runtime is asked whether it is simulated (specification 35 A2.4).
    status.insert(
        "simulated".into(),
        json!(session.is_some() && facts.simulated),
    );
    if session.is_some()
        && let Some(cycle) = facts.cycle_interval_ms
    {
        status.insert("cycleIntervalMs".into(), json!(cycle));
    }
    status.insert("runtimeState".into(), json!(state));
    if let Some(label) = session.and_then(|session| non_blank(session.target_label.as_deref())) {
        status.insert("targetLabel".into(), json!(label));
    }
    status.insert("endpoint".into(), json!(endpoint));
    status.insert("endpointConfigured".into(), json!(!configured.is_empty()));
    status.insert("endpointEnabled".into(), json!(enabled));
    status.insert("endpointProbed".into(), json!(probed));
    status.insert("endpointReachable".into(), json!(reachable));
    // The project's own runtime, which the Target picker offers while it answers (specification 25
    // §5.1); the target's probe stands for it when the target is that runtime.
    let project = non_blank(args.project_endpoint.as_deref()).unwrap_or_default();
    let project_reachable = !project.is_empty()
        && if probed && project == configured {
            reachable
        } else {
            runtimes.reachable(&project)
        };
    status.insert("projectEndpoint".into(), json!(project));
    status.insert("projectReachable".into(), json!(project_reachable));
    status.insert("rows".into(), rows(&access));
    status.insert("access".into(), access);
    // A Force on anything but the Simulator's own session must be armed first (specification 25
    // §4.2): a simulator Force is one deliberate action.
    status.insert(
        "forceRequiresArm".into(),
        json!(mode != "simulate" || state == "connected"),
    );
    (Value::Object(status), ads)
}

/// What the rows of each section may do in each row state (specification 25 §4.1, §4.2): a `live`
/// row, a `forced` row and a `readOnly` row, an ADS tag added without writes; a forced row that is
/// read-only is `forced`. An input and an ADS tag are written, forced and released as the session's
/// access allows; an output and a memory location are program-driven, so they are forced but never
/// written. A forced row is released, never written or forced again; a read-only row is neither
/// written nor forced. Each operation that is not allowed carries the reason the panel shows, the
/// most particular first: the row's state, the section, then the session's access.
fn rows(access: &Value) -> Value {
    let allows = |operation: &str| access.get(operation).and_then(Value::as_bool) == Some(true);
    let access_reason = non_blank(access.get("reason").and_then(Value::as_str));
    let operations = |allowed: [bool; 3], reasons: [Option<&str>; 3]| {
        let [write, force, release] = allowed;
        let [write_reason, force_reason, release_reason] = reasons;
        let unless = |allowed: bool, reason: Option<&str>| {
            if allowed {
                Value::Null
            } else {
                json!(reason)
            }
        };
        json!({
            "write": write,
            "force": force,
            "release": release,
            "writeReason": unless(write, write_reason),
            "forceReason": unless(force, force_reason),
            "releaseReason": unless(release, release_reason),
        })
    };
    let access_or = |fallback: &'static str| Some(access_reason.as_deref().unwrap_or(fallback));
    let section = |writable: bool, section_reason: Option<&'static str>| {
        let write = writable && allows("allowWrite");
        json!({
            "live": operations(
                [write, allows("allowForce"), false],
                [section_reason.or(access_or(NO_WRITE)), access_or(NO_FORCE), access_reason.as_deref()],
            ),
            "forced": operations(
                [false, false, allows("allowRelease")],
                [Some(RELEASE_FIRST), access_or(NO_FORCE), access_reason.as_deref()],
            ),
            "readOnly": operations(
                [false, false, false],
                [section_reason.or(Some(READ_ONLY_TAG)), Some(READ_ONLY_TAG), access_reason.as_deref()],
            ),
        })
    };
    json!({
        "inputs": section(true, None),
        "outputs": section(false, Some(PROGRAM_DRIVEN)),
        "memory": section(false, Some(PROGRAM_DRIVEN)),
        "ads": section(true, None),
    })
}

/// The access the runtime reports: its role and the I/O operations it allows.
fn reported_access(access: &Value) -> Option<Value> {
    let access = access.as_object()?;
    let io = access.get("io").and_then(Value::as_object);
    let allows = |operation: &str| {
        io.and_then(|io| io.get(operation))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    Some(json!({
        "role": access.get("role").and_then(Value::as_str),
        "allowWrite": allows("write"),
        "allowForce": allows("force"),
        "allowRelease": allows("release"),
        "reason": access.get("reason").and_then(Value::as_str),
    }))
}

/// The access when the runtime reports none: the Simulator's session may do everything; an online
/// target nothing, with the reason to connect with an engineer's token.
fn default_access(mode: &str, state: &str) -> Value {
    if mode == "simulate" {
        return json!({
            "role": "admin",
            "allowWrite": true,
            "allowForce": true,
            "allowRelease": true,
            "reason": null,
        });
    }
    let reason = if matches!(state, "connected" | "running") {
        "Write/force permissions are unknown — reconnect with an engineer token."
    } else {
        "Connect with an engineer token to write or force."
    };
    json!({
        "role": null,
        "allowWrite": false,
        "allowForce": false,
        "allowRelease": false,
        "reason": reason,
    })
}

/// The simulation facts a `status` answer reports (specification 35 A2.4 and B9.2).
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct TargetFacts {
    /// Simulation is enabled and every attached I/O driver is simulated.
    pub(super) simulated: bool,
    /// The configured speed: the reported time scale while simulation is enabled, else 1.
    pub(super) time_scale: f64,
    /// The measured speed of a running simulation, once measured.
    pub(super) achieved: Option<f64>,
    /// The runtime's speed words.
    pub(super) speed_text: String,
    /// Whether the runtime says the achieved speed is behind the configured one.
    pub(super) speed_behind: bool,
    /// The resource cycle, when the runtime reports one.
    pub(super) cycle_interval_ms: Option<f64>,
}

/// The facts of the `status` answer `report`, each read as the runtime states it, with the stricter
/// default where it states none: no `simulated` is a physical target, no speed words are 1×, and no
/// usable time scale is the speed 1.
pub(super) fn target_facts(report: &Value) -> TargetFacts {
    let field = |key: &str| report.get(key);
    let flag = |key: &str| field(key).and_then(Value::as_bool) == Some(true);
    let positive = |key: &str| {
        field(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value > 0.0)
    };
    let enabled = flag("simulation_enabled");
    TargetFacts {
        simulated: flag("simulated"),
        time_scale: positive("simulation_time_scale")
            .filter(|_| enabled)
            .unwrap_or(1.0),
        achieved: positive("simulation_achieved_time_scale").filter(|_| enabled),
        speed_text: field("simulation_speed_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        speed_behind: flag("simulation_speed_behind"),
        cycle_interval_ms: positive("cycle_interval_ms"),
    }
}

/// The speed read of the `status` answer `report`: its facts, its configured and achieved speeds,
/// and whether its scan is paused.
fn speed_read(report: &Value) -> Value {
    let facts = target_facts(report);
    json!({
        "simulated": facts.simulated,
        "configured": facts.time_scale,
        "achieved": facts.achieved,
        "speedText": facts.speed_text,
        "speedBehind": facts.speed_behind,
        "paused": report.get("state").and_then(Value::as_str) == Some("paused"),
    })
}

#[cfg(test)]
#[path = "live_values_tests.rs"]
mod tests;
