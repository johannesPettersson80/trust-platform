//! The form and connection actions of the panel's `act` (design §4.3 B7, second part): open a
//! form, validate it, save it, and remove, disable or enable a connection. Each one resolves the
//! runtime it writes to from what the session last showed, in one function of this module, and
//! sends the matching `comm.apply`: in process with
//! [`offline_comm_apply_json`](trust_runtime::control::offline_comm_apply_json) for the open
//! project's own files, or over the control channel for a runtime that answers
//! (specification 25 §5.4, §5.9 and §5.14; decision O16).
//!
//! - `form.open`: the form that [`panes::forms`](crate::panes::forms) builds from the session's
//!   runtime, or an error naming the protocol (inventory J05).
//! - `form.validate`: `comm.apply` `validate`, whose field errors are the notice's, and nothing is
//!   written (inventory I7).
//! - `connection.save`: `add`, `upsert` or `edit`; a success names the runtime's message, selects
//!   the written connection, closes the pane and refreshes now (inventories J08, J10, J11, I41
//!   and I42).
//! - `connection.remove`, `connection.disable` and `connection.enable`: the matching action for
//!   the connection's instance ID (R4).
//!
//! Every notice names the pane that started the action (specification 25 §5.13).

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_apply_json;
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

use super::effects;
use crate::panes::forms::{
    self, coerce_submission, Catalog, CatalogProtocol, EditedConnection, FormInputs, FormRequest,
};
use crate::request::{ActRequest, Token};
use crate::session::{KnownRuntime, Session};
use crate::texts::connections as connection_texts;
use crate::texts::forms as texts;
use crate::topology;
use crate::view::types::{ActReply, FieldError, Form, ItemId, Refresh, RuntimeRef, Tone};

/// The wait for one `comm.apply` on a runtime (design §1.5: 2,000 ms for each further request).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// The action's payload did not name the protocol.
const NO_PROTOCOL: &str = "The action did not name the protocol.";
/// The action's payload did not name the connection.
const NO_CONNECTION: &str = "The action did not name the connection.";
/// A form that edits a connection whose instance ID cannot be read.
const NO_INSTANCE: &str = "The action did not name the connection to edit.";

/// Where one action works: the runtime it writes to, resolved from what the session last showed,
/// and how it is reached (design §4.3 B7; specification 25 §5.14). A runtime the session does not
/// know, and one that does not answer while it is not the open project's own runtime, is a
/// refusal: nothing is written.
struct Target<'a> {
    /// The runtime, as the last view showed it.
    runtime: &'a KnownRuntime,
    /// Where its change is applied.
    writes: Writes,
}

/// Where one `comm.apply` is applied.
enum Writes {
    /// The runtime's control endpoint, which answered.
    Endpoint(String),
    /// The open project's files, in process.
    Project,
}

impl Target<'_> {
    /// Whether the runtime answered this round, so that `Test` and `Apply to running runtime` can
    /// act on it (specification 25 §5.13).
    fn reachable(&self) -> bool {
        matches!(self.writes, Writes::Endpoint(_))
    }
}

/// The target that the payload names, resolved from what the session last showed: the project's
/// own runtime over its control endpoint when it answers, else the open project's files in
/// process; any other runtime over its control endpoint when it answers, else a refusal that
/// names it. An ID the session does not know, or a session that has shown no view yet, is a
/// refusal (design §4.3 B7; specification 25 §5.14).
fn target<'a>(session: &'a Session, request: &ActRequest) -> Result<Target<'a>, String> {
    let Some(id) = super::payload_text(&request.op.payload, "runtime") else {
        return Err(texts::NO_RUNTIME.to_owned());
    };
    let Some(runtime) = session.runtime(&id) else {
        return Err(texts::runtime_not_shown(&id));
    };
    let endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty());
    if runtime.answers
        && let Some(endpoint) = endpoint
    {
        return Ok(Target {
            runtime,
            writes: Writes::Endpoint(endpoint.to_owned()),
        });
    }
    if runtime.project {
        return Ok(Target {
            runtime,
            writes: Writes::Project,
        });
    }
    Err(texts::runtime_did_not_answer(&runtime.name))
}

/// `form.open`: the form that the payload describes, built by [`panes::forms`](crate::panes::forms)
/// from the runtime that the session last showed (design §1.4.1 and §3.2 J05; specification 25 §5.4
/// and §5.14). The form names that runtime, whether it answers, the runtimes a new connection can
/// be saved to, and the catalog it serves; an edit shows the connection's current values. A
/// protocol that the catalog does not hold answers an error naming it, never a form of another
/// protocol.
#[must_use]
pub(crate) fn open(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let form_request: FormRequest = match serde_json::from_value(request.op.payload.clone()) {
        Ok(value) => value,
        Err(error) => {
            return super::error(&pane, &format!("The form request did not decode: {error}"));
        }
    };
    if let Some(setup) = form_request.setup {
        return form_reply(forms::setup_form(setup));
    }
    let target = match target(session, request) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let catalog = match catalog_of(target.runtime) {
        Ok(catalog) => catalog,
        Err(message) => return super::error(&pane, &message),
    };
    if catalog.protocol(&protocol).is_none() {
        return super::error(&pane, &no_schema(&protocol));
    }
    let runtime = runtime_ref(target.runtime);
    let choices: Vec<RuntimeRef> = session
        .runtimes()
        .iter()
        .filter(|candidate| candidate.accepts_connections())
        .map(runtime_ref)
        .collect();
    let edited = form_request
        .connection
        .as_deref()
        .and_then(|id| target.runtime.connection(id))
        .map(|connection| EditedConnection {
            id: connection.id.clone(),
            protocol_name: connection_texts::protocol_name(&connection.protocol),
            params: connection.params.clone(),
            disabled: connection.disabled,
        });
    let inputs = FormInputs {
        catalog,
        runtime: &runtime,
        runtime_choices: &choices,
        reachable: target.reachable(),
        prefill: form_request.prefill.as_ref(),
        connection: edited.as_ref(),
    };
    match forms::connection_form(&protocol, &inputs) {
        Ok(form) => form_reply(form),
        Err(message) => super::error(&pane, &message),
    }
}

/// `form.validate`: converts the values with [`coerce_submission`] and has the target check them
/// with `comm.apply` `validate`, which writes nothing (design §1.4.1, §3.3 I7). The refused fields
/// are the notice's own.
#[must_use]
pub(crate) fn validate(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let target = match target(session, request) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let schema = match schema_of(target.runtime, &protocol) {
        Ok(schema) => schema,
        Err(message) => return super::error(&pane, &message),
    };
    let params = match coerce_submission(&schema, &request.values) {
        Ok(params) => params,
        Err(errors) => return field_errors(&pane, errors),
    };
    let instance_id = instance_id(target.runtime, request);
    match send(
        request,
        &target,
        apply_request(&protocol, "validate", instance_id.as_deref(), &params),
    ) {
        Sent::Applied(answer) => super::reply(
            true,
            Some(super::notice(Tone::Ok, &pane, reply_message(&answer))),
            Vec::new(),
            false,
            Refresh::None,
        ),
        Sent::Fields(errors) => field_errors(&pane, errors),
        Sent::Refused(message) => failure(&pane, message),
    }
}

/// `connection.save`: converts the values, sends `comm.apply` `add`, `upsert` or `edit`, and on
/// success selects the written connection without a second read of the topology (design §4.3 B7;
/// inventories J08, J10, J11, I41 and I42; specification 25 §5.4).
#[must_use]
pub(crate) fn save(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let target = match target(session, request) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let schema = match schema_of(target.runtime, &protocol) {
        Ok(schema) => schema,
        Err(message) => return super::error(&pane, &message),
    };
    let params = match coerce_submission(&schema, &request.values) {
        Ok(params) => params,
        Err(errors) => return field_errors(&pane, errors),
    };
    let edited = super::payload_text(&request.op.payload, "connection");
    let action = if edited.is_some() {
        "edit"
    } else if schema.allows("add") {
        "add"
    } else {
        "upsert"
    };
    let instance_id = instance_id(target.runtime, request);
    if action == "edit" && instance_id.is_none() {
        return super::error(&pane, NO_INSTANCE);
    }
    match send(
        request,
        &target,
        apply_request(&protocol, action, instance_id.as_deref(), &params),
    ) {
        Sent::Applied(answer) => {
            let effects = written_connection(request, &answer)
                .map_or_else(Vec::new, |item| vec![effects::select(item)]);
            super::reply(
                true,
                Some(super::notice(Tone::Ok, &pane, reply_message(&answer))),
                effects,
                true,
                Refresh::Now,
            )
        }
        Sent::Fields(errors) => field_errors(&pane, errors),
        Sent::Refused(message) => failure(&pane, message),
    }
}

/// `connection.remove`: sends `comm.apply` `remove` for the connection (design §4.3 B7, R4).
#[must_use]
pub(crate) fn remove(session: &Session, request: &ActRequest) -> ActReply {
    change(session, request, "remove", false)
}

/// `connection.disable`: sends `comm.apply` `disable` for the connection (design §4.3 B7, R4).
#[must_use]
pub(crate) fn disable(session: &Session, request: &ActRequest) -> ActReply {
    change(session, request, "disable", false)
}

/// `connection.enable`: writes the connection enabled again with the submitted values, as the
/// panel's `Enable` does (design §4.3 B7, R4).
#[must_use]
pub(crate) fn enable(session: &Session, request: &ActRequest) -> ActReply {
    change(session, request, "edit", true)
}

/// One of `remove`, `disable` and `enable`: the matching `comm.apply` for the connection's
/// instance ID. `with_values` writes the converted values, which `enable` needs and the others do
/// not.
fn change(session: &Session, request: &ActRequest, action: &str, with_values: bool) -> ActReply {
    let pane = super::pane(request);
    let target = match target(session, request) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let Some(instance_id) = instance_id(target.runtime, request) else {
        return super::error(&pane, NO_CONNECTION);
    };
    let params = if with_values {
        let Ok(schema) = schema_of(target.runtime, &protocol) else {
            return super::error(&pane, &no_schema(&protocol));
        };
        match coerce_submission(&schema, &request.values) {
            Ok(params) => params,
            Err(errors) => return field_errors(&pane, errors),
        }
    } else {
        Map::new()
    };
    match send(
        request,
        &target,
        apply_request(&protocol, action, Some(&instance_id), &params),
    ) {
        Sent::Applied(answer) => super::reply(
            true,
            Some(super::notice(Tone::Ok, &pane, reply_message(&answer))),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Sent::Fields(errors) => field_errors(&pane, errors),
        Sent::Refused(message) => failure(&pane, message),
    }
}

/// What one `comm.apply` answered.
enum Sent {
    /// It was applied; the target's answer.
    Applied(Value),
    /// The fields were refused, and nothing was applied.
    Fields(Vec<FieldError>),
    /// The target refused the action, or it could not be reached.
    Refused(String),
}

/// Sends one `comm.apply` to the action's target.
fn send(request: &ActRequest, target: &Target<'_>, params: Value) -> Sent {
    match &target.writes {
        Writes::Endpoint(endpoint) => send_to_runtime(request, endpoint, params),
        Writes::Project => send_to_project(request.project.as_deref(), params),
    }
}

/// Applies `params` to the open project's files, where no runtime is needed.
fn send_to_project(project: Option<&Path>, params: Value) -> Sent {
    let Some(project) = project else {
        return Sent::Refused(texts::NO_PROJECT_FOLDER.to_owned());
    };
    match offline_comm_apply_json(project, params) {
        Ok(answer) => classify(answer),
        Err(message) => Sent::Refused(message),
    }
}

/// Applies `params` to `endpoint` over the control channel, with the token the extension stored
/// for it. Secrets on a plain channel are refused by the runtime itself (decision O16).
fn send_to_runtime(request: &ActRequest, endpoint: &str, params: Value) -> Sent {
    let Some(address) = ControlEndpoint::parse(endpoint) else {
        return Sent::Refused(format!("{endpoint} is not a control endpoint."));
    };
    let token = request.host.token(endpoint).map(Token::secret);
    let mut client = match ControlClient::connect(&address, token, REQUEST_TIMEOUT) {
        Ok(client) => client.with_response_limit(RESPONSE_LINE_LIMIT),
        Err(error) => return Sent::Refused(error.to_string()),
    };
    if let Err(error) = client.set_timeout(REQUEST_TIMEOUT) {
        return Sent::Refused(error.to_string());
    }
    match client.request("comm.apply", Some(params)) {
        Ok(answer) => classify(answer),
        Err(ControlError::Refused { error, .. }) => Sent::Refused(error),
        Err(error) => Sent::Refused(error.to_string()),
    }
}

/// Reads a `comm.apply` answer: its `field_errors` are refusals that nothing applied, else the
/// answer is an applied change.
fn classify(answer: Value) -> Sent {
    let errors: Vec<FieldError> = answer
        .get("field_errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|error| {
            Some(FieldError {
                field: error.get("field")?.as_str()?.to_owned(),
                message: error.get("message")?.as_str()?.to_owned(),
            })
        })
        .collect();
    if errors.is_empty() {
        Sent::Applied(answer)
    } else {
        Sent::Fields(errors)
    }
}

/// One `comm.apply` payload, as the runtime and the offline applier read it.
fn apply_request(
    protocol: &str,
    action: &str,
    instance_id: Option<&str>,
    params: &Map<String, Value>,
) -> Value {
    let mut request = json!({ "protocol": protocol, "action": action, "params": params });
    if let Some(instance_id) = instance_id {
        request["instance_id"] = Value::String(instance_id.to_owned());
    }
    request
}

/// The catalog that the runtime serves, else the tool's own (specification 25 §5.14); it is what
/// the view's `comm.schema` fallback and the add dialog read. `None` when neither is available.
fn catalog_of(runtime: &KnownRuntime) -> Result<&Catalog, String> {
    runtime
        .catalog
        .as_ref()
        .ok_or_else(|| texts::DEVICE_CATALOG_UNAVAILABLE.to_owned())
}

/// The catalog's protocol `protocol`, or why the catalog does not hold it.
fn schema_of(runtime: &KnownRuntime, protocol: &str) -> Result<CatalogProtocol, String> {
    catalog_of(runtime)?
        .protocol(protocol)
        .cloned()
        .ok_or_else(|| no_schema(protocol))
}

/// Why no form or coercion exists for `protocol`.
fn no_schema(protocol: &str) -> String {
    format!("The catalog does not hold the protocol '{protocol}'.")
}

/// The payload's protocol, if it named one.
fn protocol_of(request: &ActRequest) -> Option<String> {
    super::payload_text(&request.op.payload, "protocol")
}

/// The `instance_id` of the connection the payload names: the session's own entry for that
/// connection, else the item ID it names (design §4.3 B7, R4). `None` when it named none.
fn instance_id(runtime: &KnownRuntime, request: &ActRequest) -> Option<String> {
    let connection = super::payload_text(&request.op.payload, "connection")?;
    runtime
        .connection(&connection)
        .and_then(|known| known.instance_id.clone())
        .or_else(|| topology::connection_instance_id(&connection))
}

/// The item ID of the connection that `answer` wrote, from the request's runtime and the
/// `instance_id` the answer carries (design §4.3 B7, I41 and I42). `None` when the answer names
/// none, which selects nothing (specification 25 §5.4).
fn written_connection(request: &ActRequest, answer: &Value) -> Option<ItemId> {
    let runtime = super::payload_text(&request.op.payload, "runtime")?;
    let instance_id = super::payload_text(answer, "instance_id")?;
    topology::connection_id(&runtime, &instance_id).map(ItemId)
}

/// The runtime as the form names and picks it.
fn runtime_ref(runtime: &KnownRuntime) -> RuntimeRef {
    RuntimeRef {
        id: runtime.id.clone(),
        name: runtime.name.clone(),
    }
}

/// The runtime's message, which a successful action shows.
fn reply_message(answer: &Value) -> String {
    super::payload_text(answer, "message").unwrap_or_default()
}

/// The reply of `form.open`: the form and nothing else.
fn form_reply(form: Form) -> ActReply {
    ActReply {
        ok: true,
        notice: None,
        effects: Vec::new(),
        close_pane: false,
        refresh: Refresh::None,
        pending: None,
        form: Some(form),
    }
}

/// A reply whose fields were refused, shown beside those fields, with nothing applied.
fn field_errors(pane: &str, errors: Vec<FieldError>) -> ActReply {
    let text = errors
        .first()
        .map_or_else(String::new, |error| error.message.clone());
    let mut notice = super::notice(Tone::Error, pane, text);
    notice.field_errors = errors;
    super::reply(false, Some(notice), Vec::new(), false, Refresh::None)
}

/// A reply that failed for a reason that is not a field error.
fn failure(pane: &str, message: String) -> ActReply {
    super::reply(
        false,
        Some(super::notice(Tone::Error, pane, message)),
        Vec::new(),
        false,
        Refresh::None,
    )
}
