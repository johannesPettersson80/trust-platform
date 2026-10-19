//! The golden browse views: the JSON of each browse pane scenario and of the route login form, which
//! the new panel's render tests read (design §4.5 "D5: browse panes", and §3.2 rows J40 to J59). A
//! run with `TRUST_WRITE_DEVICES_BROWSE_FIXTURES` set writes them to
//! `crates/trust-devices/tests/fixtures/devices_browse/`; every other run compares them byte for
//! byte. The files are never written by hand: change a scenario here and run the fixture tool
//! again.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::browse::Browsed;
use crate::panes::browse::{self, Browse};
use crate::panes::forms::{route_login_form, RouteForm};
use crate::request::BrowseRequest;
use crate::view::types::{BrowseKind, ItemId};

/// Set to write the fixtures instead of comparing them.
const WRITE: &str = "TRUST_WRITE_DEVICES_BROWSE_FIXTURES";

/// The command that writes them.
const WRITE_COMMAND: &str = "TRUST_WRITE_DEVICES_BROWSE_FIXTURES=1 cargo test -p trust-devices --lib golden_browse_matches_the_fixtures";

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/devices_browse")
}

/// The fixture text of a value: pretty JSON with a final newline.
fn text(value: &Value) -> String {
    let json = serde_json::to_string_pretty(value).expect("a view serializes to JSON");
    format!("{json}\n")
}

/// The `devicesBrowse` body of a browse of the connection `endpoint:line`, as the panel sends it
/// (design §1.4.1). The host facts are left out, as the webview leaves them out of a pane's request.
fn browse_request(kind: &str, port: Option<u16>) -> BrowseRequest {
    let mut body = json!({
        "session": "dc-golden",
        "seq": 3,
        "source": { "connection": "endpoint:line", "connection_name": "Line" },
        "kind": kind,
        "trust_server_certificate": false,
    });
    if let Some(port) = port {
        body["port"] = json!(port);
    }
    serde_json::from_value(body).expect("a devicesBrowse body decodes")
}

/// The runtime that owns every scenario's connection.
fn owner() -> ItemId {
    ItemId("runtime:line".to_owned())
}

/// The view of a browse of `kind` over `protocol`, with the connection's `params`, projected from
/// the origin's wire `answer`; `None` is a browse that still runs.
fn project(
    kind: BrowseKind,
    protocol: &str,
    params: &Value,
    port: u16,
    request: &BrowseRequest,
    answer: Option<&Value>,
) -> Value {
    let params: Map<String, Value> = params
        .as_object()
        .cloned()
        .expect("a connection's params are an object");
    let owner = owner();
    let browsed = answer.map(Browsed::of);
    let view = browse::view(
        Browse {
            kind,
            protocol,
            name: "Line",
            params: &params,
            runtime: Some(&owner),
            port,
            request,
            cached: &[],
        },
        browsed.as_ref(),
    );
    serde_json::to_value(view).expect("a browse view serializes to JSON")
}

/// The OPC UA tree of a line: nested nodes, a point that is already added, and a leaf without a raw
/// `NodeId`, which is not selectable (specification 25 §11.2).
fn opcua_nodes() -> Value {
    let params = json!({
        "endpoint_url": "opc.tcp://192.168.10.5:4840",
        "points": [{ "node_id": "ns=2;s=Line.Speed" }],
    });
    let answer = json!({
        "schema_version": 1, "protocol": "opcua_client", "kind": "nodes",
        "tree": [{
            "id": "n:Line", "name": "Line", "path": "Line", "type": "Folder",
            "children": [
                {
                    "id": "n:Line.Speed", "name": "Speed", "path": "Line.Speed",
                    "node_id": "ns=2;s=Line.Speed", "type": "Double", "data_type": "Double",
                    "writable": true,
                },
                {
                    "id": "n:Line.Count", "name": "Count", "path": "Line.Count",
                    "node_id": "ns=2;s=Line.Count", "type": "Int32", "data_type": "Int32",
                    "writable": false,
                },
                {
                    "id": "n:Line.Alarms", "name": "Alarms", "path": "Line.Alarms", "type": "Folder",
                    "children": [
                        {
                            "id": "n:Line.Alarms.Fault", "name": "Fault",
                            "path": "Line.Alarms.Fault", "node_id": "ns=2;s=Line.Alarms.Fault",
                            "type": "Boolean", "data_type": "Boolean", "writable": false,
                        },
                        {
                            "id": "n:Line.Alarms.Anonymous", "name": "Anonymous",
                            "path": "Line.Alarms.Anonymous", "type": "String",
                            "data_type": "String", "writable": true,
                        },
                    ],
                },
            ],
        }],
    });
    project(
        BrowseKind::Nodes,
        "opcua_client",
        &params,
        0,
        &browse_request("nodes", None),
        Some(&answer),
    )
}

/// An OPC UA browse whose server certificate is not trusted: one error box with its one recovery.
fn opcua_untrusted_certificate() -> Value {
    let params = json!({ "endpoint_url": "opc.tcp://192.168.10.5:4840", "points": [] });
    let answer = json!({
        "schema_version": 1, "protocol": "opcua_client", "kind": "nodes", "tree": [],
        "error": { "code": "cert_untrusted", "message": "The server certificate is not trusted." },
    });
    project(
        BrowseKind::Nodes,
        "opcua_client",
        &params,
        0,
        &browse_request("nodes", None),
        Some(&answer),
    )
}

/// The `EtherCAT` channels of a terminal, one of them already selected.
fn ethercat_channels() -> Value {
    let params = json!({ "selected_channels": ["Term2.Output1"] });
    let answer = json!({
        "schema_version": 1, "protocol": "ethercat", "kind": "channels",
        "tree": [{
            "id": "ec:Term2", "name": "EL2008", "path": "Term2", "type": "Terminal",
            "children": [
                {
                    "id": "ec:Term2.Output1", "name": "Output 1", "path": "Term2.Output1",
                    "type": "BOOL", "writable": true,
                },
                {
                    "id": "ec:Term2.Output2", "name": "Output 2", "path": "Term2.Output2",
                    "type": "BOOL", "writable": false,
                },
            ],
        }],
    });
    project(
        BrowseKind::Channels,
        "ethercat",
        &params,
        0,
        &browse_request("channels", None),
        Some(&answer),
    )
}

/// The globals a server exposes: `Speed` is already exposed, `Count` is not.
fn expose_globals() -> Value {
    let params = json!({ "expose": ["Speed"] });
    let answer = json!({
        "schema_version": 1, "protocol": "opcua", "kind": "symbols",
        "tree": [
            {
                "id": "global:GVL.Speed", "name": "Speed", "path": "global.Speed",
                "type": "REAL", "writable": true,
            },
            {
                "id": "global:GVL.Count", "name": "Count", "path": "global.Count",
                "type": "DINT", "writable": false,
            },
        ],
    });
    project(
        BrowseKind::Expose,
        "opcua",
        &params,
        0,
        &browse_request("symbols", None),
        Some(&answer),
    )
}

/// The generated route plan of an ADS port: its route name, its `TwinCAT` target, this host and one
/// generated PowerShell file.
fn route_plan(port: u16) -> Value {
    json!({
        "route_name": "trust-runtime-line-controller-1",
        "target": {
            "name": "CX-1234", "ip": "192.168.10.5", "ams_net_id": "5.23.91.12.1.1",
            "ams_port": port, "tc_version": "3.1.4024",
        },
        "local": {
            "host_name": "line-controller-1", "chosen_ip": "192.168.10.20",
            "ams_net_id": "192.168.10.20.1.1", "nic": "eth0", "candidates": [],
            "classification": "lan",
        },
        "artifacts": [{
            "kind": "powershell", "label": "Download PowerShell",
            "filename": "add-ads-route-x.ps1", "content_type": "text/x-powershell",
            "content": "# generated",
        }],
    })
}

/// The ADS configuration of the target `CX-1234` on port 851.
fn ads_params(extra: &Value) -> Value {
    let mut params = json!({
        "name": "line1", "host": "192.168.10.5", "target_net_id": "5.23.91.12.1.1",
        "ams_port": 851, "points": [],
    });
    if let (Value::Object(base), Value::Object(more)) = (&mut params, extra) {
        for (key, value) in more {
            base.insert(key.clone(), value.clone());
        }
    }
    params
}

/// The ADS tag browser with three ports: 851 is browsed and has a tag already imported; 301 has no
/// route, so it shows the route help and `Create route`; 852 answered with an error.
fn ads_three_ports() -> Value {
    let params = ads_params(&json!({
        "imported_ads_symbols": [{ "port": 851, "paths": ["MAIN.Counter"] }],
        "ads_port_browse_results": [
            { "port": 301, "tree": [], "routeMissing": true, "routePlan": route_plan(301) },
            {
                "port": 852, "tree": [],
                "error": { "code": "ads_port_unavailable", "message": "ADS port 852 did not answer." },
            },
        ],
    }));
    let answer = json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols",
        "tree": [
            { "id": "ads:symbol:MAIN.Speed", "name": "Speed", "path": "MAIN.Speed", "type": "REAL", "writable": false },
            { "id": "ads:symbol:MAIN.Counter", "name": "Counter", "path": "MAIN.Counter", "type": "INT", "writable": true },
        ],
    });
    project(
        BrowseKind::AdsTags,
        "ads",
        &params,
        851,
        &browse_request("symbols", Some(851)),
        Some(&answer),
    )
}

/// The ADS tag browser of a target that a find reported on port 852, which is not browsed yet: its
/// card carries `Browse port 852`.
fn ads_port_to_browse() -> Value {
    let params = ads_params(&json!({ "responding_ads_ports": [852] }));
    let answer = json!({
        "schema_version": 1, "protocol": "ads", "kind": "symbols",
        "tree": [{
            "id": "ads:symbol:MAIN.Counter", "name": "Counter", "path": "MAIN.Counter",
            "type": "INT", "writable": true,
        }],
    });
    project(
        BrowseKind::AdsTags,
        "ads",
        &params,
        851,
        &browse_request("symbols", Some(851)),
        Some(&answer),
    )
}

/// The ADS tag browser while its browse of port 851 still runs.
fn ads_loading() -> Value {
    let params = ads_params(&json!({}));
    project(
        BrowseKind::AdsTags,
        "ads",
        &params,
        851,
        &browse_request("symbols", Some(851)),
        None,
    )
}

/// The login form of `Create route`: one user name, one password, and the button that posts
/// `ads.route_add`.
fn route_login() -> Value {
    let plan = route_plan(851);
    let route = RouteForm {
        pane: "browse:ads".to_owned(),
        runtime: "runtime:line".to_owned(),
        route_name: "trust-runtime-line-controller-1".to_owned(),
        target: plan["target"].clone(),
        local: plan["local"].clone(),
    };
    serde_json::to_value(route_login_form(&route)).expect("a form serializes to JSON")
}

/// Every scenario by its file name: one view each, built from the wire answers of the origins.
fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("opcua_nodes", opcua_nodes()),
        ("opcua_untrusted_certificate", opcua_untrusted_certificate()),
        ("ethercat_channels", ethercat_channels()),
        ("expose_globals", expose_globals()),
        ("ads_three_ports", ads_three_ports()),
        ("ads_port_to_browse", ads_port_to_browse()),
        ("ads_loading", ads_loading()),
        ("route_login_form", route_login()),
    ]
}

#[test]
fn golden_browse_matches_the_fixtures() {
    let directory = directory();
    let scenarios = all();
    if std::env::var_os(WRITE).is_some() {
        fs::create_dir_all(&directory).expect("create the fixture folder");
        for (name, value) in &scenarios {
            fs::write(directory.join(format!("{name}.json")), text(value))
                .expect("write a fixture");
        }
        return;
    }
    let mut problems = Vec::new();
    for (name, value) in &scenarios {
        let path = directory.join(format!("{name}.json"));
        match fs::read_to_string(&path) {
            Ok(fixture) if fixture == text(value) => {}
            Ok(_) => problems.push(format!("{name}.json differs from the view")),
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
        "the golden browse views in {} do not match:\n  {}\nWrite them with `{WRITE_COMMAND}` and review the diff.",
        directory.display(),
        problems.join("\n  ")
    );
}

#[test]
fn golden_browse_scenarios_are_the_same_every_time() {
    for ((name, first), (_, second)) in all().iter().zip(all()) {
        assert_eq!(
            text(first),
            text(&second),
            "{name} is the same view every time"
        );
    }
}
