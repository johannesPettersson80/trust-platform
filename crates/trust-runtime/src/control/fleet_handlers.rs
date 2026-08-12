use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;

use crate::ads::diagnostics::{AdsConnectionStatus, AdsConnectionStatusState, AdsStatusReport};
use crate::ads::{AdsClientConfig, AdsConnectionConfig, AdsPointAddress};
use crate::config::{IoConfig, IoDriverConfig};
use crate::discovery::DiscoveryEntry;
use crate::io::{IoAddress, IoDriverHealth, IoDriverStatus, IoSnapshot, IoSnapshotEntry};
use crate::linux_rt::LinuxRtRuntimeStatus;
use crate::memory::IoArea;
use crate::opcua::{OpcUaClientConfig, OpcUaClientConnectionState, OpcUaClientStatusReport};
use crate::scheduler::ResourceCommand;
use crate::settings::RuntimeSettings;
use trust_ads_core::{AdsDataTypeDescriptor, PointAccess};

use super::{ControlResponse, ControlState};

mod offline;

const FLEET_TOPOLOGY_SCHEMA_VERSION: u32 = 4;
const DISCOVERY_STALE_AFTER_MS: u64 = 120_000;
const ADS_STATUS_TIMEOUT: Duration = Duration::from_millis(250);
const OPCUA_CLIENT_STATUS_TIMEOUT: Duration = Duration::from_millis(250);
const CONFIGURED_IO_NO_LIVE_HEALTH_DETAIL: &str =
    "Configured in io.toml; no live driver health has been reported yet.";
const CONFIGURED_ETHERCAT_NO_LIVE_DISCOVERY_DETAIL: &str =
    "Configured in io.toml; live EtherCAT discovery may enrich this row.";
pub(super) const CONFIGURED_IO_RUNTIME_NOT_RUNNING_DETAIL: &str =
    "Configured in io.toml; runtime is not running.";
pub(super) const CONFIGURED_PROJECT_RUNTIME_NOT_RUNNING_DETAIL: &str =
    "Configured in project files; runtime is not running.";
pub(super) const CONFIGURED_RUNTIME_TOML_NOT_RUNNING_DETAIL: &str =
    "Configured in runtime.toml; runtime is not running.";

#[derive(Clone, Copy)]
pub(super) struct ConfiguredDriverDetails<'a> {
    missing_health: &'a str,
    ethercat_child: &'a str,
}

const LIVE_CONFIGURED_DRIVER_DETAILS: ConfiguredDriverDetails<'static> = ConfiguredDriverDetails {
    missing_health: CONFIGURED_IO_NO_LIVE_HEALTH_DETAIL,
    ethercat_child: CONFIGURED_ETHERCAT_NO_LIVE_DISCOVERY_DETAIL,
};

pub(super) const OFFLINE_CONFIGURED_DRIVER_DETAILS: ConfiguredDriverDetails<'static> =
    ConfiguredDriverDetails {
        missing_health: CONFIGURED_IO_RUNTIME_NOT_RUNNING_DETAIL,
        ethercat_child: CONFIGURED_IO_RUNTIME_NOT_RUNNING_DETAIL,
    };

pub(super) fn handle_fleet_topology(id: u64, state: &ControlState) -> ControlResponse {
    let response = match build_fleet_topology(state) {
        Ok(response) => response,
        Err(error) => return ControlResponse::error(id, error),
    };
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => {
            ControlResponse::error(id, format!("fleet topology serialization failed: {error}"))
        }
    }
}

pub(super) fn build_project_fleet_topology_value(
    project_root: &Path,
) -> Result<serde_json::Value, String> {
    let response = offline::build_project_fleet_topology(project_root)?;
    serde_json::to_value(response)
        .map_err(|error| format!("fleet topology serialization failed: {error}"))
}

mod endpoints;
use endpoints::*;
mod host;
use host::*;
mod io;
use io::*;
mod links;
use links::*;
mod protocols;
use protocols::*;
mod runtime;
use runtime::*;
mod topology;
use topology::*;
mod types;
use types::*;

fn build_fleet_topology(state: &ControlState) -> Result<FleetTopologyResponse, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings unavailable".to_string())?
        .clone();
    let io_health = state
        .io_health
        .lock()
        .map_err(|_| "I/O health unavailable".to_string())?
        .clone();
    let io_snapshot = state
        .io_snapshot
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let io_snapshot_seen_ms = state.io_snapshot_seen_ms.load(Ordering::Relaxed);
    let realtime = state.realtime_status.lock().ok().map(|guard| guard.clone());
    let mesh_evidence = state
        .mesh_topology
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let discovery_entries = state.discovery.snapshot();
    let io_drivers = configured_io_drivers(state);
    let ads_client_config = state
        .ads_client_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let opcua_client_config = state
        .opcua_client_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let ads_status = ads_status_report(state);
    let opcua_client_status = opcua_client_status_report(state);

    let runtime_id = state.resource_name.to_string();
    let hostname = host_name();
    let runtime_inputs = FleetRuntimeInputs {
        state,
        settings: &settings,
        io_health: &io_health,
        io_drivers: &io_drivers,
        io_snapshot: io_snapshot.as_ref(),
        io_snapshot_seen_ms,
        realtime: realtime.as_ref(),
        mesh_evidence: mesh_evidence.as_ref(),
        discovery_entries: &discovery_entries,
        ads_client_config: ads_client_config.as_ref(),
        ads_status: ads_status.as_ref(),
        opcua_client_config: opcua_client_config.as_ref(),
        opcua_client_status: opcua_client_status.as_ref(),
    };
    let runtime = runtime_node(&runtime_id, &runtime_inputs);
    let mut local_host = local_host(&hostname, &settings);
    if current_process_is_containerized() {
        local_host.containers.push(current_container(runtime));
    } else {
        local_host.runtimes.push(runtime);
    }

    let mut hosts = vec![local_host];
    hosts.extend(discovered_hosts(
        &runtime_id,
        discovery_entries.as_slice(),
        mesh_evidence.as_ref(),
    ));

    let mut links = topology_links(&runtime_id, &runtime_inputs);
    links.sort_by(|left, right| left.id.cmp(&right.id));
    links.dedup_by(|left, right| left.id == right.id);

    let shared = topology_shared(&runtime_id, io_drivers.as_slice());
    let external = topology_external(
        &runtime_id,
        &settings,
        io_drivers.as_slice(),
        ads_client_config.as_ref(),
        opcua_client_config.as_ref(),
        discovery_entries.as_slice(),
    );
    let discovered = topology_discovered(&runtime_id, discovery_entries.as_slice());

    Ok(FleetTopologyResponse {
        schema_version: FLEET_TOPOLOGY_SCHEMA_VERSION,
        hosts,
        links,
        shared,
        external,
        discovered,
    })
}

#[cfg(test)]
#[path = "fleet_handlers/host_contract_tests.rs"]
mod host_contract_tests;
#[cfg(test)]
#[path = "fleet_handlers/io_contract_tests.rs"]
mod io_contract_tests;
#[cfg(test)]
#[path = "fleet_handlers/link_contract_tests.rs"]
mod link_contract_tests;
#[cfg(test)]
#[path = "fleet_handlers/protocol_contract_tests.rs"]
mod protocol_contract_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "fleet_handlers/topology_contract_tests.rs"]
mod topology_contract_tests;
