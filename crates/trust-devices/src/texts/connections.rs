//! The texts of the connections and their inspectors (specification 25 §5.4, §5.5, §5.8 and
//! §5.13): protocol names, role words, the summaries of a connection's settings, the server and
//! ADS client views, the connector and live rows, and the actions with their reasons. Texts
//! ported from the panel's TypeScript keep its words.

/// The product name of a protocol, such as `Modbus TCP` or `ADS client`; a protocol without one
/// reads with spaces for its underscores (specification 25 §5.7 and §5.9).
#[must_use]
pub fn protocol_name(protocol: &str) -> String {
    let name = match protocol {
        "modbus_tcp" => "Modbus TCP",
        "modbus" => "Modbus",
        "ethercat" => "EtherCAT",
        "opcua" => "OPC UA server",
        "opcua_client" => "OPC UA client",
        "ads" => "ADS client",
        "ads_server" => "ADS server",
        "mqtt" => "MQTT",
        "mesh" => "Mesh",
        "openot" => "OpenOT",
        "discovery" => "Discovery",
        "realtime" | "realtime_t0" => "Realtime",
        "runtime_cloud" | "federation" => "Federation",
        "gpio" => "GPIO",
        "simulated" => "Simulated I/O",
        "loopback" => "Loopback I/O",
        "web" => "Web",
        _ => return protocol.replace('_', " "),
    };
    name.to_owned()
}

// ---- role words (specification 25 §5.13) ----

/// The mark and role word of a draft connection.
pub const DRAFT: &str = "DRAFT";
/// The role word of a client.
pub const ROLE_CLIENT: &str = "CLIENT";
/// The role word of a server.
pub const ROLE_SERVER: &str = "SERVER";
/// The role word of a fieldbus master.
pub const ROLE_MASTER: &str = "MASTER";
/// The role word of MQTT.
pub const ROLE_PUB_SUB: &str = "PUB/SUB";
/// The role word of a mesh peer.
pub const ROLE_PEER: &str = "PEER";
/// The role word of runtime discovery.
pub const ROLE_ADVERTISE: &str = "ADVERTISE";
/// The role word of the realtime exchange.
pub const ROLE_SAME_HOST: &str = "SAME-HOST";
/// The role word of a federation policy.
pub const ROLE_POLICY: &str = "POLICY";
/// The role word of local I/O.
pub const ROLE_IO: &str = "I/O";
/// `PUB/SUB` in an inspector's kind.
pub const CAPTION_PUB_SUB: &str = "Pub/Sub";
/// `SAME-HOST` in an inspector's kind.
pub const CAPTION_SAME_HOST: &str = "Same-host";

// ---- the inspector (specification 25 §5.4 and §5.13) ----

/// The kind of an ADS client's inspector.
pub const ADS_CONNECTION: &str = "ADS connection";

/// The kind of a connection's inspector: its role and whether it is a device or an endpoint,
/// such as `Client · device`.
#[must_use]
pub fn connection_kind(role: &str, device: bool) -> String {
    let what = if device { "device" } else { "endpoint" };
    format!("{role} · {what}")
}

/// The row of a connection's name.
pub const ROW_NAME: &str = "Name";
/// The row of a connection's protocol.
pub const ROW_PROTOCOL: &str = "Protocol";
/// The row of a connection's role.
pub const ROW_ROLE: &str = "Role";
/// The connector row of the connection's lifecycle (specification 25 §5.5).
pub const ROW_CONNECTION: &str = "Connection";
/// The connector row of the connection's health.
pub const ROW_HEALTH: &str = "Health";
/// The connector row of the discovery confidence, and the server row of its client evidence.
pub const ROW_VERIFICATION: &str = "Verification";
/// The connector row of the point counts.
pub const ROW_SIGNALS: &str = "Signals";
/// The row of the last time something was seen.
pub const ROW_LAST_SEEN: &str = "Last seen";
/// The row of a response time.
pub const ROW_RESPONSE_TIME: &str = "Response time";
/// The row of a single live value.
pub const ROW_VALUE: &str = "Value";
/// The heading of the connector rows.
pub const SECTION_CONNECTOR: &str = "Connector";
/// The heading of the live rows.
pub const SECTION_LIVE_VALUES: &str = "Live values";

/// The signals of a connector: the good points, and the degraded and unavailable ones together
/// (specification 25 §5.5).
#[must_use]
pub fn signals(good: usize, attention: usize) -> String {
    if attention == 0 {
        format!("{good} good")
    } else {
        format!("{good} good, {attention} need attention")
    }
}

// ---- the summary of the settings (specification 25 §5.2 and §5.4) ----

/// The summary label of a list of connections.
pub const LABEL_CONNECTIONS: &str = "Connections";
/// The summary label of the ADS server's write switch.
pub const LABEL_WRITES_ENABLED: &str = "Writes enabled";
/// The summary label of a server's exposed globals.
pub const LABEL_EXPOSED_GLOBALS: &str = "Exposed globals";
/// The summary label of an enable switch.
pub const LABEL_ENABLED: &str = "Enabled";
/// The summary label of the OPC UA client's connection file.
pub const LABEL_CONNECTION_FILE: &str = "Connection file";
/// The summary label of another configuration file.
pub const LABEL_CONFIG_FILE: &str = "Config file";
/// The summary label of a poll interval.
pub const LABEL_POLLING: &str = "Polling";

/// An empty list.
pub const NONE: &str = "None";
/// A secret that is set; its value is never shown.
pub const SECRET_SET: &str = "••• (set)";
/// A secret that is not set.
pub const SECRET_NOT_SET: &str = "—";
/// A switch that is on.
pub const ON: &str = "On";
/// A switch that is off.
pub const OFF: &str = "Off";
/// A point without a name.
pub const UNNAMED: &str = "(unnamed)";
/// `on_error = "fault"`.
pub const ON_ERROR_FAULT: &str = "Stop with fault";
/// `on_error = "warn"`.
pub const ON_ERROR_WARN: &str = "Warn and continue";
/// `on_error = "ignore"`.
pub const ON_ERROR_IGNORE: &str = "Ignore";
/// The noun of an entry of a list of connections without a name.
pub const CONNECTION_NOUN: &str = "connection";
/// The noun of an `EtherCAT` module without a model.
pub const MODULE_NOUN: &str = "module";
/// The noun of an ADS server's allowed client without an AMS Net ID.
pub const CLIENT_NOUN: &str = "client";

/// An entry of a list by its place, such as `connection 2`.
#[must_use]
pub fn numbered(noun: &str, number: usize) -> String {
    format!("{noun} {number}")
}

/// A count with its noun, such as `1 channel` or `2 channels`.
#[must_use]
pub fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// The slot of a module.
#[must_use]
pub fn slot(slot: &str) -> String {
    format!("slot {slot}")
}

/// The channels that a module offers.
#[must_use]
pub fn module_channels(count: u64) -> String {
    if count == 1 {
        "1 channel".to_owned()
    } else {
        format!("{count} channels")
    }
}

/// The selected channels of an `EtherCAT` bus.
#[must_use]
pub fn channels(names: &[String]) -> String {
    format!("{}: {}", counted(names.len(), "channel"), names.join(", "))
}

/// The mock input frames of an `EtherCAT` bus.
#[must_use]
pub fn mock_frames(count: usize) -> String {
    counted(count, "mock frame")
}

/// The globals that a server exposes.
#[must_use]
pub fn globals(names: &[String]) -> String {
    format!("{}: {}", counted(names.len(), "global"), names.join(", "))
}

/// A Modbus register of a point.
#[must_use]
pub fn register(register: &str) -> String {
    format!("register {register}")
}

/// An index of a point.
#[must_use]
pub fn index(index: &str) -> String {
    format!("index {index}")
}

/// An allowed client of the ADS server that is pinned to a source address.
#[must_use]
pub fn client_from(net_id: &str, source: &str) -> String {
    format!("{net_id} (from {source})")
}

/// An allowed client of the ADS server that is not pinned to a source address.
#[must_use]
pub fn unpinned_client(net_id: &str) -> String {
    format!("{net_id} (unpinned lab client)")
}

// ---- servers (specification 25 §5.8) ----

/// The row of where a server listens.
pub const ROW_SERVER_ENDPOINT: &str = "Server endpoint";
/// The row of the clients connected to a server.
pub const ROW_CONNECTED_CLIENTS: &str = "Connected clients";
/// No live evidence of the clients.
pub const NO_LIVE_CLIENT_EVIDENCE: &str = "No live client evidence";
/// No independent client proved the server.
pub const NO_EXTERNAL_CLIENT_VERIFIED: &str = "No external client verified";
/// Only the self-test proved the server.
pub const SELF_TEST_ONLY: &str = "Self-test available; no external client verified";
/// An external client proved the server for production, without its name.
pub const PRODUCTION_VERIFIED: &str = "Production verified by external client";
/// An external client proved the server, without its name.
pub const VERIFIED: &str = "Verified by external client";

/// An AMS Net ID.
#[must_use]
pub fn ams_net_id(net_id: &str) -> String {
    format!("AMS Net ID {net_id}")
}

/// An ADS port.
#[must_use]
pub fn ads_port(port: &str) -> String {
    format!("ADS port {port}")
}

/// The clients connected to a server.
#[must_use]
pub fn clients_connected(count: u64) -> String {
    if count == 1 {
        "1 client connected".to_owned()
    } else {
        format!("{count} clients connected")
    }
}

/// An external client that proved the server for production.
#[must_use]
pub fn production_verified_by(client: &str) -> String {
    format!("Production verified by {client}")
}

/// An external client that proved the server.
#[must_use]
pub fn verified_by(client: &str) -> String {
    format!("Verified by {client}")
}

// ---- the ADS client (specification 25 §5.13) ----

/// The ADS client of a running runtime that has no evidence yet.
pub const ADS_RUNTIME_RUNNING: &str = "Runtime running — ADS is configured.";
/// The ADS client of a stopped runtime.
pub const ADS_RUNTIME_STOPPED: &str = "Runtime stopped — start it to read tags.";
/// The ADS client while it reads its tags.
pub const ADS_CONNECTED: &str = "Connected — reading configured ADS tags.";
/// The ADS client while it is turned off.
pub const ADS_DISABLED: &str = "ADS is disabled — enable it in Advanced settings.";
/// The ADS client with a problem that has no detail.
pub const ADS_NEEDS_ATTENTION: &str = "ADS communication needs attention.";
/// The ADS client in another state without a detail.
pub const ADS_CONFIGURED: &str = "ADS is configured.";
/// The row of the ADS devices when there are none.
pub const ROW_ADS_DEVICES: &str = "ADS devices";
/// No ADS device is configured.
pub const NO_ADS_DEVICE: &str = "No ADS device is configured.";
/// The row of an ADS device's AMS Net ID.
pub const ROW_AMS_NET_ID: &str = "AMS Net ID";
/// The row of an ADS device without ports.
pub const ROW_ADS_PORTS: &str = "ADS ports";
/// An ADS device without ports.
pub const NO_PORTS: &str = "No ports configured";
/// The heading of the ADS client's settings.
pub const SECTION_ADVANCED_SETTINGS: &str = "Advanced settings";
/// The row of the ADS client's update interval.
pub const ROW_UPDATE_INTERVAL: &str = "Update interval";

/// The heading of one of several ADS devices.
#[must_use]
pub fn ads_device(number: usize) -> String {
    format!("ADS device {number}")
}

/// The tags read from one ADS port.
#[must_use]
pub fn tags(count: usize) -> String {
    counted(count, "tag")
}

// ---- actions and their reasons ----

/// Opens the connection's form.
pub const EDIT_SETTINGS: &str = "Edit settings";
/// Opens the ADS client's form.
pub const EDIT_ADVANCED_SETTINGS: &str = "Edit advanced settings";
/// Tests the connection.
pub const TEST: &str = "Test";
/// Runs the ADS checks.
pub const DIAGNOSE: &str = "Diagnose";
/// Test of a connection whose runtime does not answer.
pub const TEST_NEEDS_RUNTIME: &str = "Start or connect the runtime to test this connection.";
/// Diagnose of a connection whose runtime does not answer.
pub const DIAGNOSE_NEEDS_RUNTIME: &str =
    "Start or connect the runtime to diagnose this connection.";

/// Tests one of a connection's client connections.
#[must_use]
pub fn test_named(name: &str) -> String {
    format!("Test {name}")
}
