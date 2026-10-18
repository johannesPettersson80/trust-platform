//! `ads.tags_set`: the ADS tag browser's one add path (design §4.3 B7 and §3.3 J56, J58, I19, I21,
//! I22, I23, I29, I72 and N36; specification 25 §5.4 and its lines 387-394 and 399-401). Ticking a
//! tag of one ADS connection and port imports it; unticking it removes it. A running runtime that
//! owns the connection imports and removes over its control endpoint
//! (`ads.import_symbols.apply` and `ads.import_symbols.remove`); a stopped project imports and
//! removes in the project's own files with the same Rust functions the `trust-runtime ads`
//! commands use, and enables `[runtime.ads]` as the import does. The answer is a notice with the
//! counts, a refresh, and the generated Structured Text opened so diagnostics can refresh
//! (specification 25 §5.4); an offline add offers `Start runtime` (I22 and I72).

use serde_json::{json, Map, Value};

use super::write::{self, Writes};
use crate::request::ActRequest;
use crate::session::Session;
use crate::texts::browse as browse_texts;
use crate::texts::forms as form_texts;
use crate::texts::plans as texts;
use crate::view::types::{ActReply, Action, Emphasis, HostEffect, Op, Refresh, Tone};

/// The ADS port a connection uses when neither the payload nor the connection names one
/// (`DEFAULT_ADS_PORT`, `W/adsTargetPort.ts:3`).
const DEFAULT_ADS_PORT: u16 = 851;

/// The action's payload named no ADS connection.
const NO_CONNECTION: &str = "The action did not name the ADS connection.";

/// `ads.tags_set`: imports the ticked tags and removes the unticked ones of the named ADS
/// connection and port (design §3.3 J56; specification 25 §5.4). The connection, its target and
/// its port come from the session's own view of the runtime, never from the extension; a
/// connection that is not the runtime's own ADS connection is refused and nothing is written.
#[must_use]
pub(crate) fn tags_set(session: &Session, request: &ActRequest) -> ActReply {
    let pane =
        super::payload_text(&request.op.payload, "pane").unwrap_or_else(|| super::pane(request));
    let Some(runtime_id) = super::payload_text(&request.op.payload, "runtime") else {
        return super::error(&pane, form_texts::NO_RUNTIME);
    };
    let Some(connection_id) = super::payload_text(&request.op.payload, "connection") else {
        return super::error(&pane, NO_CONNECTION);
    };
    let Some(runtime) = session.runtime(&runtime_id) else {
        return super::error(&pane, &form_texts::runtime_not_shown(&runtime_id));
    };
    let Some(connection) = runtime.connection(&connection_id) else {
        return super::error(&pane, &browse_texts::connection_not_shown(&connection_id));
    };
    if connection.protocol != "ads" {
        return super::error(&pane, &browse_texts::not_browsable(&connection.protocol));
    }
    let Some(writes) = write::writes(runtime) else {
        return super::error(&pane, &form_texts::runtime_did_not_answer(&runtime.name));
    };
    let port = match ads_port(request, connection.params.get("ams_port")) {
        Ok(port) => port,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(route_name) = route_name(&connection.params) else {
        return super::error(&pane, texts::TAGS_NEED_ROUTE);
    };
    let target = ads_target(&connection.params, port);
    let ticked = paths_of(request, "ticked");
    let unticked = paths_of(request, "unticked");
    let writable = request
        .values
        .get("writable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let offline_add = matches!(writes, Writes::Project) && !ticked.is_empty();

    let mut added = 0usize;
    let mut generated = None;
    if !ticked.is_empty() {
        let params = import_params(
            &route_name,
            &ticked,
            &target,
            &connection.params,
            port,
            writable,
            request,
        );
        match send(request, &writes, "ads.import_symbols.apply", &params) {
            Ok(report) if applied(&report) => {
                added = selected_count(&report, ticked.len());
                generated = generated_path(&report);
            }
            Ok(report) => {
                return failure(
                    &pane,
                    texts::ads_tags_failed(reason_of(&report)),
                    offline_add,
                );
            }
            Err(reason) => {
                return failure(&pane, texts::ads_tags_failed(Some(&reason)), offline_add);
            }
        }
    }
    let mut removed = 0usize;
    if !unticked.is_empty() {
        let params = remove_params(&target, &unticked);
        match send(request, &writes, "ads.import_symbols.remove", &params) {
            Ok(report) if applied(&report) => removed = removed_count(&report),
            Ok(report) => return failure(&pane, texts::ads_tags_failed(reason_of(&report)), false),
            Err(reason) => return failure(&pane, texts::ads_tags_failed(Some(&reason)), false),
        }
    }
    let mut notice = super::notice(
        Tone::Ok,
        &pane,
        texts::ads_tags_notice(added, removed, port),
    );
    if offline_add {
        notice.actions.push(start_runtime());
    }
    let effects = generated
        .map(|path| vec![HostEffect::OpenDocument { path }])
        .unwrap_or_default();
    super::reply(true, Some(notice), effects, false, Refresh::Now)
}

/// The ADS port of the request: the payload's own port, checked as a whole number from 1 through
/// 65,535, else the connection's configured port, else the default (design §3.3 J59).
///
/// # Errors
///
/// The payload named port 0, which is not a valid ADS port.
fn ads_port(request: &ActRequest, configured: Option<&Value>) -> Result<u16, String> {
    if let Some(port) = request.op.payload.get("port").and_then(Value::as_u64) {
        return u16::try_from(port)
            .ok()
            .filter(|port| *port >= 1)
            .ok_or_else(|| browse_texts::ADS_PORT_RANGE.to_owned());
    }
    match configured
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
    {
        Some(port) if port >= 1 => Ok(port),
        _ => Ok(DEFAULT_ADS_PORT),
    }
}

/// The route name of a connection: its configured `name`, which the cached snapshot's route name
/// must match (`NC/protocolActions.ts:587-598`).
fn route_name(params: &Map<String, Value>) -> Option<String> {
    text(params, "name")
}

/// The ADS target identity of a connection, as `ads.import_symbols.apply` reads it
/// (`NC/protocolActions.ts:587-596`): its name, its address, its ADS net id, its port and, when it
/// has one, its `TwinCAT` version.
fn ads_target(params: &Map<String, Value>, port: u16) -> Value {
    let mut target = Map::new();
    if let Some(name) = text(params, "name") {
        target.insert("name".to_owned(), Value::String(name));
    }
    target.insert(
        "ip".to_owned(),
        Value::String(string_of(params, &["host", "ip"]).unwrap_or_default()),
    );
    target.insert(
        "ams_net_id".to_owned(),
        Value::String(string_of(params, &["target_net_id", "ams_net_id"]).unwrap_or_default()),
    );
    target.insert("ams_port".to_owned(), Value::Number(u64::from(port).into()));
    if let Some(tc_version) = text(params, "tc_version") {
        target.insert("tc_version".to_owned(), Value::String(tc_version));
    }
    Value::Object(target)
}

/// This host's ADS identity that the browse reported for `port`, when it reported one
/// (`adsPortBrowseEvidence` and `localAdsNetId`, `NC/adsDiscoveryPorts.ts:50-77`): the port's route
/// plan carries the identity the offline import writes. `None` when the evidence has none, and the
/// import then derives it.
fn local_identity(params: &Map<String, Value>, port: u16) -> Option<Value> {
    params
        .get("ads_port_browse_results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("port").and_then(Value::as_u64) == Some(u64::from(port)))
        .and_then(|item| item.get("routePlan"))
        .and_then(|plan| plan.get("local"))
        .filter(|local| local.is_object())
        .cloned()
}

/// The `ads.import_symbols.apply` parameters of the ticked tags: the connection's route, the
/// selected symbols, the target, the write acknowledgement, and the browse evidence the pane
/// carries (this host's identity and the cached symbol snapshot) when it has them.
fn import_params(
    connection_name: &str,
    symbols: &[String],
    target: &Value,
    params: &Map<String, Value>,
    port: u16,
    writable: bool,
    request: &ActRequest,
) -> Value {
    let mut value = json!({
        "connection_name": connection_name,
        "symbols": symbols,
        "target": target,
        "write_acknowledged": writable,
    });
    if let Some(local) = local_identity(params, port) {
        value["local"] = local;
    }
    if let Some(snapshot) = request
        .values
        .get("snapshot")
        .filter(|snapshot| snapshot.is_object())
    {
        value["snapshot"] = snapshot.clone();
    }
    value
}

/// The `ads.import_symbols.remove` parameters of the unticked paths: the target's address and ADS
/// net id, its port, and the paths (`design §1.4.2`).
fn remove_params(target: &Value, paths: &[String]) -> Value {
    let mut params = Map::new();
    params.insert(
        "ams_port".to_owned(),
        target.get("ams_port").cloned().unwrap_or(Value::Null),
    );
    if let Some(host) = target
        .get("ip")
        .and_then(Value::as_str)
        .filter(|ip| !ip.is_empty())
    {
        params.insert("host".to_owned(), Value::String(host.to_owned()));
    }
    if let Some(net_id) = target
        .get("ams_net_id")
        .and_then(Value::as_str)
        .filter(|net_id| !net_id.is_empty())
    {
        params.insert("target_net_id".to_owned(), Value::String(net_id.to_owned()));
    }
    params.insert("paths".to_owned(), json!(paths));
    Value::Object(params)
}

/// Sends one ADS import operation `entry` at `writes`: the runtime's control endpoint when it
/// answered, else the open project's files in process (design §4.3 B7;
/// [`actions::write`](crate::actions::write)).
fn send(
    request: &ActRequest,
    writes: &Writes,
    entry: &str,
    params: &Value,
) -> Result<Value, String> {
    match writes {
        Writes::Endpoint(endpoint) => write::send_control(request, endpoint, entry, params),
        Writes::Project => offline(request, entry, params),
    }
}

/// Runs one ADS import operation in the open project's files, with the same Rust function the
/// `trust-runtime ads` commands use (design §4.3 B7; specification 25 §5.4).
fn offline(request: &ActRequest, entry: &str, params: &Value) -> Result<Value, String> {
    let Some(project) = request.project.as_deref() else {
        return Err(form_texts::NO_PROJECT_FOLDER.to_owned());
    };
    match entry {
        "ads.import_symbols.apply" => {
            trust_runtime::control::offline_ads_import_symbols_json(project, params.clone())
        }
        "ads.import_symbols.remove" => {
            trust_runtime::control::offline_ads_remove_symbols_json(project, params.clone())
        }
        other => Err(format!("{other} has no offline ADS import.")),
    }
}

/// Whether an import report says the change was applied.
fn applied(report: &Value) -> bool {
    report.get("applied").and_then(Value::as_bool) == Some(true)
}

/// The report's own reason, when it gave one.
fn reason_of(report: &Value) -> Option<&str> {
    report
        .get("message")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
}

/// How many tags the import selected, else `fallback`.
fn selected_count(report: &Value, fallback: usize) -> usize {
    report
        .get("selected_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(fallback)
}

/// How many tags the removal took away.
fn removed_count(report: &Value) -> usize {
    report
        .get("removed_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

/// The generated Structured Text path that the import wrote, when it named one (specification 25
/// §5.4).
fn generated_path(report: &Value) -> Option<String> {
    report
        .get("generated_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
}

/// The trimmed, non-empty paths at `key` of the values.
fn paths_of(request: &ActRequest, key: &str) -> Vec<String> {
    request
        .values
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The trimmed, non-empty string at the first of `keys` that holds one.
fn string_of(params: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| text(params, key))
}

/// The trimmed, non-empty string at `key` of `params`.
fn text(params: &Map<String, Value>, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// A failed `ads.tags_set`: the notice in the pane the action started in. An offline add offers
/// `Start runtime` (I22 and I72).
fn failure(pane: &str, text: String, offline_add: bool) -> ActReply {
    let mut notice = super::notice(Tone::Error, pane, text);
    if offline_add {
        notice.actions.push(start_runtime());
    }
    super::reply(false, Some(notice), Vec::new(), false, Refresh::None)
}

/// The `Start runtime` action of an offline add (I22 and I72; `START_RUNTIME_ACTION`,
/// `NC/adsImportUx.ts:1`): it starts the open project's runtime, which applies the generated
/// Structured Text.
fn start_runtime() -> Action {
    Action {
        id: "start_runtime".to_owned(),
        label: texts::START_RUNTIME.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Host {
            effect: HostEffect::StartSimulator,
        },
    }
}
