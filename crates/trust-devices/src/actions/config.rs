//! The form and connection actions of the panel's `act` (design §4.3 B7, second part): open a
//! form, validate it, save it, and remove, disable or enable a connection. Each one writes through
//! the one write path of `write`, which resolves the runtime it writes to from what the session
//! last showed and sends the matching `comm.apply`, so that this module and `plans` cannot drift
//! apart (specification 25 §5.4, §5.9 and §5.14; decision O16).
//!
//! - `form.open`: the form that [`panes::forms`](crate::panes::forms) builds from the session's
//!   runtime, or an error naming the protocol (inventory J05).
//! - `form.validate`: `comm.apply` `validate`, whose field errors are the notice's, and nothing is
//!   written (inventory I7).
//! - `connection.save`: `add`, `upsert` or `edit`; a success names the runtime's message, selects
//!   the written connection, closes the pane and refreshes now (inventories J08, J10, J11, I41
//!   and I42).
//! - `connection.test` and `connection.apply_live`: `comm.test` and `comm.apply` `upsert` on the
//!   runtime the form names, answered in the form's pane; both need a runtime that answers, and
//!   say why the form's button is disabled when it does not (inventory I79).
//! - `connection.diagnose`: the connection's diagnosis, which this panel cannot run yet.
//! - `connection.remove`, `connection.disable` and `connection.enable`: the matching action for
//!   the connection's instance ID (R4).
//!
//! Every notice names the pane that started the action (specification 25 §5.13).

use serde_json::{json, Map, Value};

use super::effects;
use super::write::{self, Target, Writes};
use crate::panes::forms::{
    self, coerce_submission, Catalog, CatalogProtocol, EditedConnection, FormInputs, FormRequest,
};
use crate::request::ActRequest;
use crate::session::{KnownRuntime, Session};
use crate::texts::connections as connection_texts;
use crate::texts::forms as texts;
use crate::topology;
use crate::view::types::{ActReply, FieldError, Form, ItemId, Refresh, RuntimeRef, Tone};

/// The action's payload did not name the protocol.
const NO_PROTOCOL: &str = "The action did not name the protocol.";
/// The action's payload did not name the connection.
const NO_CONNECTION: &str = "The action did not name the connection.";
/// A form that edits a connection whose instance ID cannot be read.
const NO_INSTANCE: &str = "The action did not name the connection to edit.";

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
    if let Some(route) = &form_request.route {
        return form_reply(forms::route_login_form(route));
    }
    let target = match write::target(session, request) {
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
    // A chooser reopens the form with the values typed so far as its prefill: only it posts values
    // to `form.open`, and a payload's own prefill wins over them (specification 25 §5.14).
    let posted = (!request.values.is_empty()).then_some(&request.values);
    let inputs = FormInputs {
        catalog,
        runtime: &runtime,
        runtime_choices: &choices,
        reachable: target.reachable(),
        prefill: form_request.prefill.as_ref().or(posted),
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
    let target = match write::target(session, request) {
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
        &apply_request(&protocol, "validate", instance_id.as_deref(), &params),
    ) {
        Sent::Answered(answer) => super::reply(
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
    let target = match write::target(session, request) {
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
        &apply_request(&protocol, action, instance_id.as_deref(), &params),
    ) {
        Sent::Answered(answer) => {
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

/// `connection.test`: converts the values and sends `comm.test` to the runtime the form names,
/// answered in the form's pane (specification 25 §5.10 and §5.13). A failed OPC UA client test
/// carries the server's recovery text; a runtime that does not answer is refused with the reason of
/// the form's disabled `Test` button.
#[must_use]
pub(crate) fn test(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let (runtime, endpoint) = match live_target(session, request, texts::TEST_NEEDS_REACHABLE) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let schema = match schema_of(runtime, &protocol) {
        Ok(schema) => schema,
        Err(message) => return super::error(&pane, &message),
    };
    let params = match coerce_submission(&schema, &request.values) {
        Ok(params) => params,
        Err(errors) => return field_errors(&pane, errors),
    };
    match send_entry(
        request,
        &endpoint,
        "comm.test",
        &test_request(&protocol, &params),
    ) {
        Sent::Answered(answer) => {
            let ok = answer.get("ok").and_then(Value::as_bool).unwrap_or(false);
            let tone = if ok { Tone::Ok } else { Tone::Error };
            super::reply(
                ok,
                Some(super::notice(tone, &pane, test_message(&protocol, &answer))),
                Vec::new(),
                false,
                Refresh::None,
            )
        }
        Sent::Fields(errors) => field_errors(&pane, errors),
        Sent::Refused(message) => failure(&pane, message),
    }
}

/// `connection.apply_live`: converts the values and applies them to the runtime the form names with
/// `comm.apply` `upsert`, answering the runtime's own message and refreshing now (design §4.3 B7,
/// R2; specification 25 §5.14). A runtime that does not answer is refused with the reason of the
/// form's disabled `Apply to running runtime` button.
#[must_use]
pub(crate) fn apply_live(session: &Session, request: &ActRequest) -> ActReply {
    let pane = super::pane(request);
    let Some(protocol) = protocol_of(request) else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let (runtime, endpoint) = match live_target(session, request, texts::APPLY_NEEDS_REACHABLE) {
        Ok(target) => target,
        Err(message) => return super::error(&pane, &message),
    };
    let schema = match schema_of(runtime, &protocol) {
        Ok(schema) => schema,
        Err(message) => return super::error(&pane, &message),
    };
    let params = match coerce_submission(&schema, &request.values) {
        Ok(params) => params,
        Err(errors) => return field_errors(&pane, errors),
    };
    let instance_id = instance_id(runtime, request);
    match send_entry(
        request,
        &endpoint,
        "comm.apply",
        &apply_request(&protocol, "upsert", instance_id.as_deref(), &params),
    ) {
        Sent::Answered(answer) => super::reply(
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

/// `connection.diagnose`: the connection's diagnosis. The ADS Doctor's checks arrive in a later
/// slice, so this answers honestly that the diagnosis is not available
/// (specification 25 §5.13; inventory N28).
#[must_use]
pub(crate) fn diagnose(request: &ActRequest) -> ActReply {
    super::error(&super::pane(request), texts::DIAGNOSE_NOT_AVAILABLE)
}

/// The runtime and endpoint a live action (`Test`, `Apply to running runtime`) works on: the runtime
/// the payload names, over its control endpoint, which answered (specification 25 §5.13). A runtime
/// that does not answer is refused with `reason`, the text of the form's disabled button; a runtime
/// the session does not know is refused by name (design §4.3 B7).
fn live_target<'a>(
    session: &'a Session,
    request: &ActRequest,
    reason: &str,
) -> Result<(&'a KnownRuntime, String), String> {
    let Some(id) = super::payload_text(&request.op.payload, "runtime") else {
        return Err(texts::NO_RUNTIME.to_owned());
    };
    let Some(runtime) = session.runtime(&id) else {
        return Err(texts::runtime_not_shown(&id));
    };
    match write::writes(runtime) {
        Some(Writes::Endpoint(endpoint)) => Ok((runtime, endpoint)),
        Some(Writes::Project) | None => Err(reason.to_owned()),
    }
}

/// One `comm.test` payload, as the runtime reads it: the protocol and the connection's coerced
/// parameters (design §1.4.2). The server stamps the credential channel itself, so none is sent.
fn test_request(protocol: &str, params: &Map<String, Value>) -> Value {
    json!({ "protocol": protocol, "params": params })
}

/// The text one `comm.test` answer gives, as the extension's `commTestMessage` does
/// (specification 25 §5.10): the answer's own `detail`, except a failed OPC UA client test, whose
/// error code names the server's recovery text; an unknown code keeps the answer's detail.
pub(crate) fn test_message(protocol: &str, answer: &Value) -> String {
    let detail = super::payload_text(answer, "detail").unwrap_or_default();
    if protocol != "opcua_client" || answer.get("ok").and_then(Value::as_bool) == Some(true) {
        return detail;
    }
    let code = answer
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str);
    match code {
        Some("endpoint_unreachable") => texts::OPCUA_UNREACHABLE.to_owned(),
        Some("auth_required") => texts::OPCUA_AUTH_REQUIRED.to_owned(),
        Some("cert_untrusted") => texts::OPCUA_CERT_UNTRUSTED.to_owned(),
        Some("unsupported_security_profile") => texts::OPCUA_UNSUPPORTED_SECURITY.to_owned(),
        Some("browse_denied") => texts::OPCUA_BROWSE_DENIED.to_owned(),
        _ => detail,
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
    let target = match write::target(session, request) {
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
        &apply_request(&protocol, action, Some(&instance_id), &params),
    ) {
        Sent::Answered(answer) => super::reply(
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

/// What one `comm.apply` or `comm.test` answered.
enum Sent {
    /// It answered; the target's own result.
    Answered(Value),
    /// The fields were refused, and nothing was applied.
    Fields(Vec<FieldError>),
    /// The target refused the action, or it could not be reached.
    Refused(String),
}

/// Sends one `comm.apply` to the action's target.
fn send(request: &ActRequest, target: &Target<'_>, params: &Value) -> Sent {
    match write::apply(request, &target.writes, params) {
        Ok(answer) => classify(answer),
        Err(message) => Sent::Refused(message),
    }
}

/// Sends one control request `entry` (`comm.apply` or `comm.test`) to a live endpoint and reads its
/// answer, as `send` reads one `comm.apply`.
fn send_entry(request: &ActRequest, endpoint: &str, entry: &str, params: &Value) -> Sent {
    match write::send_control(request, endpoint, entry, params) {
        Ok(answer) => classify(answer),
        Err(message) => Sent::Refused(message),
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
        Sent::Answered(answer)
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
