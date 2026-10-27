//! A setup form of the canvas applied or tested at the runtime that serves its live data, or saved
//! into the open project's files (specification 25 §5.4, §5.9 and §5.10).
//!
//! The form's values are converted here by their fields' schema types, and a field whose
//! `visible_when` predicate does not hold is left out (`panes::forms::coerce_submission`). The
//! runtime's own `comm.apply` and `comm.test` check the converted values; a secret is never sent
//! over plain TCP to another computer (`actions::write::secret_refusal`); and a failed OPC UA
//! client test answers the server's recovery text.

use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::act::{CanvasActRequest, CanvasReply};
use crate::actions::config::test_message;
use crate::actions::write;
use crate::panes::forms::{coerce_submission, Catalog};
use crate::texts::forms as texts;
use crate::topology::{self, SIMULATOR_FALLBACK_ID};
use crate::view::types::FieldError;

/// The wait for the runtime's answer to `comm.apply` and `comm.test`.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(4);

/// The arguments of `comm_apply`, `comm_test` and `comm_save`: the form's protocol, its action, the
/// configured instance it edits, and either the values typed into the form with the catalog that
/// describes them, or parameters that already have their types (a connection's saved
/// configuration).
#[derive(Debug, Deserialize)]
pub(super) struct FormArgs {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    instance_id: Option<String>,
    /// The values typed into the form, by field ID, as the form holds them.
    #[serde(default)]
    values: Option<Map<String, Value>>,
    /// Parameters that already have their types.
    #[serde(default)]
    params: Value,
    /// The `comm.schema` answer that the canvas shows its forms from.
    #[serde(default)]
    schema: Option<Value>,
    /// The canvas node of the runtime that the form saves to.
    #[serde(default)]
    runtime_id: Option<String>,
}

/// `comm_apply`: sends the form's `comm.apply` to the runtime that serves the live data and
/// answers the runtime's result; a form without a protocol does nothing.
pub(super) fn apply(request: &CanvasActRequest, args: &FormArgs) -> CanvasReply {
    let Some(protocol) = protocol_id(&args.protocol) else {
        return CanvasReply::nothing();
    };
    let Some(runtime) = request.live() else {
        return answered(blocked(&protocol, texts::APPLY_NEEDS_REACHABLE));
    };
    let params = match form_params(&protocol, args) {
        Ok(params) => params,
        Err(refused) => return answered(refused),
    };
    let mut payload = json!({
        "protocol": protocol,
        "action": apply_action(&args.action, &protocol, args.schema.as_ref()),
        "params": params,
    });
    if let Some(instance_id) = args
        .instance_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        payload["instance_id"] = Value::from(instance_id);
    }
    if let Some(field) = write::secret_refusal(&runtime.endpoint, &payload["params"]) {
        return answered(secret_blocked(&protocol, &field));
    }
    let result = write::send_with_token(
        &runtime.endpoint,
        runtime.token.as_deref(),
        "comm.apply",
        &payload,
        REQUEST_TIMEOUT,
    )
    .unwrap_or_else(|reason| blocked(&protocol, &reason));
    answered(result)
}

/// `comm_test`: sends the form's `comm.test` to the runtime that serves the live data and answers
/// the test's result: `test_ok` with the runtime's detail, else `blocked` with the detail or, for
/// a failed OPC UA client test, the server's recovery text (specification 25 §5.10). An OPC UA
/// client test tests the first of its connections, whose parameters `comm.test` reads.
pub(super) fn test(request: &CanvasActRequest, args: &FormArgs) -> CanvasReply {
    let Some(protocol) = protocol_id(&args.protocol) else {
        return CanvasReply::nothing();
    };
    let Some(runtime) = request.live() else {
        return answered(blocked(&protocol, texts::TEST_NEEDS_REACHABLE));
    };
    let params = match form_params(&protocol, args) {
        Ok(params) => tested_params(&protocol, params),
        Err(refused) => return answered(refused),
    };
    let payload = json!({ "protocol": protocol, "params": params });
    if let Some(field) = write::secret_refusal(&runtime.endpoint, &payload["params"]) {
        return answered(secret_blocked(&protocol, &field));
    }
    let result = match write::send_with_token(
        &runtime.endpoint,
        runtime.token.as_deref(),
        "comm.test",
        &payload,
        REQUEST_TIMEOUT,
    ) {
        Ok(answer) => {
            let ok = answer.get("ok").and_then(Value::as_bool) == Some(true);
            json!({
                "schema_version": 1,
                "protocol": protocol,
                "driver": "",
                "action": "test",
                "applied": false,
                "lifecycle_effect": if ok { "test_ok" } else { "blocked" },
                "message": test_message(&protocol, &answer),
                "field_errors": answer
                    .get("field_errors")
                    .filter(|errors| errors.is_array())
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            })
        }
        Err(reason) => blocked(&protocol, &reason),
    };
    answered(result)
}

/// `comm_save`: writes the form into the open project's files with `comm.apply` (`upsert`,
/// `remove` or `disable`), which works while no runtime runs, and answers the result with the ID of
/// the connection it wrote and, as `node_id`, the canvas node that draws it under the runtime the
/// form saves to, which the canvas selects (specification 25 §5.3, §5.4 and §5.9). Without an open
/// project or a protocol it does nothing.
pub(super) fn save(request: &CanvasActRequest, args: &FormArgs) -> CanvasReply {
    let Some(protocol) = protocol_id(&args.protocol) else {
        return CanvasReply::nothing();
    };
    let Some(project) = request.project() else {
        return CanvasReply::nothing();
    };
    let params = match form_params(&protocol, args) {
        Ok(params) => params,
        Err(refused) => return answered(refused),
    };
    let action = save_action(&args.action);
    let payload = json!({ "protocol": protocol, "action": action, "params": params });
    let mut result = match trust_runtime::control::offline_comm_apply_json(project, payload) {
        Ok(answer) => answer,
        Err(reason) => blocked(&protocol, &reason),
    };
    let applied = result.get("applied").and_then(Value::as_bool) == Some(true);
    let named = result
        .get("instance_id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.trim().is_empty());
    if applied
        && action != "remove"
        && !named
        && let Some(id) = saved_connection(project, &protocol, &params)
    {
        result["instance_id"] = Value::String(id);
    }
    if applied
        && action != "remove"
        && let Some(node) = result
            .get("instance_id")
            .and_then(Value::as_str)
            .zip(args.runtime_id.as_deref())
            .and_then(|(instance, runtime)| saved_node(runtime, instance))
    {
        result["node_id"] = Value::String(node);
    }
    CanvasReply {
        applied,
        refresh: true,
        result: Some(result),
        ..CanvasReply::nothing()
    }
}

/// The canvas node that draws the connection `instance` (`{protocol}:{index}`, specification 11
/// §6.8.1a) of the runtime node `runtime`: the connection's owner-scoped identity under a runtime
/// that a report or the configuration gives (`topology::connection_id`), and the catalog's own ID
/// on the drawn simulator, whose connections the catalog names. `None` for any other node.
pub(super) fn saved_node(runtime: &str, instance: &str) -> Option<String> {
    if runtime == SIMULATOR_FALLBACK_ID {
        return Some(instance.to_owned());
    }
    topology::connection_id(runtime, instance)
}

/// The form's parameters: its typed values converted by the catalog's description of `protocol`
/// (specification 25 §5.9), else the parameters that already have their types. A value that does
/// not fit its field, or a form whose catalog does not describe its protocol, is the blocked
/// result that names why.
fn form_params(protocol: &str, args: &FormArgs) -> Result<Map<String, Value>, Value> {
    let Some(values) = &args.values else {
        return Ok(match &args.params {
            Value::Object(params) => params.clone(),
            _ => Map::new(),
        });
    };
    let Some(schema) = args
        .schema
        .as_ref()
        .and_then(Catalog::parse)
        .and_then(|catalog| catalog.protocol(protocol).cloned())
    else {
        return Err(blocked(protocol, texts::NO_NATIVE_SCHEMA));
    };
    coerce_submission(&schema, values).map_err(|errors| fields_blocked(protocol, &errors))
}

/// The parameters that `comm.test` tests: an OPC UA client's first connection when it holds a list
/// of them, else the parameters themselves.
pub(super) fn tested_params(protocol: &str, params: Map<String, Value>) -> Map<String, Value> {
    if protocol != "opcua_client" {
        return params;
    }
    match params
        .get("connections")
        .and_then(Value::as_array)
        .and_then(|connections| connections.first())
        .and_then(Value::as_object)
    {
        Some(first) => first.clone(),
        None => params,
    }
}

/// The project-local ID (`{protocol}:{index}`) of the connection that a save wrote when the
/// runtime's answer names none (specification 25 §5.4): the one connection of `protocol` in the
/// project's configuration that [`unambiguous`] names.
fn saved_connection(project: &Path, protocol: &str, params: &Map<String, Value>) -> Option<String> {
    let topology = trust_runtime::control::offline_fleet_topology_json(project).ok()?;
    let raw = unambiguous(&endpoints_of(&topology, protocol), params)?;
    instance_of(&topology, &raw)
}

/// The project-local ID of the connection whose raw endpoint ID in `topology` is `raw`: the raw ID
/// after `endpoint:<runtime ID>:`, with `:0` for the first connection of its protocol, which the
/// raw ID does not number (specification 11, "Offline communication operation and fleet-topology
/// truth").
pub(super) fn instance_of(topology: &Value, raw: &str) -> Option<String> {
    let list = |value: &Value, key: &str| -> Vec<Value> {
        value
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    list(topology, "hosts")
        .iter()
        .flat_map(|host| {
            let mut runtimes = list(host, "runtimes");
            for container in list(host, "containers") {
                runtimes.extend(list(&container, "runtimes"));
            }
            runtimes
        })
        .find_map(|runtime| {
            let runtime_id = runtime.get("runtime_id")?.as_str()?.to_owned();
            let key = raw
                .strip_prefix("endpoint:")?
                .strip_prefix(runtime_id.as_str())?
                .strip_prefix(':')?;
            Some(if key.contains(':') {
                key.to_owned()
            } else {
                format!("{key}:0")
            })
        })
}

/// The connections of `protocol` in `topology` (a `fleet.topology` report): on each computer, the
/// runtimes outside containers, then those in its containers, each connection in its order.
pub(super) fn endpoints_of<'a>(topology: &'a Value, protocol: &str) -> Vec<&'a Map<String, Value>> {
    let list = |value: &'a Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
    };
    list(topology, "hosts")
        .flat_map(|host| {
            list(host, "runtimes").chain(
                list(host, "containers").flat_map(move |container| list(container, "runtimes")),
            )
        })
        .flat_map(|runtime| list(runtime, "endpoints"))
        .filter_map(Value::as_object)
        .filter(|endpoint| endpoint.get("protocol").and_then(Value::as_str) == Some(protocol))
        .collect()
}

/// The ID of the connection among `endpoints` that a save of `params` wrote, only when that is
/// unambiguous (specification 25 §5.4): the one connection whose saved parameters agree with every
/// submitted value they hold, else the only connection. More than one candidate names none.
pub(super) fn unambiguous(
    endpoints: &[&Map<String, Value>],
    params: &Map<String, Value>,
) -> Option<String> {
    let matching: Vec<&Map<String, Value>> = endpoints
        .iter()
        .copied()
        .filter(|endpoint| {
            endpoint
                .get("params")
                .and_then(Value::as_object)
                .is_some_and(|saved| agrees(saved, params))
        })
        .collect();
    let only = match (matching.as_slice(), endpoints) {
        ([one], _) | ([], [one]) => *one,
        _ => return None,
    };
    only.get("id").and_then(Value::as_str).map(str::to_owned)
}

/// Whether every submitted value that the saved parameters hold equals the saved one.
fn agrees(saved: &Map<String, Value>, submitted: &Map<String, Value>) -> bool {
    submitted
        .iter()
        .all(|(key, value)| saved.get(key).is_none_or(|known| known == value))
}

/// The protocol ID a form names: trimmed, `-` as `_`, in lower case; `None` when blank.
fn protocol_id(protocol: &str) -> Option<String> {
    let id = protocol.trim().replace('-', "_").to_lowercase();
    (!id.is_empty()).then_some(id)
}

/// The action that applies a form: the one it names, trimmed and in lower case; without one, `add`
/// for a protocol of several instances that can be added to (the add form), else `upsert`.
pub(super) fn apply_action(action: &str, protocol: &str, schema: Option<&Value>) -> String {
    let action = action.trim().to_lowercase();
    if !action.is_empty() {
        return action;
    }
    let described = schema
        .and_then(|schema| schema.get("protocols"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(protocol));
    let several = described
        .and_then(|entry| entry.get("supports_multi_instance"))
        .and_then(Value::as_bool)
        == Some(true);
    let adds = described
        .and_then(|entry| entry.get("actions"))
        .and_then(Value::as_array)
        .is_some_and(|actions| actions.iter().any(|action| action == "add"));
    let action = if several && adds { "add" } else { "upsert" };
    action.to_owned()
}

/// The action of a save: `remove` and `disable` as named, any other `upsert`.
fn save_action(action: &str) -> &'static str {
    match action.trim().to_lowercase().as_str() {
        "remove" => "remove",
        "disable" => "disable",
        _ => "upsert",
    }
}

/// A form result that applied nothing, with `message`.
fn blocked(protocol: &str, message: &str) -> Value {
    json!({
        "schema_version": 1,
        "protocol": protocol,
        "driver": "",
        "action": "upsert",
        "applied": false,
        "lifecycle_effect": "blocked",
        "message": message,
        "field_errors": [],
    })
}

/// The result of a form whose values do not fit their fields: each refused field with its reason.
fn fields_blocked(protocol: &str, errors: &[FieldError]) -> Value {
    let mut result = blocked(protocol, texts::FIX_HIGHLIGHTED_FIELDS);
    result["field_errors"] = Value::Array(
        errors
            .iter()
            .map(|error| json!({ "field": error.field, "message": error.message }))
            .collect(),
    );
    result
}

/// The result of a form whose secret `field` was not sent (specification 25 §5.9).
fn secret_blocked(protocol: &str, field: &str) -> Value {
    write::secret_blocked_answer(&json!({ "protocol": protocol, "action": "upsert" }), field)
}

/// The reply that hands `result` to the form and reads the data again.
fn answered(result: Value) -> CanvasReply {
    CanvasReply {
        applied: result.get("applied").and_then(Value::as_bool) == Some(true),
        refresh: true,
        result: Some(result),
        ..CanvasReply::nothing()
    }
}
