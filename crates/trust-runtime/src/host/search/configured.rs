//! The devices that a project's configuration names (specification 11, "Compare with the project"):
//! which settings name a device for each protocol, and the facts of a configured device.
//!
//! The settings come in as JSON in the shape of a connection's parameters: a driver's parameters
//! for Modbus TCP, EtherCAT and MQTT, and `{"connections": [...]}` for ADS and the OPC UA client.
//! The project's files (`project_devices`) and the topology that a runtime reports give the same
//! shapes, so both name the same devices. Pure: nothing is read, looked up or opened here.

use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Map, Value};

use super::model::SourceId;

/// The OPC UA port that an endpoint URL without one means.
const OPCUA_PORT: u16 = 4840;
/// The unit IDs that a search can probe (specification 11, "Requests").
const SEARCHABLE_UNIT_IDS: std::ops::RangeInclusive<u8> = 1..=247;

/// The facts of a device: the named members that a comparison lists (specification 11, "Reply").
pub type Facts = Map<String, Value>;

/// Where a configured network device is (specification 11, "Host names").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Host {
    /// An IP address.
    Ip(IpAddr),
    /// `localhost`, which stands for the loopback addresses.
    Localhost,
    /// Any other host name, in lower case. A search reports the addresses it probed, not names.
    Name(String),
}

impl Host {
    /// Reads a host as the configuration writes it.
    ///
    /// # Errors
    /// The text is empty.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("the host is empty".to_string());
        }
        Ok(if let Ok(ip) = text.parse::<IpAddr>() {
            Self::Ip(ip)
        } else if text.eq_ignore_ascii_case("localhost") {
            Self::Localhost
        } else {
            Self::Name(text.to_ascii_lowercase())
        })
    }

    /// Whether `address`, as a search reports one (an IP address, or a host name in lower case),
    /// is this host.
    #[must_use]
    pub fn is(&self, address: &str) -> bool {
        match self {
            Self::Ip(ip) => address.parse::<IpAddr>().is_ok_and(|other| other == *ip),
            Self::Localhost => {
                address.eq_ignore_ascii_case("localhost")
                    || address.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
            }
            Self::Name(name) => address.eq_ignore_ascii_case(name),
        }
    }

    /// Whether the host is a name that a search cannot place by its address.
    #[must_use]
    pub fn is_unplaced_name(&self) -> bool {
        matches!(self, Self::Name(_))
    }

    /// The host and `port` as a known address of a search: `host:port`, with brackets around an
    /// IPv6 address.
    #[must_use]
    pub fn with_port(&self, port: u16) -> String {
        match self {
            Self::Ip(IpAddr::V6(ip)) => format!("[{ip}]:{port}"),
            Self::Ip(ip) => format!("{ip}:{port}"),
            Self::Localhost => format!("localhost:{port}"),
            Self::Name(name) => format!("{name}:{port}"),
        }
    }
}

/// A Modbus TCP device: a `modbus-tcp` driver of `io.toml`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfiguredModbus {
    /// The device's IP address and port.
    pub address: SocketAddr,
    /// The unit ID, 1 when the driver names none.
    pub unit_id: u8,
    /// The vendor name that the driver expects, when it states one.
    pub vendor_name: Option<String>,
    /// The product code that the driver expects, when it states one.
    pub product_code: Option<String>,
    /// The revision that the driver expects, when it states one.
    pub revision: Option<String>,
}

/// One module of an `ethercat` driver.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfiguredEthercat {
    /// The driver's bus adapter.
    pub adapter: String,
    /// The module's position on the bus (its `slot`).
    pub position: u16,
    /// The module's model, in upper case.
    pub model: String,
}

/// An MQTT broker: an `mqtt` driver of `io.toml`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfiguredMqtt {
    /// The broker's host.
    pub host: Host,
    /// The broker's port.
    pub port: u16,
}

/// An OPC UA server: a connection of the OPC UA client file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfiguredOpcua {
    /// The server's host.
    pub host: Host,
    /// The server's port.
    pub port: u16,
    /// The security policy of the connection, as the configuration writes it.
    pub security_policy: String,
    /// The security mode of the connection, as the configuration writes it.
    pub security_mode: String,
}

/// A TwinCAT system: a connection of the ADS file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfiguredAds {
    /// The AMS Net ID of the target.
    pub ams_net_id: String,
    /// The host that the connection reaches the target at; empty when the connection names none.
    pub host: String,
}

/// A device that the project's configuration names (specification 11, "Compare with the
/// project").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfiguredDevice {
    /// A Modbus TCP device.
    ModbusTcp(ConfiguredModbus),
    /// A module of an EtherCAT bus.
    Ethercat(ConfiguredEthercat),
    /// An MQTT broker.
    Mqtt(ConfiguredMqtt),
    /// An OPC UA server.
    Opcua(ConfiguredOpcua),
    /// A TwinCAT system.
    Ads(ConfiguredAds),
}

impl ConfiguredDevice {
    /// The source whose results can be this device.
    #[must_use]
    pub fn protocol(&self) -> SourceId {
        match self {
            Self::ModbusTcp(_) => SourceId::ModbusTcp,
            Self::Ethercat(_) => SourceId::Ethercat,
            Self::Mqtt(_) => SourceId::Mqtt,
            Self::Opcua(_) => SourceId::Opcua,
            Self::Ads(_) => SourceId::Ads,
        }
    }

    /// Whether a result of `protocol` can be compared with the project at all (specification 11,
    /// "The project's devices"): only the five protocols that name a configured device.
    #[must_use]
    pub fn is_compared(protocol: SourceId) -> bool {
        matches!(
            protocol,
            SourceId::ModbusTcp
                | SourceId::Ethercat
                | SourceId::Mqtt
                | SourceId::Opcua
                | SourceId::Ads
        )
    }

    /// The devices that the settings of one connection name. `protocol` is the connection's
    /// protocol as the topology names it (`modbus_tcp`, `ethercat`, `mqtt`, `ads` or
    /// `opcua_client`, and the spellings that a driver alias gives it: `mqtt_tcp`, `ether_cat`
    /// and `ecat`); another protocol names no device.
    ///
    /// # Errors
    /// The settings do not name a device the way the driver reads them, for example a Modbus
    /// `address` that is not an IP address and a port; the text names the setting.
    pub fn from_connection(protocol: &str, params: &Value) -> Result<Vec<Self>, String> {
        match protocol {
            "modbus_tcp" => modbus(params).map(|device| vec![ConfiguredDevice::ModbusTcp(device)]),
            "ethercat" | "ether_cat" | "ecat" => ethercat(params),
            "mqtt" | "mqtt_tcp" => mqtt(params).map(|device| vec![ConfiguredDevice::Mqtt(device)]),
            "opcua_client" => connections(params)
                .into_iter()
                .map(opcua)
                .collect::<Result<Vec<_>, _>>(),
            "ads" => connections(params)
                .into_iter()
                .map(ads)
                .collect::<Result<Vec<_>, _>>(),
            _ => Ok(Vec::new()),
        }
    }

    /// The facts that name the device and that the project states of it (specification 11,
    /// "Reply").
    #[must_use]
    pub fn facts(&self) -> Facts {
        let mut facts = Facts::new();
        match self {
            Self::ModbusTcp(device) => {
                facts.insert("address".to_string(), json!(device.address.to_string()));
                facts.insert("unit_id".to_string(), json!(device.unit_id));
                for (key, stated) in [
                    ("vendor_name", &device.vendor_name),
                    ("product_code", &device.product_code),
                    ("revision", &device.revision),
                ] {
                    if let Some(stated) = stated {
                        facts.insert(key.to_string(), json!(stated));
                    }
                }
            }
            Self::Ethercat(device) => {
                facts.insert("adapter".to_string(), json!(device.adapter));
                facts.insert("position".to_string(), json!(device.position));
                facts.insert("model".to_string(), json!(device.model));
            }
            Self::Mqtt(device) => {
                facts.insert(
                    "address".to_string(),
                    json!(device.host.with_port(device.port)),
                );
            }
            Self::Opcua(device) => {
                facts.insert(
                    "address".to_string(),
                    json!(device.host.with_port(device.port)),
                );
                facts.insert("security".to_string(), json!(device.security()));
            }
            Self::Ads(device) => {
                facts.insert("ams_net_id".to_string(), json!(device.ams_net_id));
                if !device.host.is_empty() {
                    facts.insert("address".to_string(), json!(device.host));
                }
            }
        }
        facts
    }

    /// The known address that a search should probe to look for the device, when it has one: a
    /// host name or IP address with an optional port (specification 11, "Requests").
    #[must_use]
    pub fn target(&self) -> Option<String> {
        match self {
            Self::ModbusTcp(device) => Some(device.address.to_string()),
            Self::Mqtt(device) => Some(device.host.with_port(device.port)),
            Self::Opcua(device) => Some(device.host.with_port(device.port)),
            Self::Ads(device) => Some(device.host.clone()).filter(|host| !host.is_empty()),
            Self::Ethercat(_) => None,
        }
    }

    /// The Modbus unit ID that a search can probe, when the device is a Modbus TCP device whose unit
    /// ID lies between 1 and 247.
    #[must_use]
    pub fn probe_unit_id(&self) -> Option<u8> {
        match self {
            Self::ModbusTcp(device) if SEARCHABLE_UNIT_IDS.contains(&device.unit_id) => {
                Some(device.unit_id)
            }
            _ => None,
        }
    }

    /// Whether a search cannot probe the device: a Modbus unit ID outside 1–247.
    #[must_use]
    pub fn unit_id_unsearchable(&self) -> bool {
        matches!(self, Self::ModbusTcp(device) if !SEARCHABLE_UNIT_IDS.contains(&device.unit_id))
    }

    /// Whether the device is named by a host name that a search cannot place by its address.
    #[must_use]
    pub fn named_by_unplaced_host(&self) -> bool {
        match self {
            Self::Mqtt(device) => device.host.is_unplaced_name(),
            Self::Opcua(device) => device.host.is_unplaced_name(),
            Self::ModbusTcp(_) | Self::Ethercat(_) | Self::Ads(_) => false,
        }
    }
}

impl ConfiguredOpcua {
    /// The connection's security as one fact: `<policy> / <mode>`.
    #[must_use]
    pub fn security(&self) -> String {
        format!("{} / {}", self.security_policy, self.security_mode)
    }
}

/// The host and port of an `opc.tcp://` endpoint URL; the port is 4840 when the URL names none.
///
/// # Errors
/// The text is not an `opc.tcp://` URL with a host, or its port is not a number from 1 to 65,535.
pub fn endpoint_host_and_port(endpoint_url: &str) -> Result<(Host, u16), String> {
    let url = endpoint_url.trim();
    let Some(rest) = url.strip_prefix("opc.tcp://") else {
        return Err(format!("endpoint_url '{url}' is not an opc.tcp:// URL"));
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    split_authority(authority, Some(OPCUA_PORT))
        .map_err(|reason| format!("endpoint_url '{url}': {reason}"))
}

/// The settings of the connections in `params`: the objects of its `connections` list, or the
/// object itself when it names no list.
fn connections(params: &Value) -> Vec<&Value> {
    match params.get("connections").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter(|connection| connection.is_object())
            .collect(),
        None if params.as_object().is_some_and(|object| !object.is_empty()) => vec![params],
        None => Vec::new(),
    }
}

/// The trimmed, non-empty text at `key`.
fn text<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The text a driver states for `key`: absent or empty is not stated.
fn stated(params: &Value, key: &str) -> Result<Option<String>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            Ok(Some(text.trim().to_string()).filter(|text| !text.is_empty()))
        }
        Some(_) => Err(format!("{key} must be text")),
    }
}

fn modbus(params: &Value) -> Result<ConfiguredModbus, String> {
    let address =
        text(params, "address").ok_or_else(|| "modbus_tcp: address is missing".to_string())?;
    let address = address
        .parse::<SocketAddr>()
        .map_err(|_| format!("modbus_tcp: address '{address}' is not an IP address and a port"))?;
    let unit_id = match params.get("unit_id") {
        None | Some(Value::Null) => 1,
        Some(value) => value
            .as_u64()
            .and_then(|unit_id| u8::try_from(unit_id).ok())
            .ok_or_else(|| format!("modbus_tcp: unit_id {value} is not a number from 0 to 255"))?,
    };
    let identity =
        |key: &str| stated(params, key).map_err(|reason| format!("modbus_tcp: {reason}"));
    Ok(ConfiguredModbus {
        address,
        unit_id,
        vendor_name: identity("vendor_name")?,
        product_code: identity("product_code")?,
        revision: identity("revision")?,
    })
}

fn ethercat(params: &Value) -> Result<Vec<ConfiguredDevice>, String> {
    let adapter = text(params, "adapter").unwrap_or("mock").to_string();
    let settings = to_toml(params).unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let modules = crate::io::configured_ethercat_modules(&settings)
        .map_err(|error| format!("ethercat: {error}"))?;
    Ok(modules
        .into_iter()
        .map(|module| {
            ConfiguredDevice::Ethercat(ConfiguredEthercat {
                adapter: adapter.clone(),
                position: module.slot,
                model: module.model,
            })
        })
        .collect())
}

/// The settings as the TOML value that the driver reads. Not through `serde`: a number of a JSON value
/// serializes as a structure when `serde_json` keeps arbitrary precision, which TOML cannot hold. A `null`
/// member has no TOML form and is left out, as a setting that is not stated.
fn to_toml(value: &Value) -> Option<toml::Value> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(flag) => toml::Value::Boolean(*flag),
        Value::Number(number) => number
            .as_i64()
            .map(toml::Value::Integer)
            .or_else(|| number.as_f64().map(toml::Value::Float))?,
        Value::String(text) => toml::Value::String(text.clone()),
        Value::Array(items) => toml::Value::Array(items.iter().filter_map(to_toml).collect()),
        Value::Object(members) => toml::Value::Table(
            members
                .iter()
                .filter_map(|(key, item)| Some((key.clone(), to_toml(item)?)))
                .collect(),
        ),
    })
}

fn mqtt(params: &Value) -> Result<ConfiguredMqtt, String> {
    let broker = text(params, "broker").ok_or_else(|| "mqtt: broker is missing".to_string())?;
    let address = ["tcp://", "mqtt://", "ssl://", "mqtts://"]
        .iter()
        .find_map(|scheme| broker.strip_prefix(scheme))
        .unwrap_or(broker);
    let (host, port) = split_authority(address, None)
        .map_err(|reason| format!("mqtt: broker '{broker}': {reason}"))?;
    Ok(ConfiguredMqtt { host, port })
}

fn opcua(connection: &Value) -> Result<ConfiguredDevice, String> {
    let url = text(connection, "endpoint_url")
        .ok_or_else(|| "opcua_client: endpoint_url is missing".to_string())?;
    let (host, port) =
        endpoint_host_and_port(url).map_err(|reason| format!("opcua_client: {reason}"))?;
    let setting = |key: &str| {
        text(connection, key)
            .map(str::to_string)
            .ok_or_else(|| format!("opcua_client: {key} is missing"))
    };
    Ok(ConfiguredDevice::Opcua(ConfiguredOpcua {
        host,
        port,
        security_policy: setting("security_policy")?,
        security_mode: setting("security_mode")?,
    }))
}

fn ads(connection: &Value) -> Result<ConfiguredDevice, String> {
    let ams_net_id = text(connection, "target_net_id")
        .or_else(|| text(connection, "ams_net_id"))
        .ok_or_else(|| "ads: target_net_id is missing".to_string())?
        .to_string();
    let host = text(connection, "host")
        .or_else(|| text(connection, "ip"))
        .unwrap_or_default()
        .to_string();
    Ok(ConfiguredDevice::Ads(ConfiguredAds { ams_net_id, host }))
}

/// Splits `host`, `host:port` or `[ipv6]:port` into its host and port. `default_port` is the port
/// of an authority that names none; without one, a missing port is an error.
fn split_authority(authority: &str, default_port: Option<u16>) -> Result<(Host, u16), String> {
    let authority = authority.trim();
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, tail) = bracketed
            .split_once(']')
            .ok_or_else(|| "the IPv6 address has no closing bracket".to_string())?;
        (host, tail.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => (host, Some(port)),
            _ => (authority, None),
        }
    };
    let port = match port {
        Some(port) => port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| format!("port '{port}' is not a number from 1 to 65535"))?,
        None => default_port.ok_or_else(|| "the port is missing".to_string())?,
    };
    Ok((Host::parse(host)?, port))
}

#[cfg(test)]
#[path = "configured_tests.rs"]
mod tests;
