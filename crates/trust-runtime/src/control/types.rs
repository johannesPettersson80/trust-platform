use serde::Deserialize;

// The request and response envelope is shared runtime code (spec 34 ARCH-11).
pub(crate) use trust_runtime_services::protocol::ControlResponse;
pub(super) use trust_runtime_services::protocol::{ControlAfterResponse, ControlRequest};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HistorianQueryParams {
    pub(super) variable: Option<String>,
    pub(super) since_ms: Option<u128>,
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HistorianAlertsParams {
    pub(super) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvalParams {
    pub(super) expr: String,
}

#[cfg(test)]
mod tests {
    use crate::io::{IoAddress, IoInterface, IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
    use crate::memory::MemoryLocation;
    use crate::value::{Value, ValueRef};
    use serde_json::json;
    use smol_str::SmolStr;

    /// The hosted snapshot's JSON, for the snapshot-shape tests only: the shared
    /// [`to_json`](trust_runtime_services::live_values::io_snapshot::IoSnapshot::to_json) answers
    /// `io.list` and `io.read` now.
    trait IoSnapshotJson {
        fn into_json(self) -> serde_json::Value;
    }

    impl IoSnapshotJson for IoSnapshot {
        fn into_json(self) -> serde_json::Value {
            let IoSnapshot {
                scan,
                forced,
                inputs,
                outputs,
                memory,
            } = self;
            json!({
                "scan": scan,
                "inputs": inputs.iter().map(|entry| entry_to_json(entry, &forced)).collect::<Vec<_>>(),
                "outputs": outputs.iter().map(|entry| entry_to_json(entry, &forced)).collect::<Vec<_>>(),
                "memory": memory.iter().map(|entry| entry_to_json(entry, &forced)).collect::<Vec<_>>(),
            })
        }
    }

    fn entry_to_json(
        entry: &crate::io::IoSnapshotEntry,
        forced: &[IoAddress],
    ) -> serde_json::Value {
        let mut payload = json!({
            "name": entry.name.as_ref().map(|name| name.as_str()),
            "address": entry.address.to_string(),
            "value": format_snapshot_value(&entry.value),
            "forced": forced.iter().any(|address| address == &entry.address),
        });
        if let Some(value_type) = entry
            .value_type_name
            .as_deref()
            .or_else(|| entry.value_type.and_then(crate::io::io_value_type_name))
        {
            payload["valueType"] = json!(value_type);
        }
        if let Some(source) = &entry.source {
            payload["source"] = json!(source.as_str());
        }
        payload
    }

    fn format_snapshot_value(value: &crate::io::IoSnapshotValue) -> serde_json::Value {
        match value {
            crate::io::IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
            crate::io::IoSnapshotValue::Error(err) => json!({ "error": err }),
            crate::io::IoSnapshotValue::Unresolved => json!("unresolved"),
        }
    }

    #[test]
    fn io_snapshot_json_includes_optional_source() {
        let snapshot = IoSnapshot {
            scan: Some(4),
            forced: Vec::new(),
            inputs: vec![IoSnapshotEntry {
                name: Some(SmolStr::new("In0")),
                address: IoAddress::parse("%IX0.0").expect("address"),
                value_type: None,
                value_type_name: None,
                value: IoSnapshotValue::Value(Value::Bool(true)),
                source: Some(SmolStr::new("MQTT topic plant/line1")),
            }],
            outputs: Vec::new(),
            memory: Vec::new(),
        };

        let payload = snapshot.into_json();
        assert_eq!(
            payload["inputs"][0]["source"].as_str(),
            Some("MQTT topic plant/line1")
        );
    }

    #[test]
    fn io_snapshot_json_includes_optional_value_type() {
        let snapshot = IoSnapshot {
            scan: Some(4),
            forced: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            memory: vec![IoSnapshotEntry {
                name: Some(SmolStr::new("Speed")),
                address: IoAddress::parse("%MD0").expect("address"),
                value_type: Some(trust_hir::TypeId::REAL),
                value_type_name: None,
                value: IoSnapshotValue::Value(Value::Real(1.5)),
                source: None,
            }],
        };

        let payload = snapshot.into_json();
        assert_eq!(payload["memory"][0]["valueType"].as_str(), Some("REAL"));
    }

    #[test]
    fn enum_io_snapshot_json_preserves_declared_type_for_every_value_state() {
        fn bind_enum(interface: &mut IoInterface, address: IoAddress) {
            interface.bind_ref_named_enum(
                ValueRef {
                    location: MemoryLocation::Global,
                    offset: 0,
                    generation: crate::memory::Generation::FIRST,
                    path: Vec::new(),
                },
                address,
                trust_hir::TypeId(10_000),
                trust_hir::TypeId::DINT,
                SmolStr::new("TrafficState"),
                vec![(SmolStr::new("Stopped"), 0), (SmolStr::new("Running"), 1)],
                "MainInstance.State",
            );
        }

        let address = IoAddress::parse("%QD0").expect("enum output address");
        let mut interface = IoInterface::new();
        interface.resize(0, 4, 0);
        bind_enum(&mut interface, address.clone());
        interface
            .write(&address, Value::DWord(1))
            .expect("write declared enum value");
        let value_payload = interface.snapshot().into_json();
        assert_eq!(
            value_payload["outputs"][0]["valueType"].as_str(),
            Some("TrafficState"),
            "resolved enum snapshots must expose the declared enum type"
        );

        interface
            .write(&address, Value::DWord(9))
            .expect("write undeclared enum wire value");
        let error_payload = interface.snapshot().into_json();
        assert_eq!(
            error_payload["outputs"][0]["valueType"].as_str(),
            Some("TrafficState"),
            "enum conversion errors must retain the declared enum type"
        );

        let mut unresolved = IoInterface::new();
        bind_enum(
            &mut unresolved,
            IoAddress::parse("%Q*").expect("wildcard enum output address"),
        );
        let unresolved_payload = unresolved.snapshot().into_json();
        assert_eq!(
            unresolved_payload["outputs"][0]["valueType"].as_str(),
            Some("TrafficState"),
            "unresolved enum snapshots must retain the declared enum type"
        );
    }
}
