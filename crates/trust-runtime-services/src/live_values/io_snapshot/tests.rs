//! The portable snapshot's JSON contract: byte for byte what `io.list` and `io.read` answer today
//! (specification 11, "Live Values"; specification 34, ARCH-11).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::stdlib::conversions::ConversionType;
use trust_runtime_core::value::Value;

use super::*;

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("test address")
}

#[test]
fn snapshot_json_is_the_io_list_and_io_read_answer() {
    let snapshot = IoSnapshot {
        scan: Some(4),
        forced: vec![address("%QW2")],
        inputs: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("In0")),
            address: address("%IX0.0"),
            value_type_name: Some(SmolStr::new("BOOL")),
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: Some(SmolStr::new("MQTT topic plant/line1")),
        }],
        outputs: vec![IoSnapshotEntry {
            name: None,
            address: address("%QW2"),
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::LInt(7)),
            source: None,
        }],
        memory: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("Speed")),
            address: address("%MD4"),
            value_type_name: Some(SmolStr::new("REAL")),
            value: IoSnapshotValue::Value(Value::Real(1.5)),
            source: None,
        }],
    };

    assert_eq!(
        snapshot.to_json(),
        json!({
            "scan": 4,
            "inputs": [{
                "name": "In0",
                "address": "%IX0.0",
                "value": "Bool(true)",
                "forced": false,
                "valueType": "BOOL",
                "source": "MQTT topic plant/line1",
            }],
            "outputs": [{
                "name": null,
                "address": "%QW2",
                "value": "LInt(7)",
                "forced": true,
            }],
            "memory": [{
                "name": "Speed",
                "address": "%MD4",
                "value": "Real(1.5)",
                "forced": false,
                "valueType": "REAL",
            }],
        })
    );
}

#[test]
fn snapshot_text_is_written_in_the_order_of_the_names() {
    // Specification 34 PORT-02: `io.list` and `io.read` write the snapshot as text without building
    // a value, the members of every object in the order of their names, as the value they answered
    // with before wrote them; worked out by hand from the snapshot of the first test. Catches a
    // member out of that order, a type name or source written without a value, and an address
    // written another way.
    let snapshot = IoSnapshot {
        scan: Some(4),
        forced: vec![address("%QW2")],
        inputs: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("In0")),
            address: address("%IX0.0"),
            value_type_name: Some(SmolStr::new("BOOL")),
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: Some(SmolStr::new("MQTT topic plant/line1")),
        }],
        outputs: vec![IoSnapshotEntry {
            name: None,
            address: address("%QW2"),
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::LInt(7)),
            source: None,
        }],
        memory: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("Speed")),
            address: address("%MD4"),
            value_type_name: Some(SmolStr::new("REAL")),
            value: IoSnapshotValue::Value(Value::Real(1.5)),
            source: None,
        }],
    };
    assert_eq!(
        serde_json::to_string(&snapshot).expect("the snapshot serializes"),
        concat!(
            r#"{"inputs":[{"address":"%IX0.0","forced":false,"name":"In0","source":"MQTT topic plant/line1","value":"Bool(true)","valueType":"BOOL"}],"#,
            r#""memory":[{"address":"%MD4","forced":false,"name":"Speed","value":"Real(1.5)","valueType":"REAL"}],"#,
            r#""outputs":[{"address":"%QW2","forced":true,"name":null,"value":"LInt(7)"}],"scan":4}"#
        )
    );
}

#[test]
fn snapshot_json_renders_unresolved_and_error_values() {
    let snapshot = IoSnapshot {
        scan: None,
        forced: Vec::new(),
        inputs: vec![IoSnapshotEntry {
            name: None,
            address: address("%Q*"),
            value_type_name: None,
            value: IoSnapshotValue::Unresolved,
            source: None,
        }],
        outputs: vec![IoSnapshotEntry {
            name: None,
            address: address("%QW0"),
            value_type_name: Some(SmolStr::new("INT")),
            value: IoSnapshotValue::Error(String::from(
                "process-image value 9 is not declared by enum TrafficState",
            )),
            source: None,
        }],
        memory: Vec::new(),
    };

    assert_eq!(
        snapshot.to_json(),
        json!({
            "scan": null,
            "inputs": [{
                "name": null,
                "address": "%QX*",
                "value": "unresolved",
                "forced": false,
            }],
            "outputs": [{
                "name": null,
                "address": "%QW0",
                "value": {"error": "process-image value 9 is not declared by enum TrafficState"},
                "forced": false,
                "valueType": "INT",
            }],
            "memory": [],
        })
    );
}

#[test]
fn snapshot_json_includes_optional_source() {
    let snapshot = IoSnapshot {
        scan: Some(4),
        forced: Vec::new(),
        inputs: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("In0")),
            address: address("%IX0.0"),
            value_type_name: None,
            value: IoSnapshotValue::Value(Value::Bool(true)),
            source: Some(SmolStr::new("MQTT topic plant/line1")),
        }],
        outputs: Vec::new(),
        memory: Vec::new(),
    };

    assert_eq!(
        snapshot.to_json()["inputs"][0]["source"].as_str(),
        Some("MQTT topic plant/line1")
    );
}

#[test]
fn snapshot_json_includes_optional_value_type() {
    let snapshot = IoSnapshot {
        scan: Some(4),
        forced: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        memory: vec![IoSnapshotEntry {
            name: Some(SmolStr::new("Speed")),
            address: address("%MD0"),
            value_type_name: Some(SmolStr::new("REAL")),
            value: IoSnapshotValue::Value(Value::Real(1.5)),
            source: None,
        }],
    };

    assert_eq!(
        snapshot.to_json()["memory"][0]["valueType"].as_str(),
        Some("REAL")
    );
}

#[test]
fn value_type_name_is_the_iec_name_for_every_image_type() {
    for (wire_type, name) in [
        (ConversionType::Bool, "BOOL"),
        (ConversionType::SInt, "SINT"),
        (ConversionType::Int, "INT"),
        (ConversionType::DInt, "DINT"),
        (ConversionType::LInt, "LINT"),
        (ConversionType::USInt, "USINT"),
        (ConversionType::UInt, "UINT"),
        (ConversionType::UDInt, "UDINT"),
        (ConversionType::ULInt, "ULINT"),
        (ConversionType::Real, "REAL"),
        (ConversionType::LReal, "LREAL"),
        (ConversionType::Byte, "BYTE"),
        (ConversionType::Word, "WORD"),
        (ConversionType::DWord, "DWORD"),
        (ConversionType::LWord, "LWORD"),
        (ConversionType::Time, "TIME"),
        (ConversionType::LTime, "LTIME"),
        (ConversionType::LDate, "LDATE"),
        (ConversionType::LTod, "LTOD"),
        (ConversionType::Ldt, "LDT"),
        (ConversionType::String, "STRING"),
        (ConversionType::Char, "CHAR"),
        (ConversionType::WChar, "WCHAR"),
    ] {
        assert_eq!(value_type_name(wire_type), Some(name), "{name}");
    }

    for wire_type in [
        ConversionType::Date,
        ConversionType::Tod,
        ConversionType::Dt,
    ] {
        assert_eq!(
            value_type_name(wire_type),
            None,
            "{wire_type:?} has no fixed image width"
        );
    }
}
