//! The runtimes this computer manages under a fleet root (specification 25 §5.1 and §6.5): their
//! states as their fleet status names them, whether a Start or a Stop reached its state, and their
//! logs as readable event summaries.
//!
//! - `managed` `{ fleetRoot }`: `{ ok: true, runtimes: [{ name, label, controlEndpoint, state,
//!   projectPath, logPath }] }`, each runtime of the fleet root's `fleet.toml` with the state its
//!   status reports: `running`, `stopped`, `starting` or `stopping`, and `unavailable` when its status
//!   cannot be read, which is never shown as stopped. A fleet root without a readable manifest has
//!   none, and `problem` says why.
//! - `managedOutcome` `{ lifecycle: "start" | "stop", name, fleetRoot, response }`: what the JSON
//!   answer of `trust-runtime fleet runtime start|stop --json` for the runtime `name` reached:
//!   `{ ok: true, success, status, message, controlEndpoint, projectPath, label }`, `label` the
//!   runtime's name in the extension. A Start succeeds only when the runtime is `running` and a Stop
//!   only when it is `stopped`; `starting`, `stopping` and every other state are no success
//!   (specification 25 §5.1).
//! - `managedLogs` `{ name, stdout, stderr }`: the text of `trust-runtime fleet runtime logs`, one
//!   readable line per record (specification 25 §6.5).
//! - `managedStopEndsSession` `{ stoppedEndpoint, stoppedLabel, attached: { mode, state, endpoint,
//!   label } }`: `{ ok: true, ends }`, whether the Stop of a managed runtime ends the extension's
//!   attached session: it does when the session is online and connected to that runtime, by its
//!   control endpoint or by its name (specification 25 §5.1).

use std::path::{Path, PathBuf};

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::{json, Value};
use trust_devices::actions::{lifecycle_reached, managed_label};
use trust_devices::topology::ManagedState;
use trust_runtime::config::fleet_manifest::{
    load_manifest, managed_status, runtime_project_path, FleetManifestRuntime, FLEET_MANIFEST_FILE,
};

use super::{decode, non_blank};

/// The message when `trust-runtime` gave no JSON answer.
const NO_ANSWER: &str = "The runtime tools didn't respond.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListArgs {
    fleet_root: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutcomeArgs {
    lifecycle: String,
    name: String,
    #[serde(default)]
    fleet_root: Option<PathBuf>,
    #[serde(default)]
    response: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopArgs {
    #[serde(default)]
    stopped_endpoint: Option<String>,
    #[serde(default)]
    stopped_label: Option<String>,
    attached: AttachedSession,
}

/// The extension's attached session as its status shows it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachedSession {
    mode: String,
    state: String,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogArgs {
    name: String,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
}

/// The answer of the managed-runtime action `action`.
pub(super) fn answer(action: &str, argument: Value) -> Result<Value, String> {
    match action {
        "managed" => {
            let args: ListArgs = decode(argument)?;
            Ok(list(&args.fleet_root))
        }
        "managedOutcome" => {
            let args: OutcomeArgs = decode(argument)?;
            let start = match args.lifecycle.as_str() {
                "start" => true,
                "stop" => false,
                other => return Err(format!("{other} is no managed lifecycle.")),
            };
            let mut answer = outcome(start, args.fleet_root.as_deref(), args.response.as_ref());
            answer["label"] = json!(managed_label(&args.name));
            Ok(answer)
        }
        "managedStopEndsSession" => {
            let args: StopArgs = decode(argument)?;
            Ok(json!({ "ok": true, "ends": stop_ends_session(&args) }))
        }
        _ => {
            let args: LogArgs = decode(argument)?;
            Ok(json!({ "ok": true, "text": log_text(&args.stdout, &args.stderr, &args.name) }))
        }
    }
}

/// Whether the Stop of a managed runtime ends the attached session: an online session connected to
/// it, by the same control endpoint or by the same name (specification 25 §5.1).
fn stop_ends_session(args: &StopArgs) -> bool {
    let attached = &args.attached;
    let same = |stopped: &Option<String>, current: &Option<String>| {
        non_blank(stopped.as_deref()).is_some_and(|stopped| {
            non_blank(current.as_deref()).as_deref() == Some(stopped.as_str())
        })
    };
    attached.mode == "online"
        && attached.state == "connected"
        && (same(&args.stopped_endpoint, &attached.endpoint)
            || same(&args.stopped_label, &attached.label))
}

/// The managed runtimes of `fleet_root`, their statuses read side by side.
fn list(fleet_root: &Path) -> Value {
    let manifest = match load_manifest(&fleet_root.join(FLEET_MANIFEST_FILE)) {
        Ok(manifest) => manifest,
        Err(error) => {
            return json!({ "ok": true, "runtimes": [], "problem": format!("{error:#}") });
        }
    };
    let runtimes: Vec<Value> = std::thread::scope(|scope| {
        let asks: Vec<_> = manifest
            .runtime
            .iter()
            .filter(|runtime| !runtime.name.trim().is_empty())
            .map(|runtime| scope.spawn(move || runtime_json(fleet_root, runtime)))
            .collect();
        asks.into_iter().filter_map(|ask| ask.join().ok()).collect()
    });
    json!({ "ok": true, "runtimes": runtimes })
}

/// One managed runtime with the state its status reports.
fn runtime_json(fleet_root: &Path, runtime: &FleetManifestRuntime) -> Value {
    let status = managed_status(fleet_root, runtime).ok();
    let state = ManagedState::from_status(status.as_ref().map(|status| status.status.as_str()));
    let control_endpoint = status
        .as_ref()
        .map_or(runtime.control_endpoint.as_str(), |status| {
            status.control_endpoint.as_str()
        });
    let project = status
        .as_ref()
        .map_or(runtime.path.as_str(), |status| status.path.as_str());
    json!({
        "name": runtime.name,
        "label": managed_label(&runtime.name),
        "controlEndpoint": control_endpoint,
        "state": state_name(state),
        "projectPath": project_path(fleet_root, project),
        "logPath": status.as_ref().and_then(|status| non_blank(Some(&status.log_path))),
    })
}

/// The name of a managed state as the extension shows it.
fn state_name(state: ManagedState) -> &'static str {
    match state {
        ManagedState::Running => "running",
        ManagedState::Stopped => "stopped",
        ManagedState::Starting => "starting",
        ManagedState::Stopping => "stopping",
        ManagedState::Unavailable => "unavailable",
    }
}

/// The project folder `path` of a runtime, relative to `fleet_root` unless absolute; `None` when
/// blank.
fn project_path(fleet_root: &Path, path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    Some(runtime_project_path(fleet_root, path).display().to_string())
}

/// What a fleet lifecycle answer `response` reached.
fn outcome(start: bool, fleet_root: Option<&Path>, response: Option<&Value>) -> Value {
    let Some(response) = response.filter(|response| response.is_object()) else {
        return json!({ "ok": true, "success": false, "message": NO_ANSWER });
    };
    let text = |key: &str| non_blank(response.get(key).and_then(Value::as_str));
    let status = text("status");
    let project = text("path").and_then(|path| match fleet_root {
        Some(root) => project_path(root, &path),
        None => Some(path),
    });
    json!({
        "ok": true,
        "success": status.as_deref().is_some_and(|status| lifecycle_reached(start, status)),
        "status": status,
        "message": text("message"),
        "controlEndpoint": text("control_endpoint"),
        "projectPath": project,
    })
}

/// The logs `stdout`, else `stderr`, of the runtime `name`, one readable line per record: a JSON
/// record as `[level] event key=value …`, any other line as it is.
fn log_text(stdout: &str, stderr: &str, name: &str) -> String {
    let text = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    if text.trim().is_empty() {
        return format!("No logs available for {name}.\n");
    }
    // Each line, a last empty one included, as the runtime's log ends its records with a line end.
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| log_line(line.strip_suffix('\r').unwrap_or(line)))
        .collect();
    format!("{}\n", lines.join("\n"))
}

/// One log line: a JSON record as `[level] event key=value …`, the details from its `data` object
/// when it has one, else from the record, `level`, `event` and `ts` left out; any other line
/// trimmed.
fn log_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Ok(Fields(record)) = serde_json::from_str::<Fields>(trimmed) else {
        return trimmed.to_owned();
    };
    let field = |fields: &[(String, Field)], key: &str| {
        fields
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| match value {
                Field::Other(Value::String(text)) => non_blank(Some(text)),
                _ => None,
            })
    };
    let data = record.iter().find_map(|(name, value)| match value {
        Field::Object(Fields(data)) if name == "data" => Some(data.as_slice()),
        _ => None,
    });
    let source = data.unwrap_or(record.as_slice());
    let level = field(source, "level")
        .or_else(|| field(&record, "level"))
        .unwrap_or_else(|| "info".to_owned());
    let event = field(source, "event")
        .or_else(|| field(&record, "event"))
        .unwrap_or_else(|| "runtime".to_owned());
    let details: Vec<String> = source
        .iter()
        .filter(|(name, _)| !matches!(name.as_str(), "level" | "event" | "ts"))
        .map(|(name, value)| format!("{name}={}", value_text(value)))
        .collect();
    if details.is_empty() {
        format!("[{level}] {event}")
    } else {
        format!("[{level}] {event} {}", details.join(" "))
    }
}

/// A detail's value: a text as it is, a number or a truth value as written, `null`, else its JSON.
fn value_text(value: &Field) -> String {
    match value {
        Field::Other(Value::String(text)) => text.clone(),
        Field::Other(Value::Number(number)) => number.to_string(),
        Field::Other(Value::Bool(flag)) => flag.to_string(),
        Field::Other(Value::Null) => "null".to_owned(),
        Field::Other(other) => other.to_string(),
        Field::Object(Fields(fields)) => {
            let members: Vec<String> = fields
                .iter()
                .map(|(name, value)| {
                    let value = match value {
                        Field::Other(value) => value.to_string(),
                        Field::Object(_) => value_text(value),
                    };
                    format!("{}:{value}", Value::String(name.clone()))
                })
                .collect();
            format!("{{{}}}", members.join(","))
        }
    }
}

/// The members of a JSON object in the order the record writes them.
#[derive(Debug)]
struct Fields(Vec<(String, Field)>);

/// A member's value: an object, its members in their order, or any other value.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Field {
    Object(Fields),
    Other(Value),
}

impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> Visitor<'de> for Members {
            type Value = Fields;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Fields, A::Error> {
                let mut fields = Vec::new();
                while let Some((name, value)) = map.next_entry::<String, Field>()? {
                    fields.push((name, value));
                }
                Ok(Fields(fields))
            }
        }
        deserializer.deserialize_map(Members)
    }
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
