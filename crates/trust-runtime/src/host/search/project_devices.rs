//! The devices that a project's files name (specification 11, "Compare with the project"):
//! `io.toml`, and for ADS and OPC UA the client files that `runtime.toml` enables. This is the one
//! reader of those files for the comparison: `comm.compare` and `trust-runtime discover --compare`
//! both use it. [`ConfiguredDevice`] says which settings of each connection name a device.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::configured::ConfiguredDevice;
use crate::config::{IoConfig, RuntimeConfig};
use crate::connectors::adapters::io_driver::protocol_from_io_driver_name;
use crate::connectors::ConnectorProtocol;

/// The file of a project that says which other files configure ADS and the OPC UA client.
const RUNTIME_FILE: &str = "runtime.toml";
/// The file of a project that configures its I/O drivers.
const IO_FILE: &str = "io.toml";

/// Reads the devices that the project in `project` names, in the order of its files: the drivers of
/// `io.toml`, then the ADS connections, then the OPC UA client connections. A disabled driver names no
/// device, and ADS and the OPC UA client name devices only when `runtime.toml` enables them.
///
/// # Errors
/// The folder holds no `runtime.toml`, a file cannot be read or understood, or a driver or
/// connection does not name a device the way the runtime reads it. The message names the file.
pub fn load(project: &Path) -> Result<Vec<ConfiguredDevice>, String> {
    let runtime_path = project.join(RUNTIME_FILE);
    if !runtime_path.is_file() {
        return Err(format!(
            "{} has no {RUNTIME_FILE}; it is not a truST project folder",
            project.display()
        ));
    }
    let runtime = RuntimeConfig::load(&runtime_path)
        .map_err(|error| format!("{}: {error}", runtime_path.display()))?;
    let mut devices = io_devices(&project.join(IO_FILE))?;
    if runtime.ads.enabled {
        devices.extend(sidecar_devices(
            project,
            &runtime.ads.config_path,
            "ads",
            ads_connections,
        )?);
    }
    if runtime.opcua_client.enabled {
        devices.extend(sidecar_devices(
            project,
            &runtime.opcua_client.config_path,
            "opcua_client",
            opcua_connections,
        )?);
    }
    Ok(devices)
}

/// The devices of the enabled Modbus TCP, EtherCAT and MQTT drivers of `io.toml`; none when the
/// project has no such file.
fn io_devices(path: &Path) -> Result<Vec<ConfiguredDevice>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let config = IoConfig::load(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut devices = Vec::new();
    for (index, driver) in config.drivers.iter().enumerate() {
        let protocol = match protocol_from_io_driver_name(driver.name.as_str()) {
            ConnectorProtocol::ModbusTcp => "modbus_tcp",
            ConnectorProtocol::Mqtt => "mqtt",
            ConnectorProtocol::Ethercat => "ethercat",
            _ => continue,
        };
        if !driver.enabled {
            continue;
        }
        let params = serde_json::to_value(&driver.params)
            .map_err(|error| format!("{}: driver {}: {error}", path.display(), index + 1))?;
        let named = ConfiguredDevice::from_connection(protocol, &params).map_err(|reason| {
            format!(
                "{}: driver {} ({}): {reason}",
                path.display(),
                index + 1,
                driver.name
            )
        })?;
        devices.extend(named);
    }
    Ok(devices)
}

/// The devices of a client file that `runtime.toml` names with `config_path`, read with `connections`.
fn sidecar_devices(
    project: &Path,
    config_path: &Path,
    protocol: &str,
    connections: fn(&str) -> Result<Value, String>,
) -> Result<Vec<ConfiguredDevice>, String> {
    let path: PathBuf = if config_path.is_relative() {
        project.join(config_path)
    } else {
        config_path.to_path_buf()
    };
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let params = connections(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    ConfiguredDevice::from_connection(protocol, &params)
        .map_err(|reason| format!("{}: {reason}", path.display()))
}

/// The connections of an ADS file, as `{"connections": [...]}` (specification 11, "The project's
/// devices").
fn ads_connections(text: &str) -> Result<Value, String> {
    let config = crate::ads::parse_ads_toml(text).map_err(|error| error.to_string())?;
    let connections: Vec<Value> = config
        .connections
        .iter()
        .map(|connection| {
            json!({
                "target_net_id": connection.route.target_net_id.0,
                "host": connection.route.host,
            })
        })
        .collect();
    Ok(json!({ "connections": connections }))
}

/// The connections of an OPC UA client file, as `{"connections": [...]}` (specification 11, "The
/// project's devices").
fn opcua_connections(text: &str) -> Result<Value, String> {
    let config = crate::opcua::parse_opcua_client_toml(text).map_err(|error| error.to_string())?;
    let connections: Vec<Value> = config
        .connections
        .iter()
        .map(|connection| {
            json!({
                "endpoint_url": connection.endpoint_url,
                "security_policy": connection.security.policy.as_config_value(),
                "security_mode": connection.security.mode.as_config_value(),
            })
        })
        .collect();
    Ok(json!({ "connections": connections }))
}

#[cfg(test)]
#[path = "project_devices_tests.rs"]
mod tests;
