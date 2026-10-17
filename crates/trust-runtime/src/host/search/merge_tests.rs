use serde_json::json;

use super::*;
use crate::connectors::DiscoveryConfidence;
use crate::search::model::{Identity, ModbusIdentity, SourceId};

fn result(key: &str, keys: Vec<DeviceKey>, seen_ms: u64) -> ProtocolFind {
    ProtocolFind::new(
        SourceId::ModbusTcp,
        "tcp_connect",
        DiscoveryConfidence::PortReachable,
        Identity::Modbus(ModbusIdentity { unit_id: 1 }),
        json!({ "key": key }),
        seen_ms,
    )
    .keyed(key.to_string(), keys)
}

fn address(text: &str) -> DeviceKey {
    DeviceKey::Address(text.to_string())
}

fn net_id(text: &str) -> DeviceKey {
    DeviceKey::AmsNetId(text.to_string())
}

fn ids(devices: &Devices) -> Vec<String> {
    devices
        .snapshot(None)
        .0
        .into_iter()
        .map(|device| device.id)
        .collect()
}

#[test]
fn finds_merge_by_address_netid_usb_serial_or_runtime_id() {
    let mut devices = Devices::default();
    assert!(devices.add(result("a", vec![address("10.0.0.5")], 1)));
    assert!(devices.add(result("b", vec![address("10.0.0.5")], 2)));
    assert!(devices.add(result("c", vec![net_id("5.1.2.3.1.1")], 3)));
    assert!(devices.add(result(
        "d",
        vec![net_id("5.1.2.3.1.1"), address("10.0.0.9")],
        4
    )));
    assert!(devices.add(result("e", vec![DeviceKey::UsbSerial("0671FF".into())], 5)));
    assert!(devices.add(result("f", vec![DeviceKey::UsbSerial("0671FF".into())], 6)));
    assert!(devices.add(result(
        "g",
        vec![address("10.0.0.7"), DeviceKey::RuntimeId("cell@pi".into())],
        7
    )));
    assert!(devices.add(result("h", vec![DeviceKey::RuntimeId("cell@pi".into())], 8)));
    assert!(devices.add(result(
        "i",
        vec![DeviceKey::Local("ethercat:eth1:1".into())],
        9
    )));
    assert!(devices.add(result(
        "j",
        vec![DeviceKey::Local("ethercat:eth1:2".into())],
        10
    )));
    assert_eq!(
        ids(&devices),
        vec![
            "dev:ip:10.0.0.5",
            "dev:netid:5.1.2.3.1.1",
            "dev:usb:0671FF",
            "dev:ip:10.0.0.7",
            "dev:ethercat:eth1:1",
            "dev:ethercat:eth1:2",
        ]
    );
    let (all, _) = devices.snapshot(None);
    assert_eq!(all[1].addresses, vec!["10.0.0.9"]);
    assert_eq!(all[1].ams_net_ids, vec!["5.1.2.3.1.1"]);
    assert_eq!(all[3].runtime_ids, vec!["cell@pi"]);
    assert_eq!(all[3].finds.len(), 2);
    assert_eq!(all[3].last_seen_ms, 8);

    // A result that shares the first device's address and the second's AMS Net ID joins them
    // into the one found first.
    let before = devices.cursor();
    assert!(devices.add(result(
        "k",
        vec![address("10.0.0.5"), net_id("5.1.2.3.1.1")],
        11
    )));
    let (changed, merged) = devices.snapshot(Some(before));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id, "dev:ip:10.0.0.5");
    assert_eq!(changed[0].addresses, vec!["10.0.0.5", "10.0.0.9"]);
    assert_eq!(changed[0].ams_net_ids, vec!["5.1.2.3.1.1"]);
    assert_eq!(
        changed[0]
            .finds
            .iter()
            .map(|find| find.key.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c", "d", "k"]
    );
    assert_eq!(
        merged,
        vec![MergedDevice {
            id: "dev:netid:5.1.2.3.1.1".into(),
            into: "dev:ip:10.0.0.5".into(),
        }]
    );
    assert_eq!(ids(&devices).len(), 5);
}

#[test]
fn refound_result_moves_last_seen_and_keeps_the_stronger_confidence() {
    let mut devices = Devices::default();
    let mut confirmed = result("10.0.0.5:502/1", vec![address("10.0.0.5")], 10);
    confirmed.confidence = DiscoveryConfidence::Confirmed;
    confirmed.source = "modbus_device_id".into();
    assert!(devices.add(confirmed));
    let cursor = devices.cursor();

    // The same result, weaker and later: listed once, still confirmed, seen later.
    assert!(!devices.add(result("10.0.0.5:502/1", vec![address("10.0.0.5")], 20)));
    let (all, _) = devices.snapshot(None);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].finds.len(), 1);
    assert_eq!(all[0].finds[0].confidence, DiscoveryConfidence::Confirmed);
    assert_eq!(all[0].finds[0].source, "modbus_device_id");
    assert_eq!(all[0].finds[0].last_seen_ms, 20);
    assert_eq!(all[0].last_seen_ms, 20);
    assert!(devices.cursor() > cursor, "a later sighting is a change");

    // Seen again with nothing new: no change, so `since` lists nothing.
    let cursor = devices.cursor();
    assert!(!devices.add(result("10.0.0.5:502/1", vec![address("10.0.0.5")], 20)));
    assert_eq!(devices.cursor(), cursor);
    assert!(devices.snapshot(Some(cursor)).0.is_empty());
}
