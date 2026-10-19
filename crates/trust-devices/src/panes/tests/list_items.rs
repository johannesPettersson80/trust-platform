//! The list items that the catalog describes (specification 11; specification 25 §5.9): adding
//! an item gives one that the runtime's configuration parser accepts once it is filled in. Each
//! test fills in the added item, converts the form with `coerce_submission`, and has the runtime
//! check the result with `comm.apply` `validate`, which runs the parser on the patched
//! `runtime.toml` and writes nothing.

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_apply_json;

use super::add::library;
use crate::panes::forms::{
    coerce_submission, connection_form, Catalog, CatalogProtocol, FormInputs,
};
use crate::test_project::{runtime_toml, TempFolder};
use crate::view::types::{FieldError, FieldKind, ItemId, ListEditor, RuntimeRef};

fn catalog() -> Catalog {
    Catalog::parse(&library()).expect("the tool's own catalog")
}

fn protocol<'a>(catalog: &'a Catalog, id: &str) -> &'a CatalogProtocol {
    catalog
        .protocol(id)
        .unwrap_or_else(|| panic!("the protocol {id}"))
}

/// The editor of the list `list` in the form that adds a connection of `protocol`.
fn editor(catalog: &Catalog, protocol: &str, list: &str) -> ListEditor {
    let runtime = RuntimeRef {
        id: ItemId("runtime:local".to_owned()),
        name: "Simulator".to_owned(),
    };
    let inputs = FormInputs {
        catalog,
        runtime: &runtime,
        runtime_choices: &[],
        reachable: true,
        prefill: None,
        connection: None,
    };
    connection_form(protocol, &inputs)
        .expect("the catalog holds the protocol")
        .sections
        .iter()
        .flat_map(|section| &section.fields)
        .find(|field| field.id == list)
        .and_then(|field| field.list.clone())
        .unwrap_or_else(|| panic!("{protocol}.{list} has a list editor"))
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(values) => values,
        other => panic!("values are an object: {other}"),
    }
}

fn refused(field: &str, message: &str) -> Result<Map<String, Value>, Vec<FieldError>> {
    Err(vec![FieldError {
        field: field.to_owned(),
        message: message.to_owned(),
    }])
}

/// What the runtime says of the parameters `params` of `protocol`: `Ok` when its configuration
/// parser accepts them, else its answer.
fn runtime_validates(protocol: &str, params: &Map<String, Value>) -> Result<(), Value> {
    let project = TempFolder::new("list-items");
    project.write(
        "runtime.toml",
        &runtime_toml("line", "tcp://127.0.0.1:5680", Some("list-token")),
    );
    let answer = offline_comm_apply_json(
        &project.path,
        json!({ "protocol": protocol, "action": "validate", "params": params }),
    )
    .expect("the runtime answers");
    if answer["lifecycle_effect"] == "validate_only" && answer.get("field_errors").is_none() {
        Ok(())
    } else {
        Err(answer)
    }
}

/// `[[runtime.ads_server.clients]]`: `AdsServerClientSection` (`config/parser.rs`) requires
/// `ams_net_id`, and `parse_ads_server_client_source`
/// (`config/parser/validation/runtime/ads_server.rs`) requires one of `source_ip` and
/// `source_cidr`, refuses both, and accepts neither only with `allow_unpinned_clients`.
#[test]
fn allowed_client_item_round_trips_and_the_parser_accepts_it() {
    let catalog = catalog();
    let clients = editor(&catalog, "ads_server", "clients");
    assert!(clients.can_add);
    assert_eq!(clients.item_kind, FieldKind::Object);
    assert_eq!(
        clients.item_template,
        json!({ "ams_net_id": "", "source_ip": "" }),
        "a new client starts with its source IP"
    );
    let shown: Vec<(&str, &str, Vec<&str>)> = clients
        .properties
        .iter()
        .map(|property| {
            let alternatives = property.alternatives.iter().map(String::as_str).collect();
            (property.key.as_str(), property.label.as_str(), alternatives)
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("ams_net_id", "AMS Net ID", vec![]),
            ("source_ip", "Source IP", vec!["source_cidr"]),
            ("source_cidr", "Source CIDR", vec!["source_ip"]),
        ]
    );
    let ads_server = protocol(&catalog, "ads_server");
    let form = |clients: Value, unpinned: bool| {
        object(
            json!({ "enabled": true, "listen": "127.0.0.1", "insecure_transport": true,
                       "allow_unpinned_clients": unpinned, "clients": clients }),
        )
    };
    let submit =
        |clients: Value, unpinned: bool| coerce_submission(ads_server, &form(clients, unpinned));

    let mut client = clients.item_template.clone();
    assert_eq!(
        submit(json!([client]), false),
        refused("clients", "AMS Net ID is required.")
    );
    client["ams_net_id"] = json!("5.23.91.12.1.1");
    assert_eq!(
        submit(json!([client]), false),
        refused("clients", "Enter Source IP or Source CIDR.")
    );
    client["source_ip"] = json!("192.168.10.50");
    let params = submit(json!([client]), false).expect("a filled-in client");
    assert_eq!(params["clients"], json!([client]), "the item round-trips");
    assert_eq!(runtime_validates("ads_server", &params), Ok(()));

    // The alternative: a subnet instead of the address; the empty address is left out.
    let subnet = json!({ "ams_net_id": "5.23.91.12.1.1", "source_ip": "",
                         "source_cidr": "192.168.10.0/24" });
    let params = submit(json!([subnet]), false).expect("a client pinned to a subnet");
    assert_eq!(
        params["clients"],
        json!([{ "ams_net_id": "5.23.91.12.1.1", "source_cidr": "192.168.10.0/24" }])
    );
    assert_eq!(runtime_validates("ads_server", &params), Ok(()));

    // Both, or neither, are refused as the parser refuses them.
    let both = json!({ "ams_net_id": "5.23.91.12.1.1", "source_ip": "192.168.10.50",
                       "source_cidr": "192.168.10.0/24" });
    assert_eq!(
        submit(json!([both]), false),
        refused("clients", "Enter only one of Source IP and Source CIDR.")
    );
    assert!(runtime_validates("ads_server", &form(json!([both]), false)).is_err());
    let neither = json!({ "ams_net_id": "5.23.91.12.1.1" });
    assert!(runtime_validates("ads_server", &form(json!([neither]), false)).is_err());

    // While unpinned clients are allowed, neither is accepted, as the parser accepts it.
    let params = submit(json!([neither]), true).expect("an unpinned client");
    assert_eq!(runtime_validates("ads_server", &params), Ok(()));
}

/// `runtime.cloud.wan.allow_write`: `RuntimeCloudWanAllowRuleSection` (`config/parser.rs`)
/// requires `action` and `target`, and `parse_runtime_cloud_section`
/// (`config/parser/validation/runtime/transport.rs`) refuses an empty one.
#[test]
fn wan_write_rule_item_round_trips_and_the_parser_accepts_it() {
    let catalog = catalog();
    let rules = editor(&catalog, "runtime_cloud", "wan_allow_write");
    assert_eq!(rules.item_kind, FieldKind::Object);
    assert_eq!(rules.item_template, json!({ "action": "", "target": "" }));
    let required: Vec<(&str, bool)> = rules
        .properties
        .iter()
        .map(|property| (property.key.as_str(), property.required))
        .collect();
    assert_eq!(required, [("action", true), ("target", true)]);
    let runtime_cloud = protocol(&catalog, "runtime_cloud");
    let form = |rules: Value| object(json!({ "profile": "dev", "wan_allow_write": rules }));
    let submit = |rules: Value| coerce_submission(runtime_cloud, &form(rules));

    let mut rule = rules.item_template.clone();
    assert_eq!(
        submit(json!([rule])),
        refused("wan_allow_write", "Action is required.")
    );
    rule["action"] = json!("cfg_apply");
    assert_eq!(
        submit(json!([rule])),
        refused("wan_allow_write", "Target is required.")
    );
    assert!(
        runtime_validates("runtime_cloud", &form(json!([rule]))).is_err(),
        "the parser refuses the empty target too"
    );
    rule["target"] = json!("site-b/line-1");
    let params = submit(json!([rule])).expect("a filled-in rule");
    assert_eq!(
        params["wan_allow_write"],
        json!([rule]),
        "the item round-trips"
    );
    assert_eq!(runtime_validates("runtime_cloud", &params), Ok(()));
}

/// `runtime.cloud.links.transports`: `RuntimeCloudLinkPreferenceRuleSection`
/// (`config/parser.rs`) requires all three, `parse_runtime_cloud_section` refuses an empty source
/// or target, and `RuntimeCloudPreferredTransport::parse` (`config/contracts.rs`) refuses any
/// other transport.
#[test]
fn link_transport_item_round_trips_and_the_parser_accepts_it() {
    let catalog = catalog();
    let transports = editor(&catalog, "runtime_cloud", "link_transports");
    assert_eq!(
        transports.item_template,
        json!({ "source": "", "target": "", "transport": "" })
    );
    let transport = transports
        .properties
        .iter()
        .find(|property| property.key == "transport")
        .expect("the transport property");
    assert_eq!(transport.kind, FieldKind::Choice);
    assert!(transport.required);
    let options: Vec<&str> = transport
        .options
        .iter()
        .map(|option| option.value.as_str())
        .collect();
    assert_eq!(
        options,
        [
            "realtime",
            "zenoh",
            "mesh",
            "mqtt",
            "modbus-tcp",
            "opcua",
            "discovery",
            "web"
        ]
    );
    let runtime_cloud = protocol(&catalog, "runtime_cloud");
    let form = |links: Value| object(json!({ "profile": "dev", "link_transports": links }));
    let submit = |links: Value| coerce_submission(runtime_cloud, &form(links));

    let mut link = transports.item_template.clone();
    assert_eq!(
        submit(json!([link])),
        refused("link_transports", "Source is required.")
    );
    link["source"] = json!("line-1");
    link["target"] = json!("line-2");
    assert_eq!(
        submit(json!([link])),
        refused("link_transports", "Transport is required.")
    );
    link["transport"] = json!("carrier-pigeon");
    assert_eq!(
        submit(json!([link])),
        refused("link_transports", "Choose a listed value for Transport.")
    );
    assert!(
        runtime_validates("runtime_cloud", &form(json!([link]))).is_err(),
        "the parser refuses that transport too"
    );
    link["transport"] = json!("zenoh");
    let params = submit(json!([link])).expect("a filled-in link transport");
    assert_eq!(
        params["link_transports"],
        json!([link]),
        "the item round-trips"
    );
    assert_eq!(runtime_validates("runtime_cloud", &params), Ok(()));
}

/// `runtime.realtime.cpu_affinity`: `RealtimeSection::cpu_affinity` (`config/parser.rs`) is a
/// list of unsigned integers, which `LinuxRtConfig::validate` (`host/linux_rt.rs`) requires to
/// be unique.
#[test]
fn cpu_index_item_round_trips_and_the_parser_accepts_it() {
    let catalog = catalog();
    let cpus = editor(&catalog, "realtime_t0", "cpu_affinity");
    assert!(cpus.can_add);
    assert_eq!(cpus.item_kind, FieldKind::Number, "not a text item");
    assert_eq!(cpus.item_template, json!(""), "a new CPU index is empty");
    let realtime = protocol(&catalog, "realtime_t0");
    let form = |cpus: Value| {
        object(
            json!({ "enabled": true, "scheduler": "fifo", "priority": 70,
                       "cpu_affinity": cpus }),
        )
    };
    let submit = |cpus: Value| coerce_submission(realtime, &form(cpus));

    for entry in [
        cpus.item_template.clone(),
        json!("two"),
        json!("-1"),
        json!(2.5),
    ] {
        assert_eq!(
            submit(json!([entry])),
            refused("cpu_affinity", "Enter a whole number of 0 or more."),
            "{entry}"
        );
    }
    assert!(
        runtime_validates("realtime_t0", &form(json!(["2"]))).is_err(),
        "the parser refuses a CPU index as text"
    );
    let params = submit(json!(["2", 3])).expect("filled-in CPU indexes");
    assert_eq!(
        params["cpu_affinity"],
        json!([2, 3]),
        "the items round-trip as numbers"
    );
    assert_eq!(runtime_validates("realtime_t0", &params), Ok(()));
}
