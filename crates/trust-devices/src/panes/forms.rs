//! The forms of the panel (specification 25 §5.2 to §5.4, §5.9 and §5.13): the form of a
//! connection, to add one or to edit one, and the forms of the `Runtimes` entries of `+ Add`.
//! Rust chooses every field's value, its options and their labels, the list editors, the buttons
//! and the reasons of the disabled ones; the panel draws the form and sends back what the user
//! typed.
//!
//! - `catalog`: the protocols of a `comm.schema` answer, with the words of the add picker and of
//!   the forms that the answer carries (specification 11).
//! - `connection_form` and `setup_form`: the forms that `form.open` answers. `FormRequest` is the
//!   payload of every action that opens a form; `form_id` derives a form's identity from it, so
//!   that the panel keeps an edit in progress until the protocol, the runtime or the prefill
//!   changes.
//! - `route_login_form`: the browse pane's `Create route` login form, which asks once for the
//!   `TwinCAT` administrator login and posts `ads.route_add` (specification 25 §5.4).
//! - `submit`: what a submitted form saves, `visible` and `coerce_submission`.

mod catalog;
mod submit;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub use catalog::{
    Catalog, CatalogField, CatalogProtocol, ItemField, ListTexts, OneOf, PickerGroup, PickerText,
};
pub use submit::{coerce_submission, visible};

use crate::texts::browse as browse_texts;
use crate::texts::forms as texts;
use crate::view::types::{
    Action, Confirm, Emphasis, FieldKind, FieldOption, Form, FormField, FormSection, GhostTile,
    ItemId, ListEditor, ListProperty, Op, RuntimeChoice, RuntimeRef, UiOp,
};

// ---- opening a form ----

/// What an action that opens a form sends to `form.open`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FormRequest {
    /// The protocol of a connection's form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// The form of a `Runtimes` entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<RuntimeSetup>,
    /// The runtime that the connection belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// The connection that the form edits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    /// The values that a quick start or a find fills in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefill: Option<Map<String, Value>>,
    /// The login form of the browse pane's `Create route` (specification 25 §5.4; design N26).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<RouteForm>,
}

/// What the browse pane's `Create route` needs to open its login form (specification 25 §5.4, its
/// lines 387-394; design N26): the pane that started it, the runtime that creates the route, and
/// the route plan's own route name, `TwinCAT` target and host identity. The administrator login is
/// never part of this: the form collects it once and sends it straight to `ads.route_add`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteForm {
    /// The pane that started the action, such as `browse:ads`.
    pub pane: String,
    /// The runtime that creates the route.
    pub runtime: String,
    /// The route's name.
    pub route_name: String,
    /// The `TwinCAT` target, as the route plan described it.
    pub target: Value,
    /// This host's identity, as the route plan described it.
    pub local: Value,
}

impl RouteForm {
    /// The identity of its form: its runtime and route name.
    #[must_use]
    pub fn form_id(&self) -> String {
        format!("ads.route@{}#{}", self.runtime, self.route_name)
    }
}

impl FormRequest {
    /// The request as the payload of a `form.open` action.
    #[must_use]
    pub fn payload(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// The forms of the `Runtimes` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSetup {
    /// `Connect existing runtime`.
    Connect,
    /// `Run a runtime on this computer`.
    Local,
}

impl RuntimeSetup {
    /// The identity of its form.
    #[must_use]
    pub fn form_id(self) -> &'static str {
        match self {
            Self::Connect => "runtime.connect",
            Self::Local => "runtime.local",
        }
    }
}

/// The identity of the form that `request` opens: its protocol and runtime, the connection it
/// edits, and a digest of its prefill.
#[must_use]
pub fn form_id(request: &FormRequest) -> String {
    if let Some(setup) = request.setup {
        return setup.form_id().to_owned();
    }
    if let Some(route) = &request.route {
        return route.form_id();
    }
    let mut id = format!(
        "{}@{}",
        request.protocol.as_deref().unwrap_or_default(),
        request.runtime.as_deref().unwrap_or_default()
    );
    if let Some(connection) = &request.connection {
        id = format!("{id}#{connection}");
    }
    if let Some(prefill) = request
        .prefill
        .as_ref()
        .filter(|prefill| !prefill.is_empty())
    {
        let sorted: BTreeMap<&String, &Value> = prefill.iter().collect();
        let canonical = serde_json::to_string(&sorted).unwrap_or_default();
        id = format!("{id}~{:016x}", digest(&canonical));
    }
    id
}

/// The FNV-1a digest of `text`, which stays the same across builds.
fn digest(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    text.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    })
}

// ---- the form of a connection ----

/// A configured connection that a form edits.
#[derive(Debug, Clone, PartialEq)]
pub struct EditedConnection {
    /// The connection's identity.
    pub id: ItemId,
    /// Its protocol's name as its pill shows it, such as `Modbus TCP`.
    pub protocol_name: String,
    /// Its configuration, without secrets.
    pub params: Map<String, Value>,
    /// Whether it is disabled.
    pub disabled: bool,
}

/// What the form of a connection is built from.
#[derive(Debug, Clone, Copy)]
pub struct FormInputs<'a> {
    /// The catalog of the runtime that the form writes to.
    pub catalog: &'a Catalog,
    /// That runtime.
    pub runtime: &'a RuntimeRef,
    /// The runtimes that a new connection can be saved to instead.
    pub runtime_choices: &'a [RuntimeRef],
    /// Whether that runtime answers, so that `Test` and `Apply to running runtime` can act on it.
    pub reachable: bool,
    /// The values that a quick start or a find fills in.
    pub prefill: Option<&'a Map<String, Value>>,
    /// The connection that the form edits; `None` for a new one.
    pub connection: Option<&'a EditedConnection>,
}

/// The form of a connection of `protocol`: a new one on `inputs.runtime`, with the values of
/// `inputs.prefill`, or `inputs.connection` to edit. Every field shows its current value, the
/// prefill or its default, and never a secret; the advanced fields follow under `Advanced`, open
/// (specification 25 §5.13).
///
/// # Errors
///
/// The catalog does not hold `protocol`; the text says so for the pane's notice.
pub fn connection_form(protocol: &str, inputs: &FormInputs<'_>) -> Result<Form, String> {
    let Some(schema) = inputs.catalog.protocol(protocol) else {
        return Err(texts::NO_NATIVE_SCHEMA.to_owned());
    };
    let edited = inputs.connection;
    let request = FormRequest {
        protocol: Some(schema.id.clone()),
        runtime: Some(inputs.runtime.id.0.clone()),
        connection: edited.map(|connection| connection.id.0.clone()),
        prefill: inputs.prefill.filter(|_| edited.is_none()).cloned(),
        setup: None,
        route: None,
    };
    let id = form_id(&request);
    let (basic, advanced): (Vec<&CatalogField>, Vec<&CatalogField>) =
        schema.fields.iter().partition(|field| !field.advanced);
    let section = |heading: Option<&str>, fields: Vec<&CatalogField>| FormSection {
        heading: heading.map(str::to_owned),
        fields: fields
            .into_iter()
            .map(|field| form_field(field, current_value(field, inputs)))
            .collect(),
    };
    let mut sections = vec![section(None, basic)];
    if !advanced.is_empty() {
        sections.push(section(Some(texts::ADVANCED), advanced));
    }
    let (title, breadcrumb, runtime_choices) = match edited {
        Some(connection) => (
            connection.protocol_name.clone(),
            texts::edit_breadcrumb(&connection.protocol_name),
            // An edit writes to the runtime the connection belongs to, so it offers no other one.
            Vec::new(),
        ),
        None => (
            texts::add_title(&schema.title),
            texts::ADD_FORM_BREADCRUMB.to_owned(),
            inputs
                .runtime_choices
                .iter()
                .map(|runtime| RuntimeChoice {
                    id: runtime.id.clone(),
                    name: runtime.name.clone(),
                    action: choice_action(runtime, schema, &id),
                })
                .collect(),
        ),
    };
    Ok(Form {
        buttons: form_buttons(schema, &id, inputs),
        id,
        protocol: schema.id.clone(),
        title,
        breadcrumb,
        owner_line: texts::owner_line(&inputs.runtime.name),
        purpose: (!schema.purpose.is_empty()).then(|| schema.purpose.clone()),
        runtime: Some(inputs.runtime.clone()),
        runtime_choices,
        sections,
        note: edited
            .filter(|connection| connection.disabled)
            .map(|_| texts::DISABLED_NOTE.to_owned()),
        ghost: edited.is_none().then(|| GhostTile {
            name: texts::GHOST_NAME.to_owned(),
            detail: texts::GHOST_DETAIL.to_owned(),
        }),
    })
}

/// What choosing a runtime in a form does (specification 25 §5.14): `form.open` of the same form
/// on that runtime. The panel sends the values typed so far with the action, which become the
/// reopened form's prefill, so a chooser that changes the runtime keeps what the user typed.
fn choice_action(runtime: &RuntimeRef, schema: &CatalogProtocol, form: &str) -> Action {
    button(
        runtime.id.as_str(),
        &runtime.name,
        Emphasis::Secondary,
        Op::Rust {
            action: "form.open".to_owned(),
            payload: json!({
                "form": form,
                "protocol": schema.id.as_str(),
                "runtime": runtime.id.as_str(),
            }),
        },
    )
}

/// A field's value in the form: never a secret; else the edited connection's value, or the
/// prefill of a new one, or the field's default.
fn current_value(field: &CatalogField, inputs: &FormInputs<'_>) -> Value {
    if field.secret {
        return Value::Null;
    }
    let given = match inputs.connection {
        Some(connection) => connection.params.get(&field.id),
        None => inputs.prefill.and_then(|prefill| prefill.get(&field.id)),
    };
    given.cloned().unwrap_or_else(|| field.default.clone())
}

fn field_kind(field: &CatalogField) -> FieldKind {
    if field.secret {
        return FieldKind::Secret;
    }
    match field.field_type.as_str() {
        "number" => FieldKind::Number,
        "boolean" => FieldKind::Boolean,
        "enum" => FieldKind::Choice,
        "json_array" => FieldKind::List,
        "json_object" => FieldKind::Object,
        _ => FieldKind::Text,
    }
}

fn form_field(field: &CatalogField, value: Value) -> FormField {
    let kind = field_kind(field);
    let (list, error) = if kind == FieldKind::List {
        let editor = list_editor(field, &value);
        let valid = value.is_array() || value.is_null();
        (
            Some(editor),
            (!valid).then(|| texts::INVALID_LIST.to_owned()),
        )
    } else {
        (None, None)
    };
    FormField {
        id: field.id.clone(),
        label: field.label.clone(),
        required: field.required,
        kind,
        value,
        options: field
            .options
            .iter()
            .map(|option| FieldOption {
                value: option.clone(),
                label: field
                    .option_labels
                    .get(option)
                    .cloned()
                    .unwrap_or_else(|| option.clone()),
            })
            .collect(),
        help: field.help.clone(),
        error,
        visible_when: field.visible_when.clone(),
        list,
        placeholder: None,
    }
}

/// The editor of the list `field` holding `value`. A list whose items the catalog describes
/// starts a new item from that description; any other list copies its first item, else the
/// catalog's template, and without either no item can be added.
fn list_editor(field: &CatalogField, value: &Value) -> ListEditor {
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    let words = field.list.clone().unwrap_or_else(|| ListTexts {
        item_noun: "item".to_owned(),
        empty_text: String::new(),
        item_template: Value::String(String::new()),
        item_type: None,
        item_fields: Vec::new(),
        item_one_of: Vec::new(),
    });
    let template = if words.describes_items() {
        words.item_template.clone()
    } else {
        items
            .iter()
            .find(|item| !item.is_null())
            .cloned()
            .unwrap_or_else(|| words.item_template.clone())
    };
    let item_kind = if words.item_type.is_some() {
        FieldKind::Number
    } else if words.item_fields.is_empty() {
        value_kind(&template)
    } else {
        FieldKind::Object
    };
    ListEditor {
        item_title: texts::item_title(&words.item_noun),
        add_label: texts::add_item(&words.item_noun),
        properties: properties(&words, &template, items),
        item_noun: words.item_noun,
        empty_text: words.empty_text,
        can_add: !template.is_null(),
        item_kind,
        item_template: template,
    }
}

/// The properties of the object items of a list: the described ones in their order, each with
/// the properties it can be exchanged for, then any other key of the template or the items.
fn properties(words: &ListTexts, template: &Value, items: &[Value]) -> Vec<ListProperty> {
    let described = words.item_fields.iter().map(|field| ListProperty {
        key: field.id.clone(),
        label: field.label.clone(),
        kind: if field.field_type == "enum" {
            FieldKind::Choice
        } else {
            FieldKind::Text
        },
        required: field.required,
        options: field
            .options
            .iter()
            .map(|option| FieldOption {
                value: option.clone(),
                label: option.clone(),
            })
            .collect(),
        alternatives: words
            .item_one_of
            .iter()
            .filter(|group| group.fields.contains(&field.id))
            .flat_map(|group| group.fields.iter().filter(|other| **other != field.id))
            .cloned()
            .collect(),
    });
    let known: BTreeSet<&String> = words.item_fields.iter().map(|field| &field.id).collect();
    let mut others: BTreeMap<&String, &Value> = BTreeMap::new();
    for object in std::iter::once(template)
        .chain(items)
        .filter_map(Value::as_object)
    {
        for (key, value) in object {
            if !known.contains(key) {
                others.entry(key).or_insert(value);
            }
        }
    }
    described
        .chain(others.into_iter().map(|(key, value)| ListProperty {
            key: key.clone(),
            label: texts::property_label(key),
            kind: value_kind(value),
            required: false,
            options: Vec::new(),
            alternatives: Vec::new(),
        }))
        .collect()
}

/// What a list item holds, by the item that a new one copies.
fn value_kind(value: &Value) -> FieldKind {
    match value {
        Value::Object(_) => FieldKind::Object,
        Value::Number(_) => FieldKind::Number,
        Value::Bool(_) => FieldKind::Boolean,
        _ => FieldKind::Text,
    }
}

/// The buttons of a connection's form (specification 25 §5.3 and §5.4): `Save`, or `Enable` for
/// a disabled connection; `Cancel`; `Test` when the protocol has one; for a connection being
/// edited, `Disable` and the two-step `Remove` when the catalog allows them; and
/// `Apply to running runtime`. `Test` and `Apply to running runtime` stay visible and say why
/// while the runtime does not answer.
fn form_buttons(schema: &CatalogProtocol, form: &str, inputs: &FormInputs<'_>) -> Vec<Action> {
    let mut payload = json!({
        "form": form,
        "protocol": schema.id,
        "runtime": inputs.runtime.id.as_str(),
    });
    if let Some(connection) = inputs.connection {
        payload["connection"] = Value::String(connection.id.0.clone());
    }
    let rust = |action: &str| Op::Rust {
        action: action.to_owned(),
        payload: payload.clone(),
    };
    let disabled = inputs
        .connection
        .is_some_and(|connection| connection.disabled);
    let mut buttons = vec![
        if disabled {
            button(
                "enable",
                texts::ENABLE,
                Emphasis::Primary,
                rust("connection.enable"),
            )
        } else {
            button(
                "save",
                texts::SAVE,
                Emphasis::Primary,
                rust("connection.save"),
            )
        },
        button(
            "cancel",
            texts::CANCEL,
            Emphasis::Secondary,
            Op::Ui {
                ui: UiOp::CloseInspector,
            },
        ),
    ];
    let live = |id: &str, label: &str, reason: &str, op: Op| {
        let mut action = button(id, label, Emphasis::Secondary, op);
        if !inputs.reachable {
            action.enabled = false;
            action.reason = Some(reason.to_owned());
            action.emphasis = Emphasis::Quiet;
        }
        action
    };
    if schema.supports_test {
        buttons.push(live(
            "test",
            texts::TEST,
            texts::TEST_NEEDS_REACHABLE,
            rust("connection.test"),
        ));
    }
    if inputs.connection.is_some() {
        if schema.allows("disable") && !disabled {
            buttons.push(button(
                "disable",
                texts::DISABLE,
                Emphasis::Secondary,
                rust("connection.disable"),
            ));
        }
        if schema.allows("remove") {
            let mut remove = button(
                "remove",
                texts::REMOVE,
                Emphasis::Danger,
                rust("connection.remove"),
            );
            remove.confirm = Some(Confirm {
                text: texts::REMOVE_CONFIRMATION.to_owned(),
                confirm: texts::CONFIRM_REMOVE.to_owned(),
                cancel: texts::CANCEL.to_owned(),
            });
            buttons.push(remove);
        }
    }
    buttons.push(live(
        "apply_live",
        texts::APPLY_TO_RUNNING_RUNTIME,
        texts::APPLY_NEEDS_REACHABLE,
        rust("connection.apply_live"),
    ));
    buttons
}

fn button(id: &str, label: &str, emphasis: Emphasis, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis,
        confirm: None,
        op,
    }
}

// ---- the forms of `Runtimes` ----

/// The login form of the browse pane's `Create route` (specification 25 §5.4, its lines 387-394;
/// design N26): one `TwinCAT` administrator user name and one password, asked once, and the
/// `Create route` button that posts `ads.route_add` with the route plan's own pane, runtime, route
/// name, target and identity. The login lives only in the form's values and the one request they
/// make; nothing here stores it (specification 25:389-394).
#[must_use]
pub fn route_login_form(route: &RouteForm) -> Form {
    let id = route.form_id();
    let payload = json!({
        "pane": format!("form:{id}"),
        "runtime": route.runtime,
        "route_name": route.route_name,
        "target": route.target,
        "local": route.local,
    });
    Form {
        id: id.clone(),
        protocol: "ads".to_owned(),
        title: browse_texts::CREATE_ROUTE.to_owned(),
        breadcrumb: texts::CREATE_ROUTE_BREADCRUMB.to_owned(),
        owner_line: String::new(),
        purpose: Some(texts::CREATE_ROUTE_PURPOSE.to_owned()),
        runtime: None,
        runtime_choices: Vec::new(),
        sections: vec![FormSection {
            heading: None,
            fields: vec![
                setup_field(
                    "username",
                    texts::TWINCAT_USERNAME,
                    FieldKind::Text,
                    "",
                    texts::TWINCAT_LOGIN_HELP,
                ),
                setup_field(
                    "password",
                    texts::TWINCAT_PASSWORD,
                    FieldKind::Secret,
                    "",
                    texts::TWINCAT_LOGIN_HELP,
                ),
            ],
        }],
        buttons: vec![
            button(
                "create_route",
                browse_texts::CREATE_ROUTE,
                Emphasis::Primary,
                Op::Rust {
                    action: "ads.route_add".to_owned(),
                    payload,
                },
            ),
            button(
                "cancel",
                texts::CANCEL,
                Emphasis::Secondary,
                Op::Ui {
                    ui: UiOp::CloseInspector,
                },
            ),
        ],
        note: Some(texts::LOGIN_NOT_STORED.to_owned()),
        ghost: None,
    }
}

/// The form of a `Runtimes` entry (specification 25 §5.2): the address and token of a runtime
/// that already runs elsewhere, or the name and template of another runtime on this computer.
/// Both add a runtime rather than writing to one, so they name none.
#[must_use]
pub fn setup_form(setup: RuntimeSetup) -> Form {
    let id = setup.form_id();
    let (title, purpose, fields, action, note) = match setup {
        RuntimeSetup::Connect => (
            texts::CONNECT_EXISTING_RUNTIME,
            texts::CONNECT_RUNTIME_PURPOSE,
            connect_fields(),
            "runtime.add_host",
            Some(texts::USE_DISCOVER_HINT),
        ),
        RuntimeSetup::Local => (
            texts::ADD_A_RUNTIME,
            texts::LOCAL_RUNTIME_PURPOSE,
            local_fields(),
            "runtime.add_local",
            None,
        ),
    };
    Form {
        id: id.to_owned(),
        protocol: id.to_owned(),
        title: title.to_owned(),
        breadcrumb: texts::RUNTIME_SETUP_BREADCRUMB.to_owned(),
        owner_line: String::new(),
        purpose: Some(purpose.to_owned()),
        runtime: None,
        runtime_choices: Vec::new(),
        sections: vec![FormSection {
            heading: None,
            fields,
        }],
        buttons: vec![button(
            "add_runtime",
            texts::ADD_RUNTIME,
            Emphasis::Primary,
            Op::Rust {
                action: action.to_owned(),
                payload: json!({ "form": id }),
            },
        )],
        note: note.map(str::to_owned),
        ghost: None,
    }
}

/// The address of a runtime that already runs, and its token, which is optional.
fn connect_fields() -> Vec<FormField> {
    vec![
        setup_field(
            "endpoint",
            texts::RUNTIME_ADDRESS,
            FieldKind::Text,
            texts::RUNTIME_ADDRESS_PLACEHOLDER,
            texts::RUNTIME_ADDRESS_HELP,
        ),
        FormField {
            required: false,
            ..setup_field(
                "token",
                texts::RUNTIME_TOKEN,
                FieldKind::Secret,
                texts::RUNTIME_TOKEN_PLACEHOLDER,
                texts::RUNTIME_TOKEN_HELP,
            )
        },
    ]
}

/// The name of another runtime on this computer, and its template: the simulator, or loopback
/// I/O.
fn local_fields() -> Vec<FormField> {
    let template = |value: &str, label: &str| FieldOption {
        value: value.to_owned(),
        label: label.to_owned(),
    };
    vec![
        setup_field(
            "name",
            texts::RUNTIME_NAME,
            FieldKind::Text,
            texts::RUNTIME_NAME_PLACEHOLDER,
            "",
        ),
        FormField {
            value: json!("simulate"),
            options: vec![
                template("simulate", texts::TEMPLATE_SIMULATOR),
                template("empty", texts::TEMPLATE_EMPTY),
            ],
            ..setup_field(
                "template",
                texts::RUNTIME_TEMPLATE,
                FieldKind::Choice,
                "",
                "",
            )
        },
    ]
}

/// A required field of a runtime setup form, empty until the user types.
fn setup_field(id: &str, label: &str, kind: FieldKind, placeholder: &str, help: &str) -> FormField {
    FormField {
        id: id.to_owned(),
        label: label.to_owned(),
        required: true,
        kind,
        value: Value::String(String::new()),
        options: Vec::new(),
        help: help.to_owned(),
        error: None,
        visible_when: None,
        list: None,
        placeholder: Some(placeholder.to_owned()).filter(|text| !text.is_empty()),
    }
}
