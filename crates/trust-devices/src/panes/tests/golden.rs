//! The golden forms: the JSON of each form scenario, which the new panel's render tests read
//! (design §4.5 "D3: forms and the Add dialog", and §3 rows J13 to J19). A run with
//! `TRUST_WRITE_DEVICES_FORM_FIXTURES` set writes them to
//! `crates/trust-devices/tests/fixtures/devices_forms/`; every other run compares them byte for
//! byte. The files are never written by hand: change a scenario here and run the fixture tool
//! again.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use super::add::library;
use crate::panes::forms::{
    connection_form, setup_form, Catalog, EditedConnection, FormInputs, RuntimeSetup,
};
use crate::view::types::{Form, ItemId, RuntimeRef};

/// Set to write the fixtures instead of comparing them.
const WRITE: &str = "TRUST_WRITE_DEVICES_FORM_FIXTURES";

/// The command that writes them.
const WRITE_COMMAND: &str = "TRUST_WRITE_DEVICES_FORM_FIXTURES=1 cargo test -p trust-devices --lib golden_forms_match_the_fixtures";

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devices_forms")
}

/// The fixture text of a form: pretty JSON with a final newline.
fn text(form: &Form) -> String {
    let json = serde_json::to_string_pretty(form).expect("a form serializes to JSON");
    format!("{json}\n")
}

/// The tool's own catalog, as `trust-runtime comm schema` answers it.
fn catalog() -> Catalog {
    Catalog::parse(&library()).expect("the tool's own catalog")
}

/// The runtime that every new connection is added to.
fn simulator() -> RuntimeRef {
    RuntimeRef {
        id: ItemId("runtime:local".to_owned()),
        name: "Simulator".to_owned(),
    }
}

/// The form of a new `protocol` connection on the simulator, which answers when `reachable`.
fn new_form(protocol: &str, reachable: bool) -> Form {
    let catalog = catalog();
    let runtime = simulator();
    let inputs = FormInputs {
        catalog: &catalog,
        runtime: &runtime,
        runtime_choices: std::slice::from_ref(&runtime),
        reachable,
        prefill: None,
        connection: None,
    };
    connection_form(protocol, &inputs).expect("the catalog holds the protocol")
}

/// A connection's configuration as its `params`.
fn params(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("a connection's params are an object: {other}"),
    }
}

/// Every scenario by its file name: one form each, built with the tool's own catalog.
fn all() -> Vec<(&'static str, Form)> {
    vec![
        ("mqtt_new", new_form("mqtt", true)),
        ("gpio_new", new_form("gpio", true)),
        ("mesh_new", new_form("mesh", true)),
        ("ads_server_new", new_form("ads_server", true)),
        ("modbus_tcp_edit", modbus_tcp_edit()),
        ("modbus_tcp_prefilled", modbus_tcp_prefilled()),
        ("opcua_unreachable", new_form("opcua_client", false)),
        ("runtime_connect", setup_form(RuntimeSetup::Connect)),
    ]
}

/// A disabled Modbus TCP connection being edited: its values, and `Enable` and `Remove` in place of
/// `Save`.
fn modbus_tcp_edit() -> Form {
    let catalog = catalog();
    let runtime = simulator();
    let edited = EditedConnection {
        id: ItemId("endpoint:meter".to_owned()),
        protocol_name: "Modbus TCP".to_owned(),
        params: params(json!({
            "address": "192.168.1.50:502",
            "unit_id": 1,
            "input_start": 0,
            "output_start": 0,
            "timeout_ms": 500,
            "on_error": "warn",
        })),
        disabled: true,
    };
    let inputs = FormInputs {
        catalog: &catalog,
        runtime: &runtime,
        runtime_choices: std::slice::from_ref(&runtime),
        reachable: true,
        prefill: None,
        connection: Some(&edited),
    };
    connection_form("modbus_tcp", &inputs).expect("the catalog holds the protocol")
}

/// A new Modbus TCP connection prefilled from a find, with the draft tile.
fn modbus_tcp_prefilled() -> Form {
    let catalog = catalog();
    let runtime = simulator();
    let prefill = params(json!({ "address": "192.168.1.50:502", "unit_id": 1 }));
    let inputs = FormInputs {
        catalog: &catalog,
        runtime: &runtime,
        runtime_choices: std::slice::from_ref(&runtime),
        reachable: true,
        prefill: Some(&prefill),
        connection: None,
    };
    connection_form("modbus_tcp", &inputs).expect("the catalog holds the protocol")
}

#[test]
fn golden_forms_match_the_fixtures() {
    let directory = directory();
    let scenarios = all();
    if std::env::var_os(WRITE).is_some() {
        fs::create_dir_all(&directory).expect("create the fixture folder");
        for (name, form) in &scenarios {
            fs::write(directory.join(format!("{name}.json")), text(form)).expect("write a fixture");
        }
        return;
    }
    let mut problems = Vec::new();
    for (name, form) in &scenarios {
        let path = directory.join(format!("{name}.json"));
        match fs::read_to_string(&path) {
            Ok(fixture) if fixture == text(form) => {}
            Ok(_) => problems.push(format!("{name}.json differs from the form")),
            Err(error) => problems.push(format!("{name}.json cannot be read: {error}")),
        }
    }
    let names: BTreeSet<String> = scenarios
        .iter()
        .map(|(name, _)| format!("{name}.json"))
        .collect();
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file = entry.file_name().to_string_lossy().into_owned();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && !names.contains(&file)
            {
                problems.push(format!("{file} belongs to no scenario"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "the golden forms in {} do not match:\n  {}\nWrite them with `{WRITE_COMMAND}` and review the diff.",
        directory.display(),
        problems.join("\n  ")
    );
}

#[test]
fn golden_forms_are_deterministic_and_show_no_local_socket_path() {
    for ((name, first), (_, second)) in all().iter().zip(all()) {
        let shown = text(first);
        assert_eq!(shown, text(&second), "{name} is the same form every time");
        assert!(
            !shown.contains("/tmp/trust-"),
            "{name} shows a local socket's path"
        );
    }
}
