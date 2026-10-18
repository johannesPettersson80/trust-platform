//! The texts of the connection plans (specification 25 §5.8, §5.13 and §5.14): exposing globals
//! through a server, the `EtherCAT` channel selection, an OPC UA client connection built from
//! selected nodes, one ADS connection per responding port, and the browse pane's `Create route`.
//! Texts ported from the panel's TypeScript keep its words.

use crate::texts::connections::protocol_name;

/// The note that a plan appends when the runtime must restart to apply it
/// (`NC/protocolActions.ts:188-192`).
const RESTART: &str = " Restart to apply.";
/// The note that adding an OPC UA client appends (`NC/protocolActions.ts:228-233`).
const RESTART_READ: &str = " Restart the runtime to read it.";

/// `count noun`, the noun pluralized by a trailing `s` (`countLabel`, `NC/protocolActions.ts:697-703`).
#[must_use]
fn count(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// The result of exposing globals through a server, such as
/// `OPC UA server: exposed 2 globals. Restart to apply.` (`NC/protocolActions.ts:190-192`).
#[must_use]
pub fn exposed(protocol: &str, names: usize, restart: bool) -> String {
    let restart = if restart { RESTART } else { "" };
    format!(
        "{}: exposed {}.{restart}",
        protocol_name(protocol),
        count(names, "global")
    )
}

/// Why a server of `name` cannot be exposed through yet (`NC/protocolActions.ts:170-173`).
#[must_use]
pub fn expose_not_configured(name: &str) -> String {
    format!("Configure the {name} first, then choose globals to expose.")
}

/// Why exposing globals failed, with the runtime's reason when it gave one
/// (`NC/protocolActions.ts:198-204`).
#[must_use]
pub fn expose_failed(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("edit the server config in the inspector first.");
    format!("Could not expose globals: {reason}")
}

/// The result of selecting `channels` channels on an `EtherCAT` bus, such as
/// `3 EtherCAT channels selected. Restart to apply.` (`NC/protocolActions.ts:277-281`).
#[must_use]
pub fn ethercat_selected(channels: usize, restart: bool) -> String {
    let restart = if restart { RESTART } else { "" };
    format!("{} selected.{restart}", count(channels, "EtherCAT channel"))
}

/// Why an `EtherCAT` bus cannot be selected on yet (`NC/protocolActions.ts:260-264`).
pub const ETHERCAT_NOT_CONFIGURED: &str = "Configure EtherCAT modules first, then browse channels.";

/// Why the `EtherCAT` channel selection failed, with the runtime's reason when it gave one
/// (`NC/protocolActions.ts:287-292`).
#[must_use]
pub fn ethercat_failed(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("check the configured modules and try again.");
    format!("Could not save EtherCAT channels: {reason}")
}

/// The result of adding an OPC UA client connection with `points` nodes, such as
/// `Added OPC UA client connection with 1 node(s). Restart the runtime to read it.`
/// (`NC/protocolActions.ts:228-233`).
#[must_use]
pub fn opcua_added(points: usize, restart: bool) -> String {
    let restart = if restart { RESTART_READ } else { "" };
    format!("Added OPC UA client connection with {points} node(s).{restart}")
}

/// Why the OPC UA client connection was not saved, with the runtime's reason when it gave one
/// (`NC/protocolActions.ts:240-244`).
#[must_use]
pub fn opcua_failed(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("check the endpoint and try again.");
    format!("Could not save the OPC UA client connection: {reason}")
}

/// The fallback name of an ADS device that the payload named none (`NC/protocolActions.ts:50-53`).
pub const ADS_DEVICE: &str = "ADS device";

/// Why the ADS device cannot be added: it names no address, AMS Net ID or responding port
/// (`NC/protocolActions.ts:58-63`).
pub const ADS_NO_ADDRESS: &str = "Could not add the ADS device. Scan it again so its address, AMS Net ID, and responding ports are available.";

/// Why the ADS device cannot be added: discovery returned no AMS Net ID for this computer
/// (`NC/protocolActions.ts:64-69`).
pub const ADS_NO_LOCAL_NET_ID: &str = "Could not add the ADS device because discovery did not return this computer's AMS Net ID. Scan it again and retry.";

/// The result of adding `connections` ADS connections, such as `Added 2 ADS connections.
/// Restart to apply.` The TypeScript refreshed silently; the panel now reports its result
/// (specification 25 §5.13).
#[must_use]
pub fn ads_added(connections: usize, restart: bool) -> String {
    let restart = if restart { RESTART } else { "" };
    format!("Added {}.{restart}", count(connections, "ADS connection"))
}

/// Why the ADS device was not added, with the runtime's reason when it gave one
/// (`NC/protocolActions.ts:98-103`).
#[must_use]
pub fn ads_failed(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("check the discovered device and try again.");
    format!("Could not add the ADS device: {reason}")
}

/// Why the browse pane's `Create route` cannot run: a route is added over a running runtime, never
/// into a project's files (specification 25 §5.4).
pub const ROUTE_NEEDS_RUNTIME: &str =
    "Creating a TwinCAT route needs a running runtime. Start the runtime and try again.";

/// Why the route cannot be created: the payload names no route, no `TwinCAT` target or no host
/// identity (specification 25 §5.4).
pub const ROUTE_NEEDS_TARGET: &str =
    "The route needs its TwinCAT target. Browse the ADS port again and retry.";

/// Why the route cannot be created: the form carries no administrator login (specification
/// 25:389-394).
pub const ROUTE_NEEDS_LOGIN: &str = "Enter the TwinCAT administrator login to create the route.";

/// The result of creating the `TwinCAT` route `route_name`, which asks the browse pane to retry
/// (specification 25:389-394).
#[must_use]
pub fn route_created(route_name: &str) -> String {
    format!("Created the TwinCAT route '{route_name}'. Browse retries now.")
}

/// Why creating the `TwinCAT` route failed, with the runtime's reason when it gave one
/// (specification 25 §5.4).
#[must_use]
pub fn route_failed(reason: Option<&str>) -> String {
    let reason = reason.unwrap_or("check the TwinCAT target and try again.");
    format!("Could not create the TwinCAT route: {reason}")
}
