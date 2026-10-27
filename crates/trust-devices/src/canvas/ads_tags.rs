//! ADS tags imported into and removed from the open project (specification 11, "ADS symbol-import
//! apply and merge"; specification 25 §5.4 and §5.8). While the runtime that serves the canvas's
//! live data answers, it imports with `ads.import_symbols.apply`; otherwise the import runs in this
//! process into the project's files with the same Rust function, which enables `[runtime.ads]`. A
//! removal always rewrites the project's files. A tag batch imports each ADS port's tags into the
//! connection that already serves the device on that port, else into one named after the device.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::act::{CanvasActRequest, CanvasNotice, CanvasNoticeAction, CanvasReply, LiveRuntime};
use super::ads::DEFAULT_ADS_PORT;
use crate::actions::plans::ads_connection_name;
use crate::actions::write;
use crate::texts::plans::START_RUNTIME;

/// The wait for a live import, which uploads the device's symbols first.
const LIVE_IMPORT_TIMEOUT: Duration = Duration::from_secs(20);
/// The generated Structured Text that an import or a removal rewrites, relative to the project.
const GENERATED_ADS_ST: &str = "src/generated/ads_generated.st";
/// The connection name an import falls back to.
const FALLBACK_CONNECTION: &str = "ads_import";
/// The prefix of an ADS symbol's selection key in the browse tree.
const SYMBOL_KEY_PREFIX: &str = "ads:symbol:";

/// The arguments of `ads_tags_add`: the browsed device, the tags chosen per ADS port, whether they
/// are imported writable, and the one path the user changed, when the batch follows one tick.
#[derive(Debug, Deserialize)]
pub(super) struct BatchArgs {
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    selections: Value,
    #[serde(default)]
    writable: bool,
    #[serde(default)]
    changed_path: Option<String>,
}

/// The arguments of `ads_tags_import`: the browse pane's protocol, its target, the chosen paths and
/// whether they are imported writable.
#[derive(Debug, Deserialize)]
pub(super) struct ImportArgs {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    paths: Vec<Value>,
    #[serde(default)]
    writable: bool,
}

/// The arguments of `ads_tag_remove`: the browsed device and the one tag removed from one port.
#[derive(Debug, Deserialize)]
pub(super) struct RemoveArgs {
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    selection: Value,
}

/// The tags of one ADS port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortSelection {
    /// The ADS port.
    pub port: u16,
    /// The tag paths, each once, sorted.
    pub paths: Vec<String>,
}

/// What one import did.
struct Imported {
    applied: bool,
    message: String,
    /// Why an offline import refused writable tags: it needs a running runtime.
    needs_runtime: bool,
    documents: Vec<String>,
}

/// `ads_tags_add`: imports the chosen tags of each port and answers the batch result that the tag
/// browser shows (`ports`, `addedCount`, `restartRequired` and `summary`).
pub(super) fn add_batch(request: &CanvasActRequest, args: &BatchArgs) -> CanvasReply {
    let selections = port_selections(&args.selections);
    let changed = args
        .changed_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let mut ports = Vec::new();
    let mut documents = Vec::new();
    for selection in &selections {
        let imported = match request.project() {
            None => Imported::refused(texts::OPEN_PROJECT_TO_ADD),
            Some(project) => {
                let name = planned_connection(&args.target, selection.port);
                let mut target = args.target.clone();
                target.insert("name".to_owned(), Value::String(name.clone()));
                target.insert("ams_port".to_owned(), Value::from(selection.port));
                match request.live() {
                    Some(runtime) => import_live(
                        runtime,
                        &target,
                        &selection.paths,
                        args.writable,
                        Some(&name),
                    ),
                    None => import_offline(
                        project,
                        &target,
                        &selection.paths,
                        args.writable,
                        Some(&name),
                    ),
                }
            }
        };
        documents.extend(imported.documents.iter().cloned());
        // The tags the result names: the one the user changed, when the batch follows one tick of
        // an open project's tag browser, else every chosen tag of the port.
        let paths: Vec<String> = match changed {
            Some(path)
                if request.project().is_some()
                    && selection.paths.iter().any(|known| known == path) =>
            {
                vec![path.to_owned()]
            }
            _ => selection.paths.clone(),
        };
        ports.push(json!({
            "port": selection.port,
            "addedCount": if imported.applied { paths.len() } else { 0 },
            "message": if imported.applied { texts::added_tags(paths.len()) } else { imported.message },
            "paths": paths,
            "applied": imported.applied,
        }));
    }
    let applied = ports.iter().any(applied_port);
    let added: u64 = ports
        .iter()
        .filter_map(|port| port.get("addedCount").and_then(Value::as_u64))
        .sum();
    let summary = batch_summary("add", &ports, added);
    CanvasReply {
        applied,
        refresh: applied,
        open_documents: documents,
        result: Some(json!({
            "operation": "add",
            "applied": applied,
            "addedCount": added,
            "restartRequired": applied,
            "ports": ports,
            "summary": summary,
        })),
        ..CanvasReply::nothing()
    }
}

/// `ads_tags_import`: imports the browse pane's chosen tags, at the live runtime or into the
/// project's files, and tells the result; an offline import of writable tags offers to start the
/// runtime, which applies the explicit write acknowledgement.
pub(super) fn import(request: &CanvasActRequest, args: &ImportArgs) -> CanvasReply {
    let paths: Vec<String> = args
        .paths
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if paths.is_empty() {
        return CanvasReply::nothing();
    }
    let Some(project) = request.project() else {
        return CanvasReply::warning(texts::IMPORT_NEEDS_PROJECT);
    };
    if let Some(runtime) = request.live() {
        let imported = import_live(runtime, &args.target, &paths, args.writable, None);
        if !imported.applied {
            return CanvasReply::warning(texts::could_not_add_tags(&imported.message));
        }
        return CanvasReply {
            open_documents: imported.documents,
            ..CanvasReply::done(texts::live_tags_added(&imported.message))
        };
    }
    if args.protocol.trim() != "ads" {
        return CanvasReply::warning(texts::IMPORT_NEEDS_RUNTIME);
    }
    let imported = import_offline(project, &args.target, &paths, args.writable, None);
    if imported.applied {
        return CanvasReply {
            open_documents: imported.documents,
            ..CanvasReply::done(imported.message)
        };
    }
    if imported.needs_runtime {
        return CanvasReply {
            notice: Some(CanvasNotice {
                detail: Some(texts::WRITABLE_NEEDS_RUNTIME_DETAIL.to_owned()),
                modal: true,
                actions: vec![CanvasNoticeAction {
                    id: "start_runtime",
                    label: START_RUNTIME,
                }],
                ..CanvasNotice::warning(texts::WRITABLE_NEEDS_RUNTIME_TITLE)
            }),
            ..CanvasReply::nothing()
        };
    }
    CanvasReply::warning(texts::could_not_add_ads_tags(&imported.message))
}

/// `ads_tag_remove`: removes one tag of one port from the project's files and answers the batch
/// result that the tag browser shows. A selection without a valid port or a path does nothing.
pub(super) fn remove(request: &CanvasActRequest, args: &RemoveArgs) -> CanvasReply {
    let Some(port) = args.selection.get("port").and_then(ads_port) else {
        return CanvasReply::nothing();
    };
    let Some(path) = args
        .selection
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return CanvasReply::nothing();
    };
    let removal = match request.project() {
        None => Removal::refused(texts::OPEN_PROJECT_TO_REMOVE),
        Some(project) => remove_offline(project, &args.target, port, path),
    };
    let ports = vec![json!({
        "port": port,
        "paths": [path],
        "applied": removal.applied,
        "addedCount": 0,
        "message": removal.message,
    })];
    let summary = batch_summary("remove", &ports, removal.removed);
    CanvasReply {
        applied: removal.applied,
        refresh: removal.applied,
        open_documents: removal.documents,
        result: Some(json!({
            "operation": "remove",
            "applied": removal.applied,
            "addedCount": 0,
            "removedCount": removal.removed,
            "restartRequired": removal.restart,
            "ports": ports,
            "summary": summary,
        })),
        ..CanvasReply::nothing()
    }
}

/// The tags chosen per ADS port: each port a whole number from 1 to 65535, each path trimmed and
/// kept once, a port without a path left out, the ports and their paths sorted.
pub(super) fn port_selections(value: &Value) -> Vec<PortSelection> {
    let mut selections: Vec<PortSelection> = Vec::new();
    for item in value.as_array().into_iter().flatten() {
        let Some(port) = item.get("port").and_then(ads_port) else {
            continue;
        };
        let Some(paths) = item.get("paths").and_then(Value::as_array) else {
            continue;
        };
        let paths: Vec<String> = paths
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect();
        match selections.iter_mut().find(|known| known.port == port) {
            Some(known) => known.paths.extend(paths),
            None => selections.push(PortSelection { port, paths }),
        }
    }
    for selection in &mut selections {
        selection.paths.sort();
        selection.paths.dedup();
    }
    selections.retain(|selection| !selection.paths.is_empty());
    selections.sort_by_key(|selection| selection.port);
    selections
}

/// The configured ADS connections of `connections` that serve the device `target`: a connection
/// whose AMS Net ID and host do not contradict the device's, and that shares either with it; with
/// neither on the device, every connection.
pub(super) fn device_connections<'a>(
    connections: &'a Value,
    target: &Map<String, Value>,
) -> Vec<&'a Map<String, Value>> {
    connections
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter(|connection| same_device(connection, target))
        .collect()
}

/// The tags already imported for the device `target`, per ADS port, from the points of its
/// configured connections (their `symbol`, else their `path`).
pub(super) fn imported_selections(
    connections: &Value,
    target: &Map<String, Value>,
) -> Vec<PortSelection> {
    let items: Vec<Value> = device_connections(connections, target)
        .into_iter()
        .map(|connection| {
            let paths: Vec<Value> = connection
                .get("points")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|point| {
                    point
                        .get("symbol")
                        .filter(|symbol| !symbol.is_null())
                        .or_else(|| point.get("path"))
                })
                .filter(|path| path.is_string())
                .cloned()
                .collect();
            json!({
                "port": connection.get("ams_port").cloned().unwrap_or(Value::Null),
                "paths": paths,
            })
        })
        .collect();
    port_selections(&Value::Array(items))
}

/// The connection that a port's tags are imported into: the device's configured connection on that
/// port, else the device's own connection name for that port.
fn planned_connection(target: &Map<String, Value>, port: u16) -> String {
    let configured = target
        .get("connections")
        .map_or_else(Vec::new, |connections| {
            device_connections(connections, target)
        });
    configured
        .into_iter()
        .find(|connection| {
            connection.get("ams_port").and_then(Value::as_u64) == Some(u64::from(port))
        })
        .and_then(|connection| text(connection, &["name"]))
        .unwrap_or_else(|| {
            let mut port_target = target.clone();
            port_target.insert("ams_port".to_owned(), Value::from(port));
            ads_connection_name(&port_target, FALLBACK_CONNECTION)
        })
}

/// Whether `connection` serves the device `target` (see [`device_connections`]).
fn same_device(connection: &Map<String, Value>, target: &Map<String, Value>) -> bool {
    let target_net_id = text(target, &["target_net_id", "ams_net_id"]);
    let connection_net_id = text(connection, &["target_net_id", "ams_net_id"]);
    if let (Some(target), Some(connection)) = (&target_net_id, &connection_net_id)
        && target != connection
    {
        return false;
    }
    let target_host = text(target, &["host", "ip"]);
    let connection_host = text(connection, &["host", "ip"]);
    if let (Some(target), Some(connection)) = (&target_host, &connection_host)
        && target != connection
    {
        return false;
    }
    (target_net_id.is_some() && connection_net_id.is_some())
        || (target_host.is_some() && connection_host.is_some())
        || (target_net_id.is_none() && target_host.is_none())
}

/// Imports `paths` at the runtime that serves the live data (`ads.import_symbols.apply`).
fn import_live(
    runtime: &LiveRuntime,
    source: &Map<String, Value>,
    paths: &[String],
    writable: bool,
    connection: Option<&str>,
) -> Imported {
    let port = source
        .get("ams_port")
        .filter(|port| port.is_number())
        .cloned()
        .unwrap_or_else(|| Value::from(DEFAULT_ADS_PORT));
    let mut target = Map::new();
    if let Some(name) = source.get("name").filter(|name| name.is_string()) {
        target.insert("name".to_owned(), name.clone());
    }
    if let Some(ip) = string_or(source, "host", "ip") {
        target.insert("ip".to_owned(), ip);
    }
    if let Some(net_id) = string_or(source, "target_net_id", "ams_net_id") {
        target.insert("ams_net_id".to_owned(), net_id);
    }
    target.insert("ams_port".to_owned(), port);
    if let Some(version) = source
        .get("tc_version")
        .filter(|version| !version.is_null())
    {
        target.insert("tc_version".to_owned(), version.clone());
    }
    let connection_name = connection.map_or_else(
        || ads_connection_name(source, FALLBACK_CONNECTION),
        str::to_owned,
    );
    let params = json!({
        "connection_name": connection_name,
        "symbols": paths,
        "target": target,
        "write_acknowledged": writable,
    });
    match write::send_with_token(
        &runtime.endpoint,
        runtime.token.as_deref(),
        "ads.import_symbols.apply",
        &params,
        LIVE_IMPORT_TIMEOUT,
    ) {
        Ok(report) if applied(&report) => {
            let added = count(&report, "selected_count").unwrap_or(paths.len());
            Imported {
                applied: true,
                message: texts::added_tags(added),
                needs_runtime: false,
                documents: generated_document(&report),
            }
        }
        Ok(report) => Imported::refused(
            report_message(&report).unwrap_or_else(|| texts::IMPORT_REJECTED.to_owned()),
        ),
        Err(reason) => Imported::refused(texts::live_import_failed(&reason)),
    }
}

/// Imports `paths` into the project's files in this process, as the runtime's import does
/// (`trust_runtime::control::offline_ads_import_symbols_json`). Writable tags need a running
/// runtime, which applies the explicit write acknowledgement.
fn import_offline(
    project: &Path,
    target: &Map<String, Value>,
    paths: &[String],
    writable: bool,
    connection: Option<&str>,
) -> Imported {
    if text(target, &["host", "ip"]).is_none() {
        return Imported::refused(texts::IMPORT_NEEDS_HOST);
    }
    if writable {
        return Imported {
            needs_runtime: true,
            ..Imported::refused(texts::WRITABLE_NEEDS_RUNTIME)
        };
    }
    let (params, symbols) = match offline_import_params(target, paths, connection) {
        Ok(planned) => planned,
        Err(reason) => return Imported::refused(reason),
    };
    match trust_runtime::control::offline_ads_import_symbols_json(project, params) {
        Ok(report) if applied(&report) => {
            let added = count(&report, "selected_count").unwrap_or(symbols);
            Imported {
                applied: true,
                message: texts::offline_tags_added(added),
                needs_runtime: false,
                documents: generated_document(&report),
            }
        }
        Ok(report) => Imported::refused(
            report_message(&report).unwrap_or_else(|| texts::IMPORT_FAILED.to_owned()),
        ),
        Err(reason) => Imported::refused(reason),
    }
}

/// The `ads.import_symbols.apply` parameters of a read-only import of `paths` from the device
/// `target` into `connection`, else into the device's own connection name, with the number of
/// symbols they name: each path without the browse tree's `ads:symbol:` prefix, and the device's
/// name, address, AMS Net ID, ADS port (851 when it names none) and `TwinCAT` version.
///
/// # Errors
///
/// The text that refuses the import: the target names no host, or no path names a symbol.
pub(super) fn offline_import_params(
    target: &Map<String, Value>,
    paths: &[String],
    connection: Option<&str>,
) -> Result<(Value, usize), &'static str> {
    let host = text(target, &["host", "ip"]).ok_or(texts::IMPORT_NEEDS_HOST)?;
    let symbols: Vec<&str> = paths
        .iter()
        .map(|path| path.strip_prefix(SYMBOL_KEY_PREFIX).unwrap_or(path))
        .filter(|symbol| !symbol.is_empty())
        .collect();
    if symbols.is_empty() {
        return Err(texts::SELECT_A_SYMBOL);
    }
    let connection_name = connection.map_or_else(
        || ads_connection_name(target, FALLBACK_CONNECTION),
        str::to_owned,
    );
    let mut identity = Map::new();
    if let Some(name) = target.get("name").filter(|name| name.is_string()) {
        identity.insert("name".to_owned(), name.clone());
    }
    identity.insert("ip".to_owned(), Value::String(host));
    if let Some(net_id) = text(target, &["target_net_id", "ams_net_id"]) {
        identity.insert("ams_net_id".to_owned(), Value::String(net_id));
    }
    identity.insert(
        "ams_port".to_owned(),
        target
            .get("ams_port")
            .filter(|port| port.as_u64().is_some_and(|port| port != 0))
            .cloned()
            .unwrap_or_else(|| Value::from(DEFAULT_ADS_PORT)),
    );
    if let Some(version) = target
        .get("tc_version")
        .filter(|version| version.is_string())
    {
        identity.insert("tc_version".to_owned(), version.clone());
    }
    let count = symbols.len();
    Ok((
        json!({
            "connection_name": connection_name,
            "symbols": symbols,
            "target": identity,
            "write_acknowledged": false,
        }),
        count,
    ))
}

/// What one removal of an ADS tag did, as the tag browser shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Removal {
    /// Whether it applied.
    pub(super) applied: bool,
    /// How many imported points it removed.
    pub(super) removed: u64,
    /// Whether the runtime must restart to apply it.
    pub(super) restart: bool,
    /// Its message.
    pub(super) message: String,
    /// The generated document it rewrote, which the canvas opens.
    pub(super) documents: Vec<String>,
}

impl Removal {
    fn refused(message: impl Into<String>) -> Self {
        Self {
            applied: false,
            removed: 0,
            restart: false,
            message: message.into(),
            documents: Vec::new(),
        }
    }
}

/// Removes `path` of `port` of the device `target` from the project's files in this process, as
/// `trust-runtime ads remove-symbols` does.
pub(super) fn remove_offline(
    project: &Path,
    target: &Map<String, Value>,
    port: u16,
    path: &str,
) -> Removal {
    let mut params = json!({ "ams_port": port, "paths": [path] });
    if let Some(host) = text(target, &["host", "ip"]) {
        params["host"] = Value::String(host);
    }
    if let Some(net_id) = text(target, &["target_net_id", "ams_net_id"]) {
        params["target_net_id"] = Value::String(net_id);
    }
    match trust_runtime::control::offline_ads_remove_symbols_json(project, params) {
        Err(reason) => Removal::refused(reason),
        Ok(report) => removal(project, &report),
    }
}

/// The removal that a report of `trust-runtime ads remove-symbols` describes: its applied flag,
/// count, restart need and message, and the generated Structured Text of `project` when the report
/// wrote it.
pub(super) fn removal(project: &Path, report: &Value) -> Removal {
    let wrote_generated = report
        .get("files_written")
        .and_then(Value::as_array)
        .is_some_and(|files| files.iter().any(|file| file == GENERATED_ADS_ST));
    Removal {
        applied: applied(report),
        removed: report
            .get("removed_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        restart: report.get("restart_required").and_then(Value::as_bool) == Some(true),
        message: report_message(report).unwrap_or_else(|| texts::REMOVAL_FAILED.to_owned()),
        documents: if wrote_generated {
            vec![display(&project.join(GENERATED_ADS_ST))]
        } else {
            Vec::new()
        },
    }
}

/// The tag browser's summary of a batch (specification 25 §5.4): the tags added or removed and the
/// ports that applied, or that none were.
pub(super) fn batch_summary(operation: &str, ports: &[Value], count: u64) -> String {
    let applied: Vec<String> = ports
        .iter()
        .filter(|port| applied_port(port))
        .filter_map(|port| port.get("port").and_then(Value::as_u64))
        .map(|port| port.to_string())
        .collect();
    texts::batch_summary(operation == "remove", count, &applied)
}

fn applied_port(port: &Value) -> bool {
    port.get("applied").and_then(Value::as_bool) == Some(true)
}

impl Imported {
    fn refused(message: impl Into<String>) -> Self {
        Self {
            applied: false,
            message: message.into(),
            needs_runtime: false,
            documents: Vec::new(),
        }
    }
}

/// The ADS port of `value`: a whole number from 1 to 65535.
pub(super) fn ads_port(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port >= 1)
}

/// Whether an import or removal report says it applied.
fn applied(report: &Value) -> bool {
    report.get("applied").and_then(Value::as_bool) == Some(true)
}

/// The count at `key` of a report.
fn count(report: &Value, key: &str) -> Option<usize> {
    report
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
}

/// The report's own message, when it gave one.
fn report_message(report: &Value) -> Option<String> {
    report
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
}

/// The generated document an import wrote, when its path is absolute on this computer.
fn generated_document(report: &Value) -> Vec<String> {
    report
        .get("generated_path")
        .and_then(Value::as_str)
        .filter(|path| Path::new(path).is_absolute())
        .map(|path| vec![path.to_owned()])
        .unwrap_or_default()
}

/// The trimmed, non-empty text at the first of `keys` that holds one.
fn text(value: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    })
}

/// The value at `first` when it is a text, else the value at `second`, as the live import reads the
/// target's address and AMS Net ID.
fn string_or(value: &Map<String, Value>, first: &str, second: &str) -> Option<Value> {
    value
        .get(first)
        .filter(|value| value.is_string())
        .or_else(|| value.get(second).filter(|value| !value.is_null()))
        .cloned()
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The texts of the ADS tag actions, word for word as the canvas wrote them.
mod texts {
    /// A batch without an open project.
    pub(super) const OPEN_PROJECT_TO_ADD: &str = "Open a truST project before adding ADS tags.";
    /// A removal without an open project.
    pub(super) const OPEN_PROJECT_TO_REMOVE: &str =
        "Open a truST project before removing ADS tags.";
    /// A single import without an open project.
    pub(super) const IMPORT_NEEDS_PROJECT: &str =
        "Add tags needs an open project so truST can write ads.toml and generated ST.";
    /// A single import of a browse pane that is not ADS while no runtime answers.
    pub(super) const IMPORT_NEEDS_RUNTIME: &str = "Add tags needs a reachable runtime — it writes ads.toml + the generated ST through the runtime's ADS import pipeline.";
    /// An offline import of a target without an address.
    pub(super) const IMPORT_NEEDS_HOST: &str = "ADS tag import needs a target host.";
    /// An offline import of writable tags.
    pub(super) const WRITABLE_NEEDS_RUNTIME: &str = "Write-enabled ADS imports need a running runtime so truST can apply the explicit write acknowledgement.";
    /// The title of the notice of an offline import of writable tags.
    pub(super) const WRITABLE_NEEDS_RUNTIME_TITLE: &str =
        "Write-enabled ADS imports need a running runtime.";
    /// The detail of that notice.
    pub(super) const WRITABLE_NEEDS_RUNTIME_DETAIL: &str = "truST must verify the explicit write acknowledgement before importing writable tags. Start the runtime, then import again.";
    /// An offline import without a symbol.
    pub(super) const SELECT_A_SYMBOL: &str = "Select at least one ADS symbol to import.";
    /// An offline import that failed without a reason.
    pub(super) const IMPORT_FAILED: &str = "ADS tag import failed.";
    /// A live import that the runtime refused without a reason.
    pub(super) const IMPORT_REJECTED: &str = "The runtime rejected the import.";
    /// A removal that failed without a reason.
    pub(super) const REMOVAL_FAILED: &str = "ADS tag removal failed.";

    /// `n ADS tag(s)`.
    fn tags(count: usize) -> String {
        format!("{count} ADS tag{}", if count == 1 { "" } else { "s" })
    }

    /// The message of one port's applied import in a batch.
    pub(super) fn added_tags(count: usize) -> String {
        format!("Added {}.", tags(count))
    }

    /// The notice of an applied single live import.
    pub(super) fn live_tags_added(message: &str) -> String {
        format!("{message} Restart the runtime to apply the generated ST symbols.")
    }

    /// The notice of an applied offline import.
    pub(super) fn offline_tags_added(count: usize) -> String {
        format!(
            "Added {}. Restart the runtime to use the generated ST symbols.",
            tags(count)
        )
    }

    /// A live import that failed to reach the runtime.
    pub(super) fn live_import_failed(reason: &str) -> String {
        format!("{reason} (live ADS import needs an ads-wire runtime build).")
    }

    /// A single live import that did not apply.
    pub(super) fn could_not_add_tags(reason: &str) -> String {
        format!("Could not add tags: {reason}")
    }

    /// A single offline import that did not apply.
    pub(super) fn could_not_add_ads_tags(reason: &str) -> String {
        let reason = reason.trim();
        let reason = if reason.is_empty() {
            "the import was rejected."
        } else {
            reason
        };
        format!("Could not add ADS tags: {reason}")
    }

    /// The summary of a batch: `Added 3 tags from ADS ports 301, 851.`, or that none were.
    pub(super) fn batch_summary(remove: bool, count: u64, ports: &[String]) -> String {
        if ports.is_empty() {
            return if remove {
                "No ADS tags were removed.".to_owned()
            } else {
                "No ADS tags were added.".to_owned()
            };
        }
        let verb = if remove { "Removed" } else { "Added" };
        let tags = if count == 1 { "tag" } else { "tags" };
        let port_word = if ports.len() == 1 { "port" } else { "ports" };
        format!(
            "{verb} {count} {tags} from ADS {port_word} {}.",
            ports.join(", ")
        )
    }
}
