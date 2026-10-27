//! Tests of the comparison with the project (specification 11, "Compare with the project"). Every
//! expected value is a literal worked out from the specification.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::*;
use crate::connectors::DiscoveryConfidence;
use crate::search::model::{
    AdsIdentity, GpioIdentity, ModbusIdentity, MqttIdentity, OpcuaIdentity,
};

fn find(protocol: SourceId, identity: Identity, params: Value) -> ProtocolFind {
    ProtocolFind::new(
        protocol,
        "test",
        DiscoveryConfidence::Confirmed,
        identity,
        params,
        1,
    )
}

fn device(id: &str, addresses: &[&str], finds: Vec<ProtocolFind>) -> FoundDevice {
    FoundDevice {
        id: id.to_string(),
        addresses: addresses
            .iter()
            .map(|address| (*address).to_string())
            .collect(),
        ams_net_ids: Vec::new(),
        usb_serials: Vec::new(),
        runtime_ids: Vec::new(),
        last_seen_ms: 1,
        finds,
    }
}

/// A Modbus TCP device at `address` (`ip:port`) answering `unit_id`, with the identity it reports.
fn modbus_device(
    address: &str,
    unit_id: u8,
    reported: (Option<&str>, Option<&str>, Option<&str>),
) -> FoundDevice {
    let host = address.rsplit_once(':').map_or(address, |(host, _)| host);
    let identity = Identity::Modbus(ModbusIdentity {
        unit_id,
        vendor_name: reported.0.map(str::to_string),
        product_code: reported.1.map(str::to_string),
        revision: reported.2.map(str::to_string),
    });
    device(
        &format!("dev:ip:{host}"),
        &[host],
        vec![find(
            SourceId::ModbusTcp,
            identity,
            json!({ "address": address }),
        )],
    )
}

/// An EtherCAT SubDevice named `model` at `position` of `adapter`, a device of its own.
fn subdevice(adapter: &str, position: u16, model: &str) -> FoundDevice {
    let identity = Identity::Ethercat(EthercatIdentity {
        adapter: adapter.to_string(),
        slot: position,
        configured_address: position,
        alias: 0,
        model: model.to_string(),
        channels: 8,
        vendor_id: 2,
        product_id: 0,
        revision: 0,
        serial: 0,
        input_bytes: 0,
        output_bytes: 0,
    });
    device(
        &format!("dev:ethercat:{adapter}:{position}"),
        &[],
        vec![find(SourceId::Ethercat, identity, json!({}))],
    )
}

/// An MQTT broker found at `broker` (`ip:port`).
fn mqtt_device(broker: &str) -> FoundDevice {
    let host = broker.rsplit_once(':').map_or(broker, |(host, _)| host);
    device(
        &format!("dev:ip:{host}"),
        &[host],
        vec![find(
            SourceId::Mqtt,
            Identity::Mqtt(MqttIdentity {
                tls: false,
                login_required: None,
            }),
            json!({ "broker": broker }),
        )],
    )
}

/// An OPC UA server at `host` with one endpoint per `(url, policy, mode)`.
fn opcua_device(host: &str, endpoints: &[(&str, &str, &str)]) -> FoundDevice {
    let finds = endpoints
        .iter()
        .map(|(url, policy, mode)| {
            find(
                SourceId::Opcua,
                Identity::Opcua(OpcuaIdentity {
                    endpoint_url: (*url).to_string(),
                    security_policy: Some((*policy).to_string()),
                    security_mode: Some((*mode).to_string()),
                    application_name: None,
                    application_uri: None,
                    product_uri: None,
                    unsupported: false,
                    certificate_sha256: None,
                }),
                json!({}),
            )
        })
        .collect();
    device(&format!("dev:ip:{host}"), &[host], finds)
}

fn configured(protocol: &str, params: &Value) -> Vec<ConfiguredDevice> {
    ConfiguredDevice::from_connection(protocol, params).expect("the settings name devices")
}

fn ethercat_bus(adapter: &str, modules: &[(&str, u16)]) -> Vec<ConfiguredDevice> {
    let modules: Vec<Value> = modules
        .iter()
        .map(|(model, slot)| json!({ "model": model, "slot": slot }))
        .collect();
    configured(
        "ethercat",
        &json!({ "adapter": adapter, "modules": modules }),
    )
}

fn nothing_ignored() -> BTreeSet<String> {
    BTreeSet::new()
}

#[test]
fn compare_lists_missing_unused_and_differing() {
    // Specification 11, "Compare with the project" ("Reply" and its example): a configured Modbus device
    // that was not found is `missing`, a found Modbus device that no driver names is `unused`, and an
    // EtherCAT EL2008 at position 3 where the project has it at position 2 `differs`. The Modbus device
    // that both agree on appears in no list. A wrong implementation lists the matched device as unused,
    // reports the moved module as a missing module and an unused one, or sorts the lists differently.
    let mut project = configured("modbus_tcp", &json!({ "address": "10.0.0.9:502" }));
    project.extend(configured(
        "modbus_tcp",
        &json!({ "address": "10.0.0.5:502", "unit_id": 1 }),
    ));
    project.extend(ethercat_bus("eth1", &[("EK1100", 0), ("EL2008", 2)]));
    let found = [
        modbus_device("10.0.0.5:502", 1, (None, None, None)),
        modbus_device("10.0.0.12:502", 1, (None, None, None)),
        subdevice("eth1", 0, "EK1100"),
        subdevice("eth1", 3, "EL2008"),
    ];
    let searched = [SourceId::ModbusTcp, SourceId::Ethercat];

    let comparison = compare(&searched, &found, &project, &nothing_ignored());

    assert_eq!(
        comparison.report("s-3"),
        json!({
            "schema_version": 1,
            "search_id": "s-3",
            "missing": [
                { "protocol": "modbus_tcp", "configured": { "address": "10.0.0.9:502", "unit_id": 1 } }
            ],
            "unused": [
                { "device_id": "dev:ip:10.0.0.12", "protocols": ["modbus_tcp"] }
            ],
            "differs": [{
                "device_id": "dev:ethercat:eth1:3",
                "protocol": "ethercat",
                "configured": { "adapter": "eth1", "position": 2, "model": "EL2008" },
                "found": { "adapter": "eth1", "position": 3, "model": "EL2008" },
                "fields": ["position"]
            }],
            "not_compared": []
        })
    );
    let standings: Vec<(&str, Standing)> = comparison
        .devices
        .iter()
        .map(|(id, standing)| (id.as_str(), standing.standing))
        .collect();
    assert_eq!(
        standings,
        vec![
            ("dev:ethercat:eth1:0", Standing::InProject),
            ("dev:ethercat:eth1:3", Standing::InProject),
            ("dev:ip:10.0.0.12", Standing::NotInProject),
            ("dev:ip:10.0.0.5", Standing::InProject),
        ]
    );
}

#[test]
fn ethercat_position_change_is_a_difference() {
    // Specification 11, "Matching": a module and the SubDevice of the same model are paired when the
    // position differs, and the position differs. An EL2004 stands at position 2 and the EL2008 that the
    // project has at position 2 stands at 3. The EL2004 is unused; nothing is missing. A wrong
    // implementation that matches by position alone reports the EL2008 as missing.
    let project = ethercat_bus("eth1", &[("EK1100", 0), ("EL1008", 1), ("EL2008", 2)]);
    let found = [
        subdevice("eth1", 0, "EK1100"),
        subdevice("eth1", 1, "EL1008"),
        subdevice("eth1", 2, "EL2004"),
        subdevice("eth1", 3, "EL2008"),
    ];

    let comparison = compare(&[SourceId::Ethercat], &found, &project, &nothing_ignored());

    let report = comparison.report("s-1");
    assert_eq!(report["missing"], json!([]));
    assert_eq!(
        report["differs"],
        json!([{
            "device_id": "dev:ethercat:eth1:3",
            "protocol": "ethercat",
            "configured": { "adapter": "eth1", "position": 2, "model": "EL2008" },
            "found": { "adapter": "eth1", "position": 3, "model": "EL2008" },
            "fields": ["position"]
        }])
    );
    assert_eq!(
        report["unused"],
        json!([{ "device_id": "dev:ethercat:eth1:2", "protocols": ["ethercat"] }])
    );
}

#[test]
fn ethercat_model_change_at_a_position_is_a_difference() {
    // Specification 11, "Matching" (third step) and "Differences": with no free SubDevice of the
    // module's model, the module takes the SubDevice at its position, and the model differs. A wrong
    // implementation reports the EL2008 as missing and the EL2004 as unused, or compares no model.
    let project = ethercat_bus("eth1", &[("EK1100", 0), ("EL2008", 1)]);
    let found = [
        subdevice("eth1", 0, "EK1100"),
        subdevice("eth1", 1, "EL2004"),
    ];

    let comparison = compare(&[SourceId::Ethercat], &found, &project, &nothing_ignored());

    let report = comparison.report("s-1");
    assert_eq!(
        report["differs"],
        json!([{
            "device_id": "dev:ethercat:eth1:1",
            "protocol": "ethercat",
            "configured": { "adapter": "eth1", "position": 1, "model": "EL2008" },
            "found": { "adapter": "eth1", "position": 1, "model": "EL2004" },
            "fields": ["model"]
        }])
    );
    assert_eq!(report["missing"], json!([]));
    assert_eq!(report["unused"], json!([]));
}

#[test]
fn ethercat_pairs_the_nearest_position_and_never_across_adapters() {
    // Specification 11, "Matching": the modules are taken in the order of their positions, each takes the
    // free SubDevice of its model that is nearest (the lower position on a tie), and adapters stay apart.
    // The EL1008 modules at 1 and 3 take the SubDevices at 2 and 4 (not 1 to 4 and 3 to 2). The EL2008 of
    // eth1 finds only an EL2008 on eth2, so it is missing and that one is unused. A wrong implementation
    // pairs in the other order, or pairs across adapters.
    let project = ethercat_bus("eth1", &[("EL1008", 1), ("EL1008", 3), ("EL2008", 5)]);
    let found = [
        subdevice("eth1", 2, "EL1008"),
        subdevice("eth1", 4, "EL1008"),
        subdevice("eth2", 5, "EL2008"),
    ];

    let comparison = compare(&[SourceId::Ethercat], &found, &project, &nothing_ignored());

    let report = comparison.report("s-1");
    assert_eq!(
        report["differs"],
        json!([
            {
                "device_id": "dev:ethercat:eth1:2",
                "protocol": "ethercat",
                "configured": { "adapter": "eth1", "position": 1, "model": "EL1008" },
                "found": { "adapter": "eth1", "position": 2, "model": "EL1008" },
                "fields": ["position"]
            },
            {
                "device_id": "dev:ethercat:eth1:4",
                "protocol": "ethercat",
                "configured": { "adapter": "eth1", "position": 3, "model": "EL1008" },
                "found": { "adapter": "eth1", "position": 4, "model": "EL1008" },
                "fields": ["position"]
            }
        ])
    );
    assert_eq!(
        report["missing"],
        json!([{
            "protocol": "ethercat",
            "configured": { "adapter": "eth1", "position": 5, "model": "EL2008" }
        }])
    );
    assert_eq!(
        report["unused"],
        json!([{ "device_id": "dev:ethercat:eth2:5", "protocols": ["ethercat"] }])
    );

    // Two SubDevices as near as each other: the lower position is taken.
    let tie = ethercat_bus("eth1", &[("EL1008", 2)]);
    let sides = [
        subdevice("eth1", 1, "EL1008"),
        subdevice("eth1", 3, "EL1008"),
    ];
    let report = compare(&[SourceId::Ethercat], &sides, &tie, &nothing_ignored()).report("s-2");
    assert_eq!(
        report["differs"][0]["device_id"],
        json!("dev:ethercat:eth1:1")
    );
    assert_eq!(
        report["unused"],
        json!([{ "device_id": "dev:ethercat:eth1:3", "protocols": ["ethercat"] }])
    );
}

#[test]
fn modbus_product_code_change_is_a_difference() {
    // Specification 11, "Differences": a driver that states `vendor_name`, `product_code` or `revision`
    // expects it, and the device differs when it reports another value, or none. Text compares after
    // trimming, and exactly. The driver below expects ACME, DRV-100 and 1.2; the device reports ACME,
    // DRV-200 and no revision. A wrong implementation compares only the vendor, ignores a revision that the
    // device does not report, or calls a driver that states nothing different.
    let stated = configured(
        "modbus_tcp",
        &json!({
            "address": "10.0.0.9:502", "unit_id": 3,
            "vendor_name": "ACME", "product_code": "DRV-100", "revision": "1.2"
        }),
    );
    let drive = modbus_device("10.0.0.9:502", 3, (Some("ACME"), Some("DRV-200"), None));

    let comparison = compare(
        &[SourceId::ModbusTcp],
        std::slice::from_ref(&drive),
        &stated,
        &nothing_ignored(),
    );

    assert_eq!(
        comparison.report("s-1")["differs"],
        json!([{
            "device_id": "dev:ip:10.0.0.9",
            "protocol": "modbus_tcp",
            "configured": {
                "address": "10.0.0.9:502", "unit_id": 3,
                "vendor_name": "ACME", "product_code": "DRV-100", "revision": "1.2"
            },
            "found": {
                "address": "10.0.0.9:502", "unit_id": 3,
                "vendor_name": "ACME", "product_code": "DRV-200", "revision": null
            },
            "fields": ["product_code", "revision"]
        }])
    );

    // A driver that states nothing cannot differ, whatever the device reports.
    let silent = configured(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "unit_id": 3 }),
    );
    let comparison = compare(
        &[SourceId::ModbusTcp],
        std::slice::from_ref(&drive),
        &silent,
        &nothing_ignored(),
    );
    assert_eq!(comparison.report("s-1")["differs"], json!([]));

    // Trimmed, and exact: " DRV-100 " is DRV-100, `drv-100` is not.
    let code = configured(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "unit_id": 3, "product_code": "DRV-100" }),
    );
    let padded = modbus_device("10.0.0.9:502", 3, (None, Some(" DRV-100 "), None));
    let lower = modbus_device("10.0.0.9:502", 3, (None, Some("drv-100"), None));
    let same = compare(&[SourceId::ModbusTcp], &[padded], &code, &nothing_ignored());
    let other = compare(&[SourceId::ModbusTcp], &[lower], &code, &nothing_ignored());
    assert_eq!(same.report("s-1")["differs"], json!([]));
    assert_eq!(
        other.report("s-1")["differs"][0]["fields"],
        json!(["product_code"])
    );
}

#[test]
fn a_modbus_device_is_named_by_address_port_and_unit() {
    // Specification 11, "Matching": the result's address (IP address and port) and unit ID equal the
    // driver's. The driver names 10.0.0.9:502 unit 1. A device at the same address answering unit 2, and one
    // at port 1502, are not it: the driver's device is missing and both are unused. A wrong implementation
    // matches by IP address alone.
    let project = configured(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "unit_id": 1 }),
    );
    let other_unit = modbus_device("10.0.0.9:502", 2, (None, None, None));
    let mut other_port = modbus_device("10.0.0.9:1502", 1, (None, None, None));
    other_port.id = "dev:ip:10.0.0.9:1502".to_string();

    let report = compare(
        &[SourceId::ModbusTcp],
        &[other_unit, other_port],
        &project,
        &nothing_ignored(),
    )
    .report("s-1");

    assert_eq!(
        report["missing"],
        json!([{ "protocol": "modbus_tcp", "configured": { "address": "10.0.0.9:502", "unit_id": 1 } }])
    );
    assert_eq!(
        report["unused"],
        json!([
            { "device_id": "dev:ip:10.0.0.9", "protocols": ["modbus_tcp"] },
            { "device_id": "dev:ip:10.0.0.9:1502", "protocols": ["modbus_tcp"] }
        ])
    );
}

#[test]
fn an_ignored_device_is_not_listed_as_unused() {
    // Decision O14 and specification 11, "Ignored devices": a device that the project ignores is never
    // `unused`, yet it still matches, so the configured device that an ignored result answers is not
    // missing. A wrong implementation lists the ignored device, drops its match, or forgets the
    // device's standing.
    let strange = modbus_device("10.0.0.12:502", 1, (None, None, None));
    let ignored = BTreeSet::from(["dev:ip:10.0.0.12".to_string()]);

    let hidden = compare(
        &[SourceId::ModbusTcp],
        std::slice::from_ref(&strange),
        &[],
        &ignored,
    );
    assert_eq!(hidden.report("s-1")["unused"], json!([]));
    assert_eq!(
        hidden.devices["dev:ip:10.0.0.12"].standing,
        Standing::NotInProject
    );
    assert!(hidden.devices["dev:ip:10.0.0.12"].ignored);

    let listed = compare(
        &[SourceId::ModbusTcp],
        std::slice::from_ref(&strange),
        &[],
        &nothing_ignored(),
    );
    assert_eq!(
        listed.report("s-1")["unused"],
        json!([{ "device_id": "dev:ip:10.0.0.12", "protocols": ["modbus_tcp"] }])
    );

    let project = configured("modbus_tcp", &json!({ "address": "10.0.0.12:502" }));
    let matched = compare(&[SourceId::ModbusTcp], &[strange], &project, &ignored);
    assert_eq!(matched.report("s-1")["missing"], json!([]));
    assert_eq!(
        matched.devices["dev:ip:10.0.0.12"].standing,
        Standing::InProject
    );
}

#[test]
fn a_configured_device_that_the_search_could_not_look_for_is_not_called_missing() {
    // Specification 11, "Not compared": a device whose source did not run is `not_searched`, and a Modbus
    // unit ID outside 1-247, which no search probes, is `unit_id`; neither is missing. While a Modbus device
    // is not compared for its unit ID, a Modbus result that matches nothing is not unused either (standing
    // `Undecided`). A wrong implementation calls every EtherCAT module missing after a search from a
    // computer, which never runs EtherCAT.
    let mut project = ethercat_bus("eth1", &[("EL2008", 2)]);
    project.extend(configured(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "unit_id": 255 }),
    ));
    let bystander = modbus_device("10.0.0.20:502", 1, (None, None, None));

    let comparison = compare(
        &[SourceId::ModbusTcp],
        std::slice::from_ref(&bystander),
        &project,
        &nothing_ignored(),
    );

    let report = comparison.report("s-1");
    assert_eq!(report["missing"], json!([]));
    assert_eq!(
        report["not_compared"],
        json!([
            {
                "protocol": "modbus_tcp",
                "reason": "unit_id",
                "configured": { "address": "10.0.0.9:502", "unit_id": 255 }
            },
            {
                "protocol": "ethercat",
                "reason": "not_searched",
                "configured": { "adapter": "eth1", "position": 2, "model": "EL2008" }
            }
        ])
    );
    assert_eq!(report["unused"], json!([]));
    assert_eq!(
        comparison.devices["dev:ip:10.0.0.20"].standing,
        Standing::Undecided
    );
}

#[test]
fn a_configured_host_name_is_matched_by_name_and_never_looked_up() {
    // Specification 11, "Host names": a broker named `broker.plant` is not looked up, so it matches no
    // result that carries an address; it is `host_name`, and an MQTT result that matches nothing is
    // undecided, not unused. `localhost` matches the loopback addresses. An OPC UA server found under its
    // name matches that name. A wrong implementation lists the broker as missing and the result as unused.
    let mut project = configured("mqtt", &json!({ "broker": "broker.plant:1883" }));
    project.extend(configured(
        "mqtt",
        &json!({ "broker": "tcp://localhost:1884" }),
    ));
    project.extend(configured(
        "opcua_client",
        &json!({ "connections": [{
            "endpoint_url": "opc.tcp://PLC01:4840/UA",
            "security_policy": "none", "security_mode": "none"
        }] }),
    ));
    let found = [
        mqtt_device("10.0.0.5:1883"),
        mqtt_device("127.0.0.1:1884"),
        opcua_device("plc01", &[("opc.tcp://plc01:4840", "none", "none")]),
    ];

    let comparison = compare(
        &[SourceId::Mqtt, SourceId::Opcua],
        &found,
        &project,
        &nothing_ignored(),
    );

    let report = comparison.report("s-1");
    assert_eq!(report["missing"], json!([]));
    assert_eq!(report["unused"], json!([]));
    assert_eq!(report["differs"], json!([]));
    assert_eq!(
        report["not_compared"],
        json!([{
            "protocol": "mqtt",
            "reason": "host_name",
            "configured": { "address": "broker.plant:1883" }
        }])
    );
    assert_eq!(
        comparison.devices["dev:ip:10.0.0.5"].standing,
        Standing::Undecided
    );
    assert_eq!(
        comparison.devices["dev:ip:127.0.0.1"].standing,
        Standing::InProject
    );
    assert_eq!(
        comparison.devices["dev:ip:plc01"].standing,
        Standing::InProject
    );
}

#[test]
fn an_mqtt_broker_is_named_by_host_and_port() {
    // Specification 11, "Matching": the result's `broker` equals the driver's host and port. The project's
    // broker is 10.0.0.5:1883; one at port 8883 is another broker. A wrong implementation matches by host.
    let project = configured("mqtt", &json!({ "broker": "10.0.0.5:1883" }));
    let found = [mqtt_device("10.0.0.5:8883")];

    let report = compare(&[SourceId::Mqtt], &found, &project, &nothing_ignored()).report("s-1");

    assert_eq!(
        report["missing"],
        json!([{ "protocol": "mqtt", "configured": { "address": "10.0.0.5:1883" } }])
    );
    assert_eq!(
        report["unused"],
        json!([{ "device_id": "dev:ip:10.0.0.5", "protocols": ["mqtt"] }])
    );
}

#[test]
fn an_opcua_security_that_the_server_does_not_announce_is_a_difference() {
    // Specification 11, "Differences": a connection differs when none of the server's endpoints on its port
    // announces its policy together with its mode. The server announces none/none on port 4840 and
    // basic256sha256/sign on 4841; the connection wants basic256sha256/sign_and_encrypt on 4840. A wrong
    // implementation takes the endpoint of the other port, or compares the policy only.
    let project = configured(
        "opcua_client",
        &json!({ "connections": [{
            "endpoint_url": "opc.tcp://10.0.0.6:4840/UA",
            "security_policy": "basic256sha256", "security_mode": "sign_and_encrypt"
        }] }),
    );
    let server = opcua_device(
        "10.0.0.6",
        &[
            ("opc.tcp://10.0.0.6:4840", "none", "none"),
            ("opc.tcp://10.0.0.6:4840", "basic256sha256", "sign"),
            (
                "opc.tcp://10.0.0.6:4841",
                "basic256sha256",
                "sign_and_encrypt",
            ),
        ],
    );

    let report = compare(
        &[SourceId::Opcua],
        std::slice::from_ref(&server),
        &project,
        &nothing_ignored(),
    )
    .report("s-1");

    assert_eq!(
        report["differs"],
        json!([{
            "device_id": "dev:ip:10.0.0.6",
            "protocol": "opcua",
            "configured": { "address": "10.0.0.6:4840", "security": "basic256sha256 / sign_and_encrypt" },
            "found": { "address": "10.0.0.6:4840", "security": "none / none, basic256sha256 / sign" },
            "fields": ["security"]
        }])
    );

    let announced = opcua_device(
        "10.0.0.6",
        &[(
            "opc.tcp://10.0.0.6:4840",
            "basic256sha256",
            "sign_and_encrypt",
        )],
    );
    let agreed = compare(
        &[SourceId::Opcua],
        &[announced],
        &project,
        &nothing_ignored(),
    );
    assert_eq!(agreed.report("s-1")["differs"], json!([]));
}

#[test]
fn an_ads_device_is_named_by_its_ams_net_id_and_never_differs() {
    // Specification 11, "Matching" and "Differences": ADS matches by AMS Net ID, whatever address it
    // answers from, and nothing about it differs. The connection reaches 5.80.201.232.1.1 at 10.0.0.5; the
    // TwinCAT system answers from 192.168.1.7. A wrong implementation matches by host and calls it missing.
    let project = configured(
        "ads",
        &json!({ "connections": [
            { "target_net_id": "5.80.201.232.1.1", "host": "10.0.0.5", "ams_port": 851 },
            { "target_net_id": "5.80.201.233.1.1", "host": "10.0.0.6", "ams_port": 851 }
        ] }),
    );
    let mut system = device(
        "dev:netid:5.80.201.232.1.1",
        &["192.168.1.7"],
        vec![find(
            SourceId::Ads,
            Identity::Ads(AdsIdentity {
                ams_net_id: "5.80.201.232.1.1".to_string(),
                computer_name: None,
                twincat_version: None,
            }),
            json!({}),
        )],
    );
    system.ams_net_ids = vec!["5.80.201.232.1.1".to_string()];

    let comparison = compare(&[SourceId::Ads], &[system], &project, &nothing_ignored());

    let report = comparison.report("s-1");
    assert_eq!(
        report["missing"],
        json!([{
            "protocol": "ads",
            "configured": { "ams_net_id": "5.80.201.233.1.1", "address": "10.0.0.6" }
        }])
    );
    assert_eq!(report["differs"], json!([]));
    assert_eq!(report["unused"], json!([]));
    assert_eq!(
        comparison.devices["dev:netid:5.80.201.232.1.1"].matched,
        vec![SourceId::Ads]
    );
}

#[test]
fn results_of_other_protocols_are_not_compared() {
    // Specification 11, "The project's devices": GPIO, serial, USB, Bluetooth and truST runtimes name no
    // configured device, so their results carry no standing and are in no list. A wrong implementation
    // lists every GPIO line and USB port of the computer as a device the project does not use.
    let gpio = device(
        "dev:gpio:gpiochip0",
        &[],
        vec![find(
            SourceId::Gpio,
            Identity::Gpio(GpioIdentity {
                chip: "gpiochip0".to_string(),
                chip_label: None,
                line: 4,
                direction: None,
            }),
            json!({}),
        )],
    );

    let comparison = compare(&[SourceId::Gpio], &[gpio], &[], &nothing_ignored());

    assert!(comparison.devices.is_empty());
    assert_eq!(comparison.report("s-1")["unused"], json!([]));
}

#[test]
fn read_status_keeps_the_sources_that_ran_and_the_results_it_can_read() {
    // Specification 11, "One search" (the status) and specification 25 §2: a skipped source did not search,
    // a source or a result of a protocol this build does not know is left out, and the rest is read, even a
    // result that says nothing of its source, its simulation or its time, which the comparison does not
    // use. A wrong implementation counts a skipped source as searched, fails on the unknown protocol, or
    // drops the result that omits a member the comparison does not read.
    let status = json!({
        "sources": [
            { "id": "modbus_tcp", "state": "found", "count": 1 },
            { "id": "ethercat", "state": "skipped", "reason": "not here" },
            { "id": "ads", "state": "timed_out", "count": 0 },
            { "id": "fieldx", "state": "found", "count": 1 }
        ],
        "devices": [{
            "id": "dev:ip:10.0.0.5",
            "addresses": ["10.0.0.5"],
            "last_seen_ms": 5,
            "finds": [
                {
                    "protocol": "modbus_tcp", "confidence": "port_reachable",
                    "identity": { "kind": "modbus", "unit_id": 1 },
                    "params": { "address": "10.0.0.5:502" }
                },
                {
                    "protocol": "fieldx", "source": "x", "confidence": "confirmed",
                    "simulated": false, "last_seen_ms": 5,
                    "identity": { "kind": "fieldx" }, "params": {}
                }
            ]
        }]
    });

    let (searched, devices) = read_status(&status);

    assert_eq!(searched, vec![SourceId::ModbusTcp, SourceId::Ads]);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id, "dev:ip:10.0.0.5");
    assert_eq!(devices[0].finds.len(), 1);
    assert_eq!(devices[0].finds[0].protocol, SourceId::ModbusTcp);
}

#[test]
fn a_reason_names_the_reply_value_and_reads_in_the_words_of_specification_11() {
    // Specification 11, "Not compared": the three reasons are `not_searched`, `host_name` and
    // `unit_id`, in that order, and the command line and the Discover pane print one set of words for
    // them. A wrong implementation lists a reason twice, renames one in the reply, or lets the two
    // surfaces word a reason differently (the words live in `Reason::words` alone).
    let named: Vec<(&str, &str)> = Reason::ALL
        .into_iter()
        .map(|reason| (reason.as_str(), reason.words()))
        .collect();

    assert_eq!(
        named,
        vec![
            ("not_searched", "its source did not run in this search"),
            ("host_name", "its host name is not looked up"),
            ("unit_id", "a search cannot probe this unit ID"),
        ]
    );
}
