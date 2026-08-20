use super::*;

pub(super) fn configured_io_drivers(state: &ControlState) -> Vec<IoDriverConfig> {
    let Some(root) = state.project_root.as_ref() else {
        return Vec::new();
    };
    IoConfig::load(root.join("io.toml"))
        .map(|config| config.drivers)
        .unwrap_or_default()
}

pub(super) fn driver_health_for_config<'a>(
    io_health: &'a [IoDriverStatus],
    index: usize,
    driver: &str,
) -> Option<&'a IoDriverStatus> {
    io_health
        .iter()
        .filter(|status| same_driver_name(status.name.as_str(), driver))
        .nth(index)
}

pub(super) fn same_driver_name(left: &str, right: &str) -> bool {
    protocol_from_driver_name(left) == protocol_from_driver_name(right)
}

pub(super) fn io_snapshot_live(
    snapshot: Option<&IoSnapshot>,
    seen_ms: u64,
) -> Option<serde_json::Value> {
    let snapshot = snapshot?;
    if seen_ms == 0 {
        return None;
    }
    Some(json!({
        "value": {
            "inputs": snapshot.inputs.len(),
            "outputs": snapshot.outputs.len(),
            "memory": snapshot.memory.len(),
            "sample": io_snapshot_sample(snapshot),
        },
        "last_seen_ms": seen_ms,
    }))
}

pub(super) fn io_snapshot_sample(snapshot: &IoSnapshot) -> Vec<serde_json::Value> {
    snapshot
        .inputs
        .iter()
        .map(|entry| ("input", entry))
        .chain(snapshot.outputs.iter().map(|entry| ("output", entry)))
        .chain(snapshot.memory.iter().map(|entry| ("memory", entry)))
        .take(8)
        .map(|(direction, entry)| {
            json!({
                "direction": direction,
                "name": entry.name.as_ref().map(|name| name.as_str()),
                "address": format_io_address(&entry.address),
                "value": format_io_value(entry),
            })
        })
        .collect()
}

pub(super) fn format_io_value(entry: &IoSnapshotEntry) -> serde_json::Value {
    match &entry.value {
        crate::io::IoSnapshotValue::Value(value) => json!(format!("{value:?}")),
        crate::io::IoSnapshotValue::Error(error) => json!({ "error": error }),
        crate::io::IoSnapshotValue::Unresolved => json!("unresolved"),
    }
}

pub(super) fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        crate::io::IoSize::Bit => "X",
        crate::io::IoSize::Byte => "B",
        crate::io::IoSize::Word => "W",
        crate::io::IoSize::DWord => "D",
        crate::io::IoSize::LWord => "L",
        crate::io::IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if address.size == crate::io::IoSize::Bit {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

pub(super) fn driver_endpoint_address(params: &toml::Value) -> Option<String> {
    ["address", "broker"]
        .into_iter()
        .filter_map(|key| params.get(key).and_then(toml::Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn driver_display_name(protocol: &str, index: usize, params: &toml::Value) -> String {
    fn protocol_display_name(protocol: &str) -> &str {
        match protocol {
            "modbus_tcp" => "Modbus TCP",
            "mqtt" => "MQTT broker",
            "ethercat" => "EtherCAT",
            "gpio" => "GPIO",
            "simulated" => "Simulated I/O",
            "loopback" => "Loopback I/O",
            _ => protocol,
        }
    }

    let display = protocol_display_name(protocol);
    driver_endpoint_address(params)
        .map(|address| format!("{display} {address}"))
        .unwrap_or_else(|| {
            if index == 0 {
                display.to_string()
            } else {
                format!("{display} {}", index + 1)
            }
        })
}

pub(super) fn bool_param(params: &toml::Value, key: &str) -> bool {
    params
        .get(key)
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn redacted_toml_params(params: &toml::Value) -> serde_json::Value {
    toml_to_json_redacted(params, "")
}

pub(super) fn ethercat_endpoint_children(
    protocol: &str,
    params: &toml::Value,
    configured_child_detail: &str,
) -> Vec<FleetEndpointChild> {
    if protocol != "ethercat" {
        return Vec::new();
    }
    crate::io::configured_ethercat_modules(params)
        .map(|modules| {
            modules
                .into_iter()
                .map(|module| FleetEndpointChild {
                    id: format!("ethercat:slot:{}", module.slot),
                    kind: "field_slave".to_string(),
                    name: format!("{} (slot {})", module.model, module.slot),
                    slot: Some(module.slot),
                    model: Some(module.model),
                    channels: Some(module.channels),
                    health: "configured_policy".to_string(),
                    detail: configured_child_detail.to_string(),
                    source: Some("config".to_string()),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn toml_to_json_redacted(value: &toml::Value, key: &str) -> serde_json::Value {
    if is_secret_param_key(key) {
        return serde_json::Value::String("<redacted>".to_string());
    }
    match value {
        toml::Value::String(value) => serde_json::Value::String(value.clone()),
        toml::Value::Integer(value) => json!(value),
        toml::Value::Float(value) => json!(value),
        toml::Value::Boolean(value) => json!(value),
        toml::Value::Datetime(value) => serde_json::Value::String(value.to_string()),
        toml::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| toml_to_json_redacted(value, key))
                .collect(),
        ),
        toml::Value::Table(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), toml_to_json_redacted(value, key)))
                .collect(),
        ),
    }
}

pub(super) fn is_secret_param_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "password"
            | "auth_token"
            | "token"
            | "secret"
            | "client_secret"
            | "credential"
            | "credentials"
            | "private_key"
            | "source_ip"
            | "source_cidr"
            | "allowed_clients"
            | "clients"
    )
}
