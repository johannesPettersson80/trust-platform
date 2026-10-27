//! Tests of the devices that settings name (specification 11, "Compare with the project", "The project's
//! devices"). Every expected value is a literal worked out from the specification.

use std::net::IpAddr;

use serde_json::{json, Value};

use super::*;

fn devices(protocol: &str, params: &Value) -> Vec<ConfiguredDevice> {
    ConfiguredDevice::from_connection(protocol, params).expect("the settings name devices")
}

fn refusal(protocol: &str, params: &Value) -> String {
    ConfiguredDevice::from_connection(protocol, params)
        .expect_err("the settings name no device the way the runtime reads them")
}

fn ip(text: &str) -> IpAddr {
    text.parse().expect("an IP address")
}

#[test]
fn a_modbus_driver_names_its_address_unit_and_stated_identity() {
    // Specification 11, "The project's devices": a `modbus-tcp` driver is named by `address` and `unit_id` (1
    // when absent), and it may state `vendor_name`, `product_code` and `revision`, which are text. A wrong
    // implementation defaults the unit to 0, keeps the padding of a stated value, or reads an empty value as
    // stated.
    let plain = devices("modbus_tcp", &json!({ "address": "10.0.0.9:502" }));
    assert_eq!(plain.len(), 1);
    assert_eq!(
        Value::Object(plain[0].facts()),
        json!({ "address": "10.0.0.9:502", "unit_id": 1 })
    );

    let stated = devices(
        "modbus_tcp",
        &json!({
            "address": "10.0.0.9:502", "unit_id": 7,
            "vendor_name": "ACME", "product_code": " DRV-1 ", "revision": ""
        }),
    );
    assert_eq!(
        Value::Object(stated[0].facts()),
        json!({
            "address": "10.0.0.9:502", "unit_id": 7,
            "vendor_name": "ACME", "product_code": "DRV-1"
        })
    );

    assert!(
        refusal("modbus_tcp", &json!({ "address": "plc.local:502" }))
            .contains("not an IP address and a port")
    );
    assert!(refusal(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "unit_id": 256 })
    )
    .contains("unit_id"));
    assert!(refusal(
        "modbus_tcp",
        &json!({ "address": "10.0.0.9:502", "product_code": 5 })
    )
    .contains("product_code must be text"));
}

#[test]
fn an_ethercat_driver_names_one_device_per_module() {
    // Specification 11, "The project's devices": each module of an `ethercat` driver is a device named by the
    // driver's adapter (`mock` when absent) and the module's model, in upper case, at its `slot`, which is its
    // index when the module names none; a driver without `modules` has the driver's default chain. A wrong
    // implementation takes the adapter from nowhere, keeps the model's case, or invents its own default chain.
    let named = devices(
        "ethercat",
        &json!({ "adapter": "eth1", "modules": [
            { "model": "ek1100" }, { "model": "el1008" }, { "model": "el2008", "slot": 5 }
        ] }),
    );
    let facts: Vec<Value> = named
        .iter()
        .map(|device| Value::Object(device.facts()))
        .collect();
    assert_eq!(
        facts,
        vec![
            json!({ "adapter": "eth1", "position": 0, "model": "EK1100" }),
            json!({ "adapter": "eth1", "position": 1, "model": "EL1008" }),
            json!({ "adapter": "eth1", "position": 5, "model": "EL2008" }),
        ]
    );

    let default_chain = devices("ethercat", &json!({}));
    let facts: Vec<Value> = default_chain
        .iter()
        .map(|device| Value::Object(device.facts()))
        .collect();
    assert_eq!(
        facts,
        vec![
            json!({ "adapter": "mock", "position": 0, "model": "EK1100" }),
            json!({ "adapter": "mock", "position": 1, "model": "EL1008" }),
            json!({ "adapter": "mock", "position": 2, "model": "EL2008" }),
        ]
    );

    let error = refusal("ethercat", &json!({ "modules": [{ "model": "EL3001" }] }));
    assert!(
        error.starts_with("ethercat: ") && error.contains("EL3001"),
        "{error}"
    );
}

#[test]
fn an_mqtt_broker_reads_its_scheme_host_and_port() {
    // Specification 11, "The project's devices": `broker` is `host:port` with an optional `tcp://`,
    // `mqtt://`, `ssl://` or `mqtts://` in front; the host is an address, `localhost`, or a name in lower case.
    // A wrong implementation keeps the scheme in the host, or accepts a broker without a port, which the
    // driver refuses.
    let read = |broker: &str| devices("mqtt", &json!({ "broker": broker }));
    assert_eq!(
        read("mqtt://10.0.0.5:1883"),
        vec![ConfiguredDevice::Mqtt(ConfiguredMqtt {
            host: Host::Ip(ip("10.0.0.5")),
            port: 1883
        })]
    );
    assert_eq!(
        read("mqtts://Broker.Local:8883"),
        vec![ConfiguredDevice::Mqtt(ConfiguredMqtt {
            host: Host::Name("broker.local".to_string()),
            port: 8883
        })]
    );
    assert_eq!(
        read("[::1]:1883")[0].facts().get("address"),
        Some(&json!("[::1]:1883"))
    );
    assert_eq!(
        read("LOCALHOST:1884"),
        vec![ConfiguredDevice::Mqtt(ConfiguredMqtt {
            host: Host::Localhost,
            port: 1884
        })]
    );
    assert!(refusal("mqtt", &json!({ "broker": "10.0.0.5" })).contains("the port is missing"));
    assert!(refusal("mqtt", &json!({ "broker": "10.0.0.5:0" })).contains("port"));
    assert!(refusal("mqtt", &json!({})).contains("broker is missing"));
}

#[test]
fn an_opcua_connection_reads_its_endpoint_host_port_and_security() {
    // Specification 11, "The project's devices": a connection is named by the host and port of its
    // `endpoint_url` (4840 when the URL names none) and states `security_policy` and `security_mode`. A
    // wrong implementation takes the URL path for the port, forgets the default port, or reads another
    // scheme.
    let list = devices(
        "opcua_client",
        &json!({ "connections": [
            { "endpoint_url": "opc.tcp://10.0.0.6/UA", "security_policy": "none", "security_mode": "none" },
            { "endpoint_url": "opc.tcp://[fe80::1]:4841/x", "security_policy": "basic256sha256",
              "security_mode": "sign_and_encrypt" }
        ] }),
    );
    let facts: Vec<Value> = list
        .iter()
        .map(|device| Value::Object(device.facts()))
        .collect();
    assert_eq!(
        facts,
        vec![
            json!({ "address": "10.0.0.6:4840", "security": "none / none" }),
            json!({ "address": "[fe80::1]:4841", "security": "basic256sha256 / sign_and_encrypt" }),
        ]
    );

    let one = devices(
        "opcua_client",
        &json!({ "endpoint_url": "opc.tcp://plc01:4840", "security_policy": "none", "security_mode": "none" }),
    );
    assert_eq!(
        one,
        vec![ConfiguredDevice::Opcua(ConfiguredOpcua {
            host: Host::Name("plc01".to_string()),
            port: 4840,
            security_policy: "none".to_string(),
            security_mode: "none".to_string(),
        })]
    );

    let wrong = json!({ "endpoint_url": "http://10.0.0.6:4840", "security_policy": "none", "security_mode": "none" });
    assert!(refusal("opcua_client", &wrong).contains("not an opc.tcp:// URL"));
    let bare = json!({ "endpoint_url": "opc.tcp://10.0.0.6:4840", "security_mode": "none" });
    assert!(refusal("opcua_client", &bare).contains("security_policy is missing"));
}

#[test]
fn an_ads_connection_reads_its_net_id_and_host_from_a_list_or_one_object() {
    // Specification 11, "The project's devices": a connection is named by `target_net_id` and states `host`;
    // the topology reports the connections as a list and a single connection is read as itself. A wrong
    // implementation reads only the top level, so that a runtime's own report names no device.
    let list = devices(
        "ads",
        &json!({ "connections": [
            { "name": "a", "target_net_id": "5.80.201.232.1.1", "host": "10.0.0.5", "ams_port": 851 },
            { "name": "b", "target_net_id": "5.80.201.233.1.1", "host": "10.0.0.6", "ams_port": 301 }
        ] }),
    );
    assert_eq!(list.len(), 2);
    assert_eq!(
        list[1],
        ConfiguredDevice::Ads(ConfiguredAds {
            ams_net_id: "5.80.201.233.1.1".to_string(),
            host: "10.0.0.6".to_string(),
        })
    );

    let flat = devices(
        "ads",
        &json!({ "ams_net_id": "5.1.2.3.1.1", "ip": "10.0.0.7" }),
    );
    assert_eq!(
        flat,
        vec![ConfiguredDevice::Ads(ConfiguredAds {
            ams_net_id: "5.1.2.3.1.1".to_string(),
            host: "10.0.0.7".to_string(),
        })]
    );
    assert!(devices("ads", &json!({})).is_empty());
    assert!(refusal("ads", &json!({ "host": "10.0.0.7" })).contains("target_net_id is missing"));
}

#[test]
fn a_driver_alias_names_its_devices_under_the_topology_spelling() {
    // Specification 11, "The project's devices" and the driver registry (§6.5): a driver may be written
    // under an alias (`mqtt-tcp`, `ether-cat`, `ecat`), and the topology then names its protocol `mqtt_tcp`,
    // `ether_cat` or `ecat`. These name the devices of the canonical protocol. A wrong implementation reads
    // only the canonical spelling, so that the Discover pane calls a configured device unknown.
    let broker = json!({ "broker": "10.0.0.5:1883" });
    assert_eq!(devices("mqtt_tcp", &broker), devices("mqtt", &broker));
    let bus = json!({ "adapter": "eth1", "modules": [{ "model": "EL2008", "slot": 2 }] });
    for alias in ["ether_cat", "ecat"] {
        assert_eq!(devices(alias, &bus), devices("ethercat", &bus), "{alias}");
    }
    assert_eq!(devices("ecat", &bus).len(), 1);
}

#[test]
fn other_protocols_name_no_device() {
    // Specification 11, "The project's devices": `gpio`, `simulated` and `loopback` drivers, and every
    // other source, name no configured device. A wrong implementation lists a GPIO driver as a device.
    for protocol in ["gpio", "simulated", "loopback", "opcua", "usb"] {
        assert!(
            devices(protocol, &json!({ "chip": "gpiochip0" })).is_empty(),
            "{protocol}"
        );
    }
}

#[test]
fn a_host_is_placed_only_by_its_address_or_by_localhost() {
    // Specification 11, "Host names": an address is the host a search reports for it, `localhost` is
    // the loopback addresses, and any other name is the same name in any case and nothing else. A wrong
    // implementation takes `localhost` for 127.0.0.1 only, or a name for an address.
    let address = Host::parse("10.0.0.5").expect("a host");
    assert!(address.is("10.0.0.5") && !address.is("10.0.0.6") && !address.is("plc01"));
    let loopback = Host::parse("Localhost").expect("a host");
    assert_eq!(loopback, Host::Localhost);
    assert!(loopback.is("127.0.0.1") && loopback.is("::1") && loopback.is("localhost"));
    assert!(!loopback.is("10.0.0.5"));
    let name = Host::parse("PLC01").expect("a host");
    assert_eq!(name, Host::Name("plc01".to_string()));
    assert!(name.is("plc01") && name.is("PLC01") && !name.is("10.0.0.5"));
    assert!(name.is_unplaced_name() && !address.is_unplaced_name() && !loopback.is_unplaced_name());
    assert!(Host::parse("  ").is_err());
}

#[test]
fn a_search_is_directed_at_the_addresses_and_units_of_the_devices() {
    // Specification 11, "Command line" (`--compare`): the addresses of the Modbus TCP, MQTT, OPC UA and ADS
    // devices are known addresses, and only the Modbus unit IDs from 1 to 247 can be probed. A wrong
    // implementation offers unit 0 or 255, which the request refuses, or an address for an EtherCAT module.
    let modbus = |unit: u64| {
        devices(
            "modbus_tcp",
            &json!({ "address": "10.0.0.9:502", "unit_id": unit }),
        )
        .remove(0)
    };
    assert_eq!(modbus(1).target().as_deref(), Some("10.0.0.9:502"));
    assert_eq!(modbus(7).probe_unit_id(), Some(7));
    assert_eq!(modbus(247).probe_unit_id(), Some(247));
    for unit in [0, 248, 255] {
        assert_eq!(modbus(unit).probe_unit_id(), None, "unit {unit}");
        assert!(modbus(unit).unit_id_unsearchable(), "unit {unit}");
    }
    assert!(!modbus(1).unit_id_unsearchable());

    let mqtt = devices("mqtt", &json!({ "broker": "[::1]:1883" })).remove(0);
    assert_eq!(mqtt.target().as_deref(), Some("[::1]:1883"));
    let opcua = devices(
        "opcua_client",
        &json!({ "endpoint_url": "opc.tcp://plc01/UA", "security_policy": "none", "security_mode": "none" }),
    )
    .remove(0);
    assert_eq!(opcua.target().as_deref(), Some("plc01:4840"));
    assert!(opcua.named_by_unplaced_host() && !mqtt.named_by_unplaced_host());
    let ads = devices(
        "ads",
        &json!({ "target_net_id": "5.1.2.3.1.1", "host": "10.0.0.7" }),
    )
    .remove(0);
    assert_eq!(ads.target().as_deref(), Some("10.0.0.7"));
    let without_host = devices("ads", &json!({ "target_net_id": "5.1.2.3.1.1" })).remove(0);
    assert_eq!(without_host.target(), None);
    let module = devices("ethercat", &json!({ "adapter": "eth1" })).remove(0);
    assert_eq!(module.target(), None);
}
