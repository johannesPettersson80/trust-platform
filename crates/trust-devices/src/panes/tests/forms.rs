//! The forms (specification 25 §5.3, §5.4, §5.9 and §5.13): their fields, sections and buttons,
//! and what a submitted form saves. The list texts and option labels are the ones that
//! `SchemaFields.tsx` held, which the catalog now carries.

use serde_json::{json, Map, Value};

use super::add::{library, without_words};
use crate::panes::forms::{
    coerce_submission, connection_form, form_id, route_login_form, visible, Catalog,
    EditedConnection, FormInputs, FormRequest, RouteForm,
};
use crate::view::types::{
    Action, FieldError, FieldKind, Form, FormField, ItemId, ListEditor, Op, RuntimeRef,
};

fn catalog() -> Catalog {
    Catalog::parse(&library()).expect("the tool's own catalog")
}

fn simulator() -> RuntimeRef {
    RuntimeRef {
        id: ItemId("runtime:local".to_owned()),
        name: "Simulator".to_owned(),
    }
}

/// The form of a new connection of `protocol` on the simulator, which answers when `reachable`.
fn new_form(catalog: &Catalog, protocol: &str, reachable: bool) -> Form {
    let runtime = simulator();
    let inputs = FormInputs {
        catalog,
        runtime: &runtime,
        runtime_choices: std::slice::from_ref(&runtime),
        reachable,
        prefill: None,
        connection: None,
    };
    connection_form(protocol, &inputs).expect("the catalog holds the protocol")
}

fn field<'a>(form: &'a Form, id: &str) -> &'a FormField {
    form.sections
        .iter()
        .flat_map(|section| &section.fields)
        .find(|field| field.id == id)
        .unwrap_or_else(|| panic!("a field {id}"))
}

fn list<'a>(form: &'a Form, id: &str) -> &'a ListEditor {
    field(form, id)
        .list
        .as_ref()
        .unwrap_or_else(|| panic!("{id} has a list editor"))
}

fn ids(form: &Form, section: usize) -> Vec<&str> {
    form.sections[section]
        .fields
        .iter()
        .map(|field| field.id.as_str())
        .collect()
}

fn values(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(values) => values,
        other => panic!("values are an object: {other}"),
    }
}

fn errors(fields: &[(&str, &str)]) -> Vec<FieldError> {
    fields
        .iter()
        .map(|(field, message)| FieldError {
            field: (*field).to_owned(),
            message: (*message).to_owned(),
        })
        .collect()
}

fn button_ids(buttons: &[Action]) -> Vec<&str> {
    buttons.iter().map(|button| button.id.as_str()).collect()
}

#[test]
fn advanced_fields_under_a_heading_not_collapsed() {
    let catalog = catalog();
    let mqtt = new_form(&catalog, "mqtt", true);
    assert_eq!(mqtt.title, "Add MQTT");
    assert_eq!(
        mqtt.breadcrumb,
        "Devices & Connections / Add device or connection"
    );
    assert_eq!(mqtt.owner_line, "on Simulator");
    assert_eq!(
        mqtt.purpose.as_deref(),
        Some("Publish and subscribe process I/O through a broker.")
    );
    assert_eq!(mqtt.sections.len(), 2);
    assert_eq!(mqtt.sections[0].heading, None);
    assert_eq!(
        ids(&mqtt, 0),
        [
            "broker",
            "client_id",
            "topic_in",
            "topic_out",
            "username",
            "password",
            "tls",
            "allow_insecure_remote",
            "reconnect_ms",
            "keep_alive_s",
            "on_error",
        ]
    );
    assert_eq!(mqtt.sections[1].heading.as_deref(), Some("Advanced"));
    assert_eq!(
        ids(&mqtt, 1),
        [
            "tls_ca_path",
            "tls_client_cert_path",
            "tls_client_key_path",
            "tls_alpn",
        ],
        "every advanced field is listed, none is hidden"
    );
    let ads_server = new_form(&catalog, "ads_server", true);
    assert_eq!(ads_server.sections[1].fields.len(), 7, "the seven limits");
    let loopback = new_form(&catalog, "loopback", true);
    assert_eq!(loopback.sections.len(), 1, "no empty Advanced section");
    let modbus = new_form(&catalog, "modbus_tcp", true);
    assert_eq!(field(&modbus, "address").value, json!("127.0.0.1:502"));
    assert_eq!(modbus.sections.len(), 2);
    assert_eq!(modbus.sections[1].heading.as_deref(), Some("Advanced"));
    assert_eq!(
        ids(&modbus, 1),
        ["vendor_name", "product_code", "revision"],
        "the expected identity is advanced (specification 11, Compare with the project)"
    );
    assert_eq!(field(&mqtt, "password").kind, FieldKind::Secret);
    assert_eq!(field(&mqtt, "password").value, Value::Null);
    let ghost = mqtt.ghost.as_ref().expect("the draft tile");
    assert_eq!(
        (ghost.name.as_str(), ghost.detail.as_str()),
        ("New device", "Draft: configure and Apply")
    );
}

#[test]
fn hidden_conditional_fields_are_not_submitted() {
    let catalog = catalog();
    let gpio = catalog.protocol("gpio").expect("gpio");
    let form = new_form(&catalog, "gpio", true);
    let rule = field(&form, "chip")
        .visible_when
        .as_ref()
        .expect("the chip path depends on the backend");
    assert_eq!(
        (rule.field.as_str(), rule.equals.as_str()),
        ("backend", "libgpiod")
    );

    let submitted = |backend: Option<&str>| {
        let mut submitted = values(json!({
            "chip": "/dev/gpiochip9", "sysfs_base": "/sys/class/gpio",
            "inputs": [], "outputs": [],
        }));
        if let Some(backend) = backend {
            submitted.insert("backend".to_owned(), json!(backend));
        }
        submitted
    };
    let sysfs = submitted(Some("sysfs"));
    let chip = gpio.field("chip").expect("chip");
    assert!(!visible(gpio, chip, &sysfs));
    let params = coerce_submission(gpio, &sysfs).expect("valid values");
    assert_eq!(params.get("chip"), None, "a hidden field is not saved");
    assert_eq!(params["sysfs_base"], json!("/sys/class/gpio"));

    let libgpiod = coerce_submission(gpio, &submitted(Some("libgpiod"))).expect("valid values");
    assert_eq!(libgpiod["chip"], json!("/dev/gpiochip9"));
    assert_eq!(libgpiod.get("sysfs_base"), None);

    // Without the backend, its default `libgpiod` decides.
    let default = coerce_submission(gpio, &submitted(None)).expect("valid values");
    assert_eq!(default["chip"], json!("/dev/gpiochip9"));
    assert_eq!(default.get("sysfs_base"), None);
}

#[test]
fn coerce_submission_by_schema_type() {
    let catalog = catalog();
    let modbus = catalog.protocol("modbus_tcp").expect("modbus_tcp");
    let params = coerce_submission(
        modbus,
        &values(json!({
            "address": "10.0.0.9:502", "unit_id": "7", "input_start": 3,
            "timeout_ms": " 250 ", "on_error": "warn", "unknown": "dropped",
        })),
    )
    .expect("valid values");
    assert_eq!(
        Value::Object(params),
        json!({ "address": "10.0.0.9:502", "unit_id": 7, "input_start": 3,
                "timeout_ms": 250, "on_error": "warn" })
    );
    let refused = coerce_submission(
        modbus,
        &values(json!({ "unit_id": "seven", "timeout_ms": 2.5, "on_error": "explode" })),
    );
    assert_eq!(
        refused,
        Err(errors(&[
            ("unit_id", "Enter a whole number."),
            ("timeout_ms", "Enter a whole number."),
            ("on_error", "Choose a listed value."),
        ])),
        "invalid input is a field error, never text"
    );

    let mqtt = catalog.protocol("mqtt").expect("mqtt");
    let params = coerce_submission(
        mqtt,
        &values(
            json!({ "tls": "true", "tls_alpn": "[\"h2\"]", "password": null, "client_id": "" }),
        ),
    )
    .expect("valid values");
    assert_eq!(
        Value::Object(params),
        json!({ "tls": true, "tls_alpn": ["h2"], "client_id": "" }),
        "a secret left as null is not sent"
    );
    let refused = coerce_submission(
        mqtt,
        &values(json!({ "tls": "yes", "tls_alpn": "h2", "client_id": 5 })),
    );
    assert_eq!(
        refused,
        Err(errors(&[
            ("client_id", "Enter text."),
            ("tls", "Choose a listed value."),
            ("tls_alpn", "Enter a JSON array."),
        ]))
    );

    let mesh = catalog.protocol("mesh").expect("mesh");
    let params = coerce_submission(
        mesh,
        &values(json!({ "subscribe": "{\"a\": \"b\"}", "connect": "" })),
    )
    .expect("valid values");
    assert_eq!(params["subscribe"], json!({ "a": "b" }));
    assert_eq!(
        params["connect"],
        json!([]),
        "an empty list box is an empty list"
    );
    assert_eq!(
        coerce_submission(mesh, &values(json!({ "subscribe": "[1]" }))),
        Err(errors(&[("subscribe", "Enter a JSON object.")]))
    );

    // A list's items are converted to the shape of its default items.
    let gpio = catalog.protocol("gpio").expect("gpio");
    let params = coerce_submission(
        gpio,
        &values(json!({ "inputs": [{ "address": "%IX0.1", "line": "18" }] })),
    )
    .expect("valid values");
    assert_eq!(
        params["inputs"],
        json!([{ "address": "%IX0.1", "line": 18 }])
    );
    assert_eq!(
        coerce_submission(gpio, &values(json!({ "inputs": [{ "line": "x" }] }))),
        Err(errors(&[("inputs", "Enter a whole number.")]))
    );
}

/// The options of a field, each with its label.
type Labels = &'static [(&'static str, &'static str)];

/// The option labels of `SchemaFields.tsx`: protocol, field, and each option with its label.
const OPTION_LABELS: [(&str, &str, Labels); 6] = [
    (
        "modbus_tcp",
        "on_error",
        &[
            ("fault", "Stop with fault"),
            ("warn", "Warn and continue"),
            ("ignore", "Ignore"),
        ],
    ),
    (
        "realtime_t0",
        "scheduler",
        &[
            ("fifo", "FIFO"),
            ("rr", "Round robin"),
            ("other", "Default"),
        ],
    ),
    (
        "openot",
        "source",
        &[("heartbeat", "Heartbeat"), ("st-fb", "ST producer")],
    ),
    (
        "openot",
        "fence_mode",
        &[("fenced", "Fenced"), ("unfenced", "Unfenced")],
    ),
    (
        "runtime_cloud",
        "profile",
        &[("dev", "Development"), ("plant", "Plant"), ("wan", "WAN")],
    ),
    (
        "gpio",
        "backend",
        &[("libgpiod", "libgpiod"), ("sysfs", "sysfs")],
    ),
];

#[test]
fn option_labels_and_item_nouns_come_from_rust() {
    let catalog = catalog();
    for (protocol, id, expected) in OPTION_LABELS {
        let form = new_form(&catalog, protocol, true);
        let shown: Vec<(&str, &str)> = field(&form, id)
            .options
            .iter()
            .map(|option| (option.value.as_str(), option.label.as_str()))
            .collect();
        assert_eq!(shown, expected, "{protocol}.{id}");
    }

    let mesh = new_form(&catalog, "mesh", true);
    let peers = list(&mesh, "connect");
    assert_eq!(
        (
            peers.item_noun.as_str(),
            peers.item_title.as_str(),
            peers.add_label.as_str(),
            peers.empty_text.as_str(),
        ),
        (
            "peer address",
            "Peer address",
            "Add peer address",
            "No peer addresses yet. Add an item below."
        )
    );
    assert!(peers.can_add);
    assert_eq!(peers.item_kind, FieldKind::Text);
    let alpn = list(&new_form(&catalog, "mqtt", true), "tls_alpn").clone();
    assert_eq!(
        (alpn.item_title.as_str(), alpn.add_label.as_str()),
        ("TLS ALPN", "Add TLS ALPN"),
        "acronyms keep their capitals"
    );
    let modules = list(&new_form(&catalog, "ethercat", true), "modules").clone();
    assert_eq!(modules.item_kind, FieldKind::Object);
    assert_eq!(
        modules.item_template,
        json!({ "model": "EK1100", "slot": 0 })
    );
    let labels: Vec<(&str, &str)> = modules
        .properties
        .iter()
        .map(|property| (property.key.as_str(), property.label.as_str()))
        .collect();
    assert_eq!(
        labels,
        [
            ("channels", "Channels"),
            ("model", "Model"),
            ("slot", "Slot")
        ]
    );

    // The words come from the catalog: a runtime's own words win, and a runtime without them
    // gets the tool's own.
    let mut served = library();
    served["protocols"][0]["fields"][5]["option_labels"]["fault"] = json!("Halt");
    let served = Catalog::parse(&served).expect("the served catalog");
    assert_eq!(
        field(&new_form(&served, "modbus_tcp", true), "on_error").options[0].label,
        "Halt"
    );
    let older = Catalog::parse(&without_words(library())).expect("an older catalog");
    assert_eq!(older.protocols[0].fields[5].option_labels.len(), 0);
}

#[test]
fn connection_lists_and_invalid_values_keep_their_texts() {
    let catalog = catalog();
    let client = new_form(&catalog, "opcua_client", true);
    let connections = list(&client, "connections");
    assert!(
        !connections.can_add,
        "connections come from Discover or Browse"
    );
    assert_eq!(
        connections.empty_text,
        "No connections yet. Use Discover or Browse to add one instead of typing JSON."
    );
    assert_eq!(
        list(&new_form(&catalog, "opcua", true), "expose").empty_text,
        "No globals selected yet. Use Choose globals to pick project variables, or add a \
         pattern manually."
    );

    // An existing connection is the template of another one, and an invalid list says so.
    let runtime = simulator();
    let edited = EditedConnection {
        id: ItemId("endpoint:client".to_owned()),
        protocol_name: "OPC UA client".to_owned(),
        params: values(json!({
            "connections": [{ "name": "line", "endpoint_url": "opc.tcp://a:4840" }],
        })),
        disabled: false,
    };
    let inputs = FormInputs {
        catalog: &catalog,
        runtime: &runtime,
        runtime_choices: &[],
        reachable: true,
        prefill: None,
        connection: Some(&edited),
    };
    let form = connection_form("opcua_client", &inputs).expect("a form");
    let connections = list(&form, "connections");
    assert!(connections.can_add);
    let endpoint = connections
        .properties
        .iter()
        .find(|property| property.key == "endpoint_url")
        .expect("the endpoint's property");
    assert_eq!(endpoint.label, "Endpoint Url");
    let mut broken = edited.clone();
    broken.params = values(json!({ "connections": "not a list" }));
    let inputs = FormInputs {
        connection: Some(&broken),
        ..inputs
    };
    let form = connection_form("opcua_client", &inputs).expect("a form");
    assert_eq!(
        field(&form, "connections").error.as_deref(),
        Some("Existing value is not a valid list yet. Fix it here or reset the field.")
    );
    let unknown = connection_form("mystery", &inputs);
    assert_eq!(
        unknown,
        Err("This runtime did not return a native setup schema for this protocol.".to_owned())
    );
}

#[test]
fn form_buttons_follow_the_catalog_and_the_runtime() {
    let catalog = catalog();
    let add = new_form(&catalog, "modbus_tcp", false);
    assert_eq!(
        button_ids(&add.buttons),
        ["save", "cancel", "test", "apply_live"]
    );
    let test = &add.buttons[2];
    assert!(!test.enabled);
    assert_eq!(
        test.reason.as_deref(),
        Some("Select a reachable online runtime before testing Communication setup.")
    );
    assert_eq!(
        add.buttons[3].reason.as_deref(),
        Some("Select a reachable online runtime before applying Communication setup.")
    );
    assert!(new_form(&catalog, "modbus_tcp", true)
        .buttons
        .iter()
        .all(|button| button.enabled));
    assert_eq!(
        button_ids(&new_form(&catalog, "ethercat", true).buttons),
        ["save", "cancel", "apply_live"],
        "no Test without one in the catalog"
    );

    let runtime = simulator();
    let mut edited = EditedConnection {
        id: ItemId("endpoint:modbus".to_owned()),
        protocol_name: "Modbus TCP".to_owned(),
        params: values(json!({ "address": "10.0.0.9:502" })),
        disabled: false,
    };
    let edit = |edited: &EditedConnection| {
        let inputs = FormInputs {
            catalog: &catalog,
            runtime: &runtime,
            runtime_choices: &[],
            reachable: true,
            prefill: None,
            connection: Some(edited),
        };
        connection_form("modbus_tcp", &inputs).expect("a form")
    };
    let form = edit(&edited);
    assert_eq!(form.title, "Modbus TCP");
    assert_eq!(form.breadcrumb, "Devices & Connections / Edit Modbus TCP");
    assert_eq!(field(&form, "address").value, json!("10.0.0.9:502"));
    assert_eq!(form.ghost, None);
    assert_eq!(
        button_ids(&form.buttons),
        ["save", "cancel", "test", "disable", "remove", "apply_live"]
    );
    let remove = &form.buttons[4];
    let confirm = remove.confirm.as_ref().expect("Remove asks first");
    assert_eq!(
        (
            confirm.text.as_str(),
            confirm.confirm.as_str(),
            confirm.cancel.as_str()
        ),
        (
            "Remove this endpoint from the project? This writes the config file and takes effect \
             after restart.",
            "Confirm remove",
            "Cancel"
        )
    );
    let Op::Rust { action, payload } = &remove.op else {
        panic!("Remove is a Rust action");
    };
    assert_eq!(action, "connection.remove");
    assert_eq!(payload["connection"], json!("endpoint:modbus"));

    edited.disabled = true;
    let form = edit(&edited);
    assert_eq!(
        button_ids(&form.buttons),
        ["enable", "cancel", "test", "remove", "apply_live"]
    );
    assert_eq!(form.buttons[0].label, "Enable");
    assert_eq!(
        form.note.as_deref(),
        Some(
            "This endpoint is disabled. Use Enable to turn it back on; restart the runtime to \
             apply the change."
        )
    );
}

#[test]
fn form_identity_follows_protocol_runtime_and_prefill() {
    let request = |prefill: Value| FormRequest {
        protocol: Some("modbus_tcp".to_owned()),
        runtime: Some("runtime:local".to_owned()),
        prefill: Some(values(prefill)),
        ..FormRequest::default()
    };
    let plain = form_id(&request(json!({})));
    assert_eq!(plain, "modbus_tcp@runtime:local");
    let first = form_id(&request(json!({ "unit_id": 1, "address": "10.0.0.9:502" })));
    let same = form_id(&request(json!({ "address": "10.0.0.9:502", "unit_id": 1 })));
    let other = form_id(&request(
        json!({ "address": "10.0.0.10:502", "unit_id": 1 }),
    ));
    assert_eq!(
        first, same,
        "the identity does not depend on the order of the values"
    );
    assert_ne!(first, other);
    assert!(first.starts_with("modbus_tcp@runtime:local~"));
}

/// Catches a route login that keeps its password after a post, and an MQTT form that forgets a
/// password which a refused Save would then lose.
#[test]
fn only_a_route_login_is_forgotten_after_a_post() {
    let route = RouteForm {
        pane: "browse:ads".to_owned(),
        runtime: "runtime:local".to_owned(),
        route_name: "PLC_ROUTE".to_owned(),
        target: json!({ "ip": "192.168.10.20" }),
        local: json!({}),
    };
    let login = route_login_form(&route);
    assert!(field(&login, "username").forget_after_post);
    assert!(field(&login, "password").forget_after_post);
    let mqtt = new_form(&catalog(), "mqtt", true);
    assert_eq!(field(&mqtt, "password").kind, FieldKind::Secret);
    assert!(
        mqtt.sections
            .iter()
            .flat_map(|section| &section.fields)
            .all(|field| !field.forget_after_post),
        "an MQTT form keeps every value after a post"
    );
}
