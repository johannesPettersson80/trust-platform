//! The texts of the devices and systems at the other end of the connections, of the links and of
//! the mesh bus (specification 25 §5.7 and §5.13). Texts ported from the panel's TypeScript keep
//! its words.

/// What a system presents when no link reaches it.
pub const PRESENTS_EXTERNAL_SYSTEM: &str = "external system";
/// The name of a system without a name and without a link.
pub const EXTERNAL_SYSTEM: &str = "External system";
/// The kind of a shared system's inspector.
pub const SHARED_SYSTEM: &str = "Shared system";

/// The remote end of an OPC UA client.
pub const OPCUA_SERVER: &str = "OPC UA server";
/// The remote end of an OPC UA server.
pub const OPCUA_CLIENT: &str = "OPC UA client";
/// The remote end of an ADS client.
pub const ADS_SERVER: &str = "ADS server";
/// The remote end of an ADS server.
pub const ADS_CLIENT: &str = "ADS client";

/// The counterpart of an MQTT client.
pub const COUNTERPART_BROKER: &str = "broker";
/// The counterpart of a server.
pub const COUNTERPART_CLIENT: &str = "client";
/// The counterpart of a client.
pub const COUNTERPART_SERVER: &str = "server";
/// The counterpart of a fieldbus master.
pub const COUNTERPART_SLAVES: &str = "slaves";
/// The counterpart of a peer, an advertiser, or a link without a role.
pub const COUNTERPART_PEER: &str = "peer";

/// What a system presents: the protocol's product name and the counterpart's role, such as
/// `Modbus TCP server` or `MQTT broker`.
#[must_use]
pub fn presents(protocol: &str, role: &str) -> String {
    format!("{protocol} {role}")
}

/// What a device on a fieldbus without a model presents, such as `EtherCAT device`.
#[must_use]
pub fn bus_device(protocol: &str) -> String {
    format!("{protocol} device")
}

/// The row of what a system presents.
pub const ROW_PRESENTS: &str = "Presents";
/// The row of a system's scope.
pub const ROW_SCOPE: &str = "Scope";
/// The scope of a system outside the fleet in its inspector.
pub const SCOPE_CONFIGURED: &str = "external · configured on our side";
/// The scope of a system outside the fleet on hover.
pub const SCOPE_EXTERNAL: &str = "external system";
/// The row of the protocols that reach a system.
pub const ROW_PROTOCOLS: &str = "Protocols";
/// The row of the runtimes that use a shared system.
pub const ROW_USED_BY: &str = "Used by";
/// The row of a device's model.
pub const ROW_MODEL: &str = "Model";
/// The row of a device's position on its bus.
pub const ROW_SLOT: &str = "Slot";
/// The row of a device's channels.
pub const ROW_CHANNELS: &str = "Channels";
/// The row of the connection whose bus a device is on.
pub const ROW_BUS: &str = "Bus";

/// The label of a mesh bus with more than one peer.
pub const MESH_FABRIC: &str = "Mesh fabric";

/// What a link or a mesh drop shows on hover: its status and, when it has one, its detail.
#[must_use]
pub fn status_and_detail(status: &str, detail: &str) -> String {
    let detail = detail.trim();
    if detail.is_empty() {
        status.to_owned()
    } else {
        format!("{status} — {detail}")
    }
}
