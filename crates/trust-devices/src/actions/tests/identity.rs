//! The expected identity of a Modbus TCP connection survives the panel (specification 11, "Compare with the
//! project", the project's devices): the form shows the vendor name, product code and revision that the
//! connection states, and a Save of that form writes them back. Every expected value is written out from the
//! specification.

use std::time::Instant;

use serde_json::{json, Value};
use trust_runtime::search::project_devices;

use super::config::{request, seen, CONNECTION, RUNTIME};
use crate::actions::act;
use crate::test_project::{runtime_toml, TempFolder};

/// The `fleet.topology` answer of the runtime `line` on `host:line-pc` whose `modbus_tcp` connection has the
/// settings `params`, as the runtime reports a driver's settings.
fn topology_with(params: &Value) -> Value {
    json!({
        "schema_version": 4,
        "hosts": [{
            "host_id": "host:line-pc", "hostname": "line-pc", "arch": "aarch64", "os": "linux",
            "ips": ["127.0.0.1"], "containers": [], "source": "config",
            "runtimes": [{
                "runtime_id": "line", "name": "line",
                "control_endpoint": "unix:///tmp/trust-runtime-line.sock", "mode": "stopped",
                "cycle_ms": 100, "health": "configured_policy",
                "detail": "Configured in project files; runtime is not running.",
                "source": "config",
                "endpoints": [{
                    "id": "endpoint:line:modbus_tcp", "kind": "field", "protocol": "modbus_tcp",
                    "name": "Modbus meter", "role": "client", "health": "configured_policy",
                    "detail": "Configured in io.toml; runtime is not running.",
                    "owned": true, "supports_test": true, "source": "config",
                    "params": params,
                }],
            }],
        }],
        "links": [], "shared": [], "external": [],
    })
}

#[test]
fn a_saved_modbus_connection_keeps_its_expected_identity() {
    // Specification 11, "Compare with the project": a `modbus-tcp` driver states the vendor name, product code
    // and revision it expects of its device, and the comparison reads them from the connections of the panel.
    // So the connection form must show the three with their saved values, and a Save of that form, as the
    // webview posts it with the address changed, must write them back. Here the connection at 10.0.0.5 states
    // Acme, AC-100 and 1.2; after the Save the project's devices are one Modbus device at 10.0.0.6, unit 2,
    // still stating Acme, AC-100 and 1.2. A wrong implementation leaves the three out of the form, so the Save
    // rewrites the driver without them and the next comparison compares against nothing.
    let project = TempFolder::new("expected-identity");
    project.write(
        "runtime.toml",
        &runtime_toml("line", "unix:///tmp/trust-runtime-line.sock", None),
    );
    project.write(
        "io.toml",
        concat!(
            "[io]\n",
            "safe_state = [{ address = \"%QW2\", value = \"4242\" }]\n",
            "\n",
            "[[io.drivers]]\n",
            "name = \"modbus-tcp\"\n",
            "\n",
            "[io.drivers.params]\n",
            "address = \"10.0.0.5:502\"\n",
            "unit_id = 2\n",
            "vendor_name = \"Acme\"\n",
            "product_code = \"AC-100\"\n",
            "revision = \"1.2\"\n",
        ),
    );
    let stated = json!({
        "address": "10.0.0.5:502", "unit_id": 2,
        "vendor_name": "Acme", "product_code": "AC-100", "revision": "1.2",
    });
    let (mut panel, _view) = seen(Some(topology_with(&stated)), Vec::new());
    let session = panel.session("dc-test", Instant::now());

    let opened = act(
        session,
        request(
            "form.open",
            json!({ "protocol": "modbus_tcp", "runtime": RUNTIME, "connection": CONNECTION }),
        ),
    );

    let form = opened.form.expect("form.open answers the form");
    let fields: Vec<_> = form
        .sections
        .iter()
        .flat_map(|section| &section.fields)
        .collect();
    let identity: Vec<(&str, &Value)> = fields
        .iter()
        .filter(|field| ["vendor_name", "product_code", "revision"].contains(&field.id.as_str()))
        .map(|field| (field.id.as_str(), &field.value))
        .collect();
    assert_eq!(
        identity,
        vec![
            ("vendor_name", &json!("Acme")),
            ("product_code", &json!("AC-100")),
            ("revision", &json!("1.2")),
        ],
        "the form shows what the connection states"
    );

    let mut save = request(
        "connection.save",
        json!({
            "form": form.id,
            "protocol": "modbus_tcp",
            "runtime": RUNTIME,
            "connection": CONNECTION,
        }),
    );
    for field in &fields {
        save.values.insert(field.id.clone(), field.value.clone());
    }
    save.values
        .insert("address".to_owned(), json!("10.0.0.6:502"));
    save.project = Some(project.path.clone());
    let saved = act(session, save);

    assert!(saved.ok, "the Save is applied: {:?}", saved.notice);
    let devices = project_devices::load(&project.path).expect("the project's files are readable");
    assert_eq!(
        devices
            .iter()
            .map(|device| Value::Object(device.facts()))
            .collect::<Vec<_>>(),
        vec![json!({
            "address": "10.0.0.6:502", "unit_id": 2,
            "vendor_name": "Acme", "product_code": "AC-100", "revision": "1.2",
        })],
        "the saved connection names its device with the identity it expected"
    );
}
